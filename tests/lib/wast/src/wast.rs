use crate::error::{DirectiveError, DirectiveErrors};
use crate::spectest::spectest_importobject;
use anyhow::{anyhow, bail, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str;
use wasmer::*;

/// The wast test script language allows modules to be defined and actions
/// to be performed on them.
pub struct Wast {
    /// Wast files have a concept of a "current" module, which is the most
    /// recently defined.
    current: Option<Instance>,
    /// The Import Object that all wast tests will have
    import_object: ImportObject,
    /// The instances in the test
    instances: HashMap<String, Instance>,
    /// Allowed failures (ideally this should be empty)
    allowed_instantiation_failures: HashSet<String>,
    /// If the current module was an allowed failure, we allow test to fail
    current_is_allowed_failure: bool,
    /// The wasm Store
    store: Store,
    /// A flag indicating if Wast tests should stop as soon as one test fails.
    pub fail_fast: bool,
}

impl Wast {
    /// Construct a new instance of `Wast` with a given imports.
    pub fn new(store: Store, import_object: ImportObject) -> Self {
        Self {
            current: None,
            store,
            import_object,
            allowed_instantiation_failures: HashSet::new(),
            current_is_allowed_failure: false,
            instances: HashMap::new(),
            fail_fast: true,
        }
    }

    /// A list of instantiation failures to allow
    pub fn allow_instantiation_failures(&mut self, failures: &[&str]) {
        for &failure_str in failures.iter() {
            self.allowed_instantiation_failures
                .insert(failure_str.to_string());
        }
    }

    /// Construct a new instance of `Wast` with the spectests imports.
    pub fn new_with_spectest(store: Store) -> Self {
        let import_object = spectest_importobject(&store);
        Self::new(store, import_object)
    }

    fn get_instance(&self, instance_name: Option<&str>) -> Result<Instance> {
        match instance_name {
            Some(name) => self
                .instances
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow!("failed to find instance named `{}`", name)),
            None => self
                .current
                .clone()
                .ok_or_else(|| anyhow!("no previous instance found")),
        }
    }

    /// Perform the action portion of a command.
    fn perform_execute(&mut self, exec: wast::WastExecute<'_>) -> Result<Vec<Val>> {
        match exec {
            wast::WastExecute::Invoke(invoke) => self.perform_invoke(invoke),
            wast::WastExecute::Module(mut module) => {
                let binary = module.encode()?;
                let result = self.instantiate(&binary);
                match result {
                    Ok(_) => Ok(Vec::new()),
                    Err(e) => Err(e),
                }
            }
            wast::WastExecute::Get { module, global } => self.get(module.map(|s| s.name()), global),
        }
    }

    fn perform_invoke(&mut self, exec: wast::WastInvoke<'_>) -> Result<Vec<Val>> {
        let values = exec
            .args
            .iter()
            .map(Self::runtime_value)
            .collect::<Result<Vec<_>>>()?;
        self.invoke(exec.module.map(|i| i.name()), exec.name, &values)
    }

    fn assert_return(
        &self,
        result: Result<Vec<Val>>,
        results: &[wast::AssertExpression],
    ) -> Result<()> {
        let values = result?;
        for (v, e) in values.iter().zip(results) {
            if val_matches(v, e)? {
                continue;
            }
            if let Val::V128(bits) = v {
                if let wast::AssertExpression::V128(pattern) = e {
                    bail!(
                        "expected {:?}, got {:?} (v128 bits: {})",
                        e,
                        v128_format(*bits, pattern),
                        bits
                    );
                }
            }
            bail!("expected {:?}, got {:?}", e, v)
        }
        Ok(())
    }

    fn assert_trap(&self, result: Result<Vec<Val>>, expected: &str) -> Result<()> {
        let actual = match result {
            Ok(values) => bail!("expected trap, got {:?}", values),
            Err(t) => format!("{}", t),
        };
        if Self::matches_message_assert_trap(expected, &actual) {
            return Ok(());
        }
        bail!("expected '{}', got '{}'", expected, actual)
    }

    fn run_directive(&mut self, directive: wast::WastDirective) -> Result<()> {
        use wast::WastDirective::*;

        match directive {
            Module(mut module) => {
                let binary = module.encode()?;
                self.module(module.id.map(|s| s.name()), &binary)?;
            }
            Register {
                span: _,
                name,
                module,
            } => {
                self.register(module.map(|s| s.name()), name)?;
            }
            Invoke(i) => {
                self.perform_invoke(i)?;
            }
            AssertReturn {
                span: _,
                exec,
                results,
            } => {
                let result = self.perform_execute(exec);
                self.assert_return(result, &results)?;
            }
            #[cfg(not(feature = "test-no-traps"))]
            AssertTrap {
                span: _,
                exec,
                message,
            } => {
                let result = self.perform_execute(exec);
                self.assert_trap(result, message)?;
            }
            #[cfg(not(feature = "test-no-traps"))]
            AssertExhaustion {
                span: _,
                call,
                message,
            } => {
                let result = self.perform_invoke(call);
                self.assert_trap(result, message)?;
            }
            // See https://github.com/wasmerio/wasmer/issues/1550 for more info
            #[cfg(feature = "test-no-traps")]
            AssertTrap { .. } | AssertExhaustion { .. } => {}
            AssertInvalid {
                span: _,
                mut module,
                message,
            } => {
                let bytes = module.encode()?;
                let err = match self.module(None, &bytes) {
                    Ok(()) => bail!("expected module to fail to build"),
                    Err(e) => e,
                };
                let error_message = format!("{:?}", err);
                if !Self::matches_message_assert_invalid(&message, &error_message) {
                    bail!(
                        "assert_invalid: expected \"{}\", got \"{}\"",
                        message,
                        error_message
                    )
                }
            }
            QuoteModule { .. } => {
                // Do nothing
            }
            AssertMalformed {
                module,
                span: _,
                message: _,
            } => {
                let mut module = match module {
                    wast::QuoteModule::Module(m) => m,
                    // This is a `*.wat` parser test which we're not
                    // interested in.
                    wast::QuoteModule::Quote(_) => return Ok(()),
                };
                let bytes = module.encode()?;
                if self.module(None, &bytes).is_ok() {
                    bail!("expected malformed module to fail to instantiate");
                }
            }
            AssertUnlinkable {
                span: _,
                mut module,
                message,
            } => {
                let bytes = module.encode()?;
                let err = match self.module(None, &bytes) {
                    Ok(()) => bail!("expected module to fail to link"),
                    Err(e) => e,
                };
                let error_message = format!("{:?}", err);
                if !Self::matches_message_assert_unlinkable(&message, &error_message) {
                    bail!(
                        "assert_unlinkable: expected {}, got {}",
                        message,
                        error_message
                    )
                }
            }
        }

        Ok(())
    }

    /// Run a wast script from a byte buffer.
    pub fn run_buffer(&mut self, filename: &str, wast: &[u8]) -> Result<()> {
        let wast = str::from_utf8(wast)?;

        let adjust_wast = |mut err: wast::Error| {
            err.set_path(filename.as_ref());
            err.set_text(wast);
            err
        };

        let buf = wast::parser::ParseBuffer::new(wast).map_err(adjust_wast)?;
        let ast = wast::parser::parse::<wast::Wast>(&buf).map_err(adjust_wast)?;
        let mut errors = Vec::with_capacity(ast.directives.len());
        for directive in ast.directives {
            let sp = directive.span();
            if let Err(e) = self.run_directive(directive) {
                let message = format!("{}", e);
                // If depends on an instance that doesn't exist
                if message.contains("no previous instance found") {
                    continue;
                }
                // We don't compute it, comes from instantiating an instance
                // that we expected to fail.
                if self.current.is_none() && self.current_is_allowed_failure {
                    continue;
                }
                let (line, col) = sp.linecol_in(wast);
                errors.push(DirectiveError {
                    line: line + 1,
                    col,
                    message,
                });
                if self.fail_fast {
                    break;
                }
            }
        }
        if !errors.is_empty() {
            return Err(DirectiveErrors {
                filename: filename.to_string(),
                errors,
            }
            .into());
        }
        Ok(())
    }

    /// Run a wast script from a file.
    pub fn run_file(&mut self, path: &Path) -> Result<()> {
        let bytes = std::fs::read(path)?;
        self.run_buffer(path.to_str().unwrap(), &bytes)
    }
}

// This is the implementation specific to the Runtime
impl Wast {
    /// Define a module and register it.
    fn module(&mut self, instance_name: Option<&str>, module: &[u8]) -> Result<()> {
        let instance = match self.instantiate(module) {
            Ok(i) => i,
            Err(e) => {
                // We set the current to None to allow running other
                // spectests when `fail_fast` is `false`.
                self.current = None;
                let error_message = format!("{}", e);
                self.current_is_allowed_failure = false;
                for allowed_failure in self.allowed_instantiation_failures.iter() {
                    if error_message.contains(allowed_failure) {
                        self.current_is_allowed_failure = true;
                        break;
                    }
                }
                bail!("instantiation failed with: {}", e)
            }
        };
        if let Some(name) = instance_name {
            self.instances.insert(name.to_string(), instance.clone());
        }
        self.current = Some(instance);
        self.current_is_allowed_failure = false;
        Ok(())
    }

    fn instantiate(&self, module: &[u8]) -> Result<Instance> {
        let module = Module::new(&self.store, module)?;
        let mut imports = self.import_object.clone();

        for import in module.imports() {
            let module_name = import.module();
            if imports.contains_namespace(module_name) {
                continue;
            }
            let instance = self
                .instances
                .get(module_name)
                .ok_or_else(|| anyhow!("no module named `{}`", module_name))?;
            imports.register(module_name, instance.exports.clone());
        }

        let instance = Instance::new(&module, &imports)?;
        Ok(instance)
    }

    /// Register an instance to make it available for performing actions.
    fn register(&mut self, name: Option<&str>, as_name: &str) -> Result<()> {
        let instance = self.get_instance(name)?;
        self.instances.insert(as_name.to_string(), instance);
        Ok(())
    }

    /// Invoke an exported function from an instance.
    fn invoke(
        &mut self,
        instance_name: Option<&str>,
        field: &str,
        args: &[Val],
    ) -> Result<Vec<Val>> {
        let instance = self.get_instance(instance_name.as_deref())?;
        let func: &Function = instance.exports.get(field)?;
        match func.call(args) {
            Ok(result) => Ok(result.into()),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the value of an exported global from an instance.
    fn get(&mut self, instance_name: Option<&str>, field: &str) -> Result<Vec<Val>> {
        let instance = self.get_instance(instance_name.as_deref())?;
        let global: &Global = instance.exports.get(field)?;
        Ok(vec![global.get()])
    }

    /// Translate from a `script::Value` to a `Val`.
    fn runtime_value(v: &wast::Expression<'_>) -> Result<Val> {
        use wast::Instruction::*;

        if v.instrs.len() != 1 {
            bail!("too many instructions in {:?}", v);
        }
        Ok(match &v.instrs[0] {
            I32Const(x) => Val::I32(*x),
            I64Const(x) => Val::I64(*x),
            F32Const(x) => Val::F32(f32::from_bits(x.bits)),
            F64Const(x) => Val::F64(f64::from_bits(x.bits)),
            V128Const(x) => Val::V128(u128::from_le_bytes(x.to_le_bytes())),
            other => bail!("couldn't convert {:?} to a runtime value", other),
        })
    }

    // Checks if the `assert_unlinkable` message matches the expected one
    fn matches_message_assert_unlinkable(expected: &str, actual: &str) -> bool {
        actual.contains(&expected)
    }

    // Checks if the `assert_invalid` message matches the expected one
    fn matches_message_assert_invalid(expected: &str, actual: &str) -> bool {
        actual.contains(expected)
            // Waiting on https://github.com/WebAssembly/bulk-memory-operations/pull/137
            // to propagate to WebAssembly/testsuite.
            || (expected.contains("unknown table") && actual.contains("unknown elem"))
            // wasmparser return the wrong message
            || (expected.contains("unknown memory") && actual.contains("no linear memories are present"))
            // `elem.wast` and `proposals/bulk-memory-operations/elem.wast` disagree
            // on the expected error message for the same error.
            || (expected.contains("out of bounds") && actual.contains("does not fit"))
    }

    // Checks if the `assert_trap` message matches the expected one
    fn matches_message_assert_trap(expected: &str, actual: &str) -> bool {
        actual.contains(expected)
            // `bulk-memory-operations/bulk.wast` checks for a message that
            // specifies which element is uninitialized, but our traps don't
            // shepherd that information out.
            || (expected.contains("uninitialized element 2") && actual.contains("uninitialized element"))
    }
}

fn extract_lane_as_i8(bytes: u128, lane: usize) -> i8 {
    (bytes >> (lane * 8)) as i8
}

fn extract_lane_as_i16(bytes: u128, lane: usize) -> i16 {
    (bytes >> (lane * 16)) as i16
}

fn extract_lane_as_i32(bytes: u128, lane: usize) -> i32 {
    (bytes >> (lane * 32)) as i32
}

fn extract_lane_as_i64(bytes: u128, lane: usize) -> i64 {
    (bytes >> (lane * 64)) as i64
}

fn val_matches(actual: &Val, expected: &wast::AssertExpression) -> Result<bool> {
    Ok(match (actual, expected) {
        (Val::I32(a), wast::AssertExpression::I32(b)) => a == b,
        (Val::I64(a), wast::AssertExpression::I64(b)) => a == b,
        // Note that these float comparisons are comparing bits, not float
        // values, so we're testing for bit-for-bit equivalence
        (Val::F32(a), wast::AssertExpression::F32(b)) => f32_matches(*a, b),
        (Val::F64(a), wast::AssertExpression::F64(b)) => f64_matches(*a, b),
        (Val::V128(a), wast::AssertExpression::V128(b)) => v128_matches(*a, b),
        _ => bail!(
            "don't know how to compare {:?} and {:?} yet",
            actual,
            expected
        ),
    })
}

fn f32_matches(actual: f32, expected: &wast::NanPattern<wast::Float32>) -> bool {
    match expected {
        wast::NanPattern::CanonicalNan => actual.is_canonical_nan(),
        wast::NanPattern::ArithmeticNan => actual.is_arithmetic_nan(),
        wast::NanPattern::Value(expected_value) => actual.to_bits() == expected_value.bits,
    }
}

fn f64_matches(actual: f64, expected: &wast::NanPattern<wast::Float64>) -> bool {
    match expected {
        wast::NanPattern::CanonicalNan => actual.is_canonical_nan(),
        wast::NanPattern::ArithmeticNan => actual.is_arithmetic_nan(),
        wast::NanPattern::Value(expected_value) => actual.to_bits() == expected_value.bits,
    }
}

fn v128_matches(actual: u128, expected: &wast::V128Pattern) -> bool {
    match expected {
        wast::V128Pattern::I8x16(b) => b
            .iter()
            .enumerate()
            .all(|(i, b)| *b == extract_lane_as_i8(actual, i)),
        wast::V128Pattern::I16x8(b) => b
            .iter()
            .enumerate()
            .all(|(i, b)| *b == extract_lane_as_i16(actual, i)),
        wast::V128Pattern::I32x4(b) => b
            .iter()
            .enumerate()
            .all(|(i, b)| *b == extract_lane_as_i32(actual, i)),
        wast::V128Pattern::I64x2(b) => b
            .iter()
            .enumerate()
            .all(|(i, b)| *b == extract_lane_as_i64(actual, i)),
        wast::V128Pattern::F32x4(b) => b.iter().enumerate().all(|(i, b)| {
            let a = extract_lane_as_i32(actual, i) as u32;
            f32_matches(f32::from_bits(a), b)
        }),
        wast::V128Pattern::F64x2(b) => b.iter().enumerate().all(|(i, b)| {
            let a = extract_lane_as_i64(actual, i) as u64;
            f64_matches(f64::from_bits(a), b)
        }),
    }
}

fn v128_format(actual: u128, expected: &wast::V128Pattern) -> wast::V128Pattern {
    match expected {
        wast::V128Pattern::I8x16(_) => wast::V128Pattern::I8x16([
            extract_lane_as_i8(actual, 0),
            extract_lane_as_i8(actual, 1),
            extract_lane_as_i8(actual, 2),
            extract_lane_as_i8(actual, 3),
            extract_lane_as_i8(actual, 4),
            extract_lane_as_i8(actual, 5),
            extract_lane_as_i8(actual, 6),
            extract_lane_as_i8(actual, 7),
            extract_lane_as_i8(actual, 8),
            extract_lane_as_i8(actual, 9),
            extract_lane_as_i8(actual, 10),
            extract_lane_as_i8(actual, 11),
            extract_lane_as_i8(actual, 12),
            extract_lane_as_i8(actual, 13),
            extract_lane_as_i8(actual, 14),
            extract_lane_as_i8(actual, 15),
        ]),
        wast::V128Pattern::I16x8(_) => wast::V128Pattern::I16x8([
            extract_lane_as_i16(actual, 0),
            extract_lane_as_i16(actual, 1),
            extract_lane_as_i16(actual, 2),
            extract_lane_as_i16(actual, 3),
            extract_lane_as_i16(actual, 4),
            extract_lane_as_i16(actual, 5),
            extract_lane_as_i16(actual, 6),
            extract_lane_as_i16(actual, 7),
        ]),
        wast::V128Pattern::I32x4(_) => wast::V128Pattern::I32x4([
            extract_lane_as_i32(actual, 0),
            extract_lane_as_i32(actual, 1),
            extract_lane_as_i32(actual, 2),
            extract_lane_as_i32(actual, 3),
        ]),
        wast::V128Pattern::I64x2(_) => wast::V128Pattern::I64x2([
            extract_lane_as_i64(actual, 0),
            extract_lane_as_i64(actual, 1),
        ]),
        wast::V128Pattern::F32x4(_) => wast::V128Pattern::F32x4([
            wast::NanPattern::Value(wast::Float32 {
                bits: extract_lane_as_i32(actual, 0) as _,
            }),
            wast::NanPattern::Value(wast::Float32 {
                bits: extract_lane_as_i32(actual, 1) as _,
            }),
            wast::NanPattern::Value(wast::Float32 {
                bits: extract_lane_as_i32(actual, 2) as _,
            }),
            wast::NanPattern::Value(wast::Float32 {
                bits: extract_lane_as_i32(actual, 3) as _,
            }),
        ]),
        wast::V128Pattern::F64x2(_) => wast::V128Pattern::F64x2([
            wast::NanPattern::Value(wast::Float64 {
                bits: extract_lane_as_i64(actual, 0) as _,
            }),
            wast::NanPattern::Value(wast::Float64 {
                bits: extract_lane_as_i64(actual, 1) as _,
            }),
        ]),
    }
}

pub trait NaNCheck {
    fn is_arithmetic_nan(&self) -> bool;
    fn is_canonical_nan(&self) -> bool;
}

impl NaNCheck for f32 {
    fn is_arithmetic_nan(&self) -> bool {
        const AF32_NAN: u32 = 0x0040_0000;
        (self.to_bits() & AF32_NAN) == AF32_NAN
    }

    fn is_canonical_nan(&self) -> bool {
        (self.to_bits() & 0x7fff_ffff) == 0x7fc0_0000
    }
}

impl NaNCheck for f64 {
    fn is_arithmetic_nan(&self) -> bool {
        const AF64_NAN: u64 = 0x0008_0000_0000_0000;
        (self.to_bits() & AF64_NAN) == AF64_NAN
    }

    fn is_canonical_nan(&self) -> bool {
        (self.to_bits() & 0x7fff_ffff_ffff_ffff) == 0x7ff8_0000_0000_0000
    }
}
