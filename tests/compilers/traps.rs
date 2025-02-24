use crate::utils::get_store;
use anyhow::Result;
use std::panic::{self, AssertUnwindSafe};
use wasmer::*;

#[test]
fn test_trap_return() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module
        (func $hello (import "" "hello"))
        (func (export "run") (call $hello))
        )
    "#;

    let module = Module::new(&store, wat)?;
    let hello_type = FunctionType::new(vec![], vec![]);
    let hello_func = Function::new(&store, &hello_type, |_| Err(RuntimeError::new("test 123")));

    let instance = Instance::new(
        &module,
        &imports! {
            "" => {
                "hello" => hello_func
            }
        },
    )?;
    let run_func = instance
        .exports
        .get_function("run")
        .expect("expected function export");

    let e = run_func.call(&[]).err().expect("error calling function");

    assert_eq!(e.message(), "test 123");

    Ok(())
}

#[test]
#[cfg_attr(feature = "test-singlepass", ignore)]
fn test_trap_trace() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module $hello_mod
            (func (export "run") (call $hello))
            (func $hello (unreachable))
        )
    "#;

    let module = Module::new(&store, wat)?;
    let instance = Instance::new(&module, &imports! {})?;
    let run_func = instance
        .exports
        .get_function("run")
        .expect("expected function export");

    let e = run_func.call(&[]).err().expect("error calling function");

    let trace = e.trace();
    assert_eq!(trace.len(), 2);
    assert_eq!(trace[0].module_name(), "hello_mod");
    assert_eq!(trace[0].func_index(), 1);
    assert_eq!(trace[0].function_name(), Some("hello"));
    assert_eq!(trace[1].module_name(), "hello_mod");
    assert_eq!(trace[1].func_index(), 0);
    assert_eq!(trace[1].function_name(), None);
    assert!(
        e.message().contains("unreachable"),
        "wrong message: {}",
        e.message()
    );

    Ok(())
}

#[test]
fn test_trap_trace_cb() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module $hello_mod
            (import "" "throw" (func $throw))
            (func (export "run") (call $hello))
            (func $hello (call $throw))
        )
    "#;

    let fn_type = FunctionType::new(vec![], vec![]);
    let fn_func = Function::new(&store, &fn_type, |_| Err(RuntimeError::new("cb throw")));

    let module = Module::new(&store, wat)?;
    let instance = Instance::new(
        &module,
        &imports! {
            "" => {
                "throw" => fn_func
            }
        },
    )?;
    let run_func = instance
        .exports
        .get_function("run")
        .expect("expected function export");

    let e = run_func.call(&[]).err().expect("error calling function");

    let trace = e.trace();
    println!("Trace {:?}", trace);
    // TODO: Reenable this (disabled as it was not working with llvm/singlepass)
    // assert_eq!(trace.len(), 2);
    // assert_eq!(trace[0].module_name(), "hello_mod");
    // assert_eq!(trace[0].func_index(), 2);
    // assert_eq!(trace[1].module_name(), "hello_mod");
    // assert_eq!(trace[1].func_index(), 1);
    assert_eq!(e.message(), "cb throw");

    Ok(())
}

#[test]
#[cfg_attr(feature = "test-singlepass", ignore)]
fn test_trap_stack_overflow() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module $rec_mod
            (func $run (export "run") (call $run))
        )
    "#;

    let module = Module::new(&store, wat)?;
    let instance = Instance::new(&module, &imports! {})?;
    let run_func = instance
        .exports
        .get_function("run")
        .expect("expected function export");

    let e = run_func.call(&[]).err().expect("error calling function");

    let trace = e.trace();
    assert!(trace.len() >= 32);
    for i in 0..trace.len() {
        assert_eq!(trace[i].module_name(), "rec_mod");
        assert_eq!(trace[i].func_index(), 0);
        assert_eq!(trace[i].function_name(), Some("run"));
    }
    assert!(e.message().contains("call stack exhausted"));

    Ok(())
}

#[test]
#[cfg_attr(any(feature = "test-singlepass", feature = "test-llvm"), ignore)]
fn trap_display_pretty() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module $m
            (func $die unreachable)
            (func call $die)
            (func $foo call 1)
            (func (export "bar") call $foo)
        )
    "#;

    let module = Module::new(&store, wat)?;
    let instance = Instance::new(&module, &imports! {})?;
    let run_func = instance
        .exports
        .get_function("bar")
        .expect("expected function export");

    let e = run_func.call(&[]).err().expect("error calling function");
    assert_eq!(
        e.to_string(),
        "\
RuntimeError: unreachable
    at die (m[0]:0x23)
    at <unnamed> (m[1]:0x27)
    at foo (m[2]:0x2c)
    at <unnamed> (m[3]:0x31)"
    );
    Ok(())
}

#[test]
#[cfg_attr(any(feature = "test-singlepass", feature = "test-llvm"), ignore)]
fn trap_display_multi_module() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module $a
            (func $die unreachable)
            (func call $die)
            (func $foo call 1)
            (func (export "bar") call $foo)
        )
    "#;

    let module = Module::new(&store, wat)?;
    let instance = Instance::new(&module, &imports! {})?;
    let bar = instance.exports.get_function("bar")?.clone();

    let wat = r#"
        (module $b
            (import "" "" (func $bar))
            (func $middle call $bar)
            (func (export "bar2") call $middle)
        )
    "#;
    let module = Module::new(&store, wat)?;
    let instance = Instance::new(
        &module,
        &imports! {
            "" => {
                "" => bar
            }
        },
    )?;
    let bar2 = instance
        .exports
        .get_function("bar2")
        .expect("expected function export");

    let e = bar2.call(&[]).err().expect("error calling function");
    assert_eq!(
        e.to_string(),
        "\
RuntimeError: unreachable
    at die (a[0]:0x23)
    at <unnamed> (a[1]:0x27)
    at foo (a[2]:0x2c)
    at <unnamed> (a[3]:0x31)
    at middle (b[1]:0x29)
    at <unnamed> (b[2]:0x2e)"
    );
    Ok(())
}

#[test]
fn trap_start_function_import() -> Result<()> {
    let store = get_store();
    let binary = r#"
        (module $a
            (import "" "" (func $foo))
            (start $foo)
        )
    "#;

    let module = Module::new(&store, &binary)?;
    let sig = FunctionType::new(vec![], vec![]);
    let func = Function::new(&store, &sig, |_| Err(RuntimeError::new("user trap")));
    let err = Instance::new(
        &module,
        &imports! {
            "" => {
                "" => func
            }
        },
    )
    .err()
    .unwrap();
    match err {
        InstantiationError::Link(_) => panic!("It should be a start error"),
        InstantiationError::Start(err) => {
            assert_eq!(err.message(), "user trap");
        }
    }

    Ok(())
}

#[test]
fn rust_panic_import() -> Result<()> {
    let store = get_store();
    let binary = r#"
        (module $a
            (import "" "foo" (func $foo))
            (import "" "bar" (func $bar))
            (func (export "foo") call $foo)
            (func (export "bar") call $bar)
        )
    "#;

    let module = Module::new(&store, &binary)?;
    let sig = FunctionType::new(vec![], vec![]);
    let func = Function::new(&store, &sig, |_| panic!("this is a panic"));
    let instance = Instance::new(
        &module,
        &imports! {
            "" => {
                "foo" => func,
                "bar" => Function::new_native(&store, || panic!("this is another panic"))
            }
        },
    )?;
    let func = instance.exports.get_function("foo")?.clone();
    let err = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(func.call(&[]));
    }))
    .unwrap_err();
    assert_eq!(err.downcast_ref::<&'static str>(), Some(&"this is a panic"));

    // TODO: Reenable this (disabled as it was not working with llvm/singlepass)
    // It doesn't work either with cranelift and `--test-threads=1`.
    // let func = instance.exports.get_function("bar")?.clone();
    // let err = panic::catch_unwind(AssertUnwindSafe(|| {
    //     drop(func.call(&[]));
    // }))
    // .unwrap_err();
    // assert_eq!(
    //     err.downcast_ref::<&'static str>(),
    //     Some(&"this is another panic")
    // );
    Ok(())
}

#[test]
fn rust_panic_start_function() -> Result<()> {
    let store = get_store();
    let binary = r#"
        (module $a
            (import "" "" (func $foo))
            (start $foo)
        )
    "#;

    let module = Module::new(&store, &binary)?;
    let sig = FunctionType::new(vec![], vec![]);
    let func = Function::new(&store, &sig, |_| panic!("this is a panic"));
    let err = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(Instance::new(
            &module,
            &imports! {
                "" => {
                    "" => func
                }
            },
        ));
    }))
    .unwrap_err();
    assert_eq!(err.downcast_ref::<&'static str>(), Some(&"this is a panic"));

    let func = Function::new_native(&store, || panic!("this is another panic"));
    let err = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(Instance::new(
            &module,
            &imports! {
                "" => {
                    "" => func
                }
            },
        ));
    }))
    .unwrap_err();
    assert_eq!(
        err.downcast_ref::<&'static str>(),
        Some(&"this is another panic")
    );
    Ok(())
}

#[test]
fn mismatched_arguments() -> Result<()> {
    let store = get_store();
    let binary = r#"
        (module $a
            (func (export "foo") (param i32))
        )
    "#;

    let module = Module::new(&store, &binary)?;
    let instance = Instance::new(&module, &imports! {})?;
    let func: &Function = instance.exports.get("foo")?;
    assert_eq!(
        func.call(&[]).unwrap_err().message(),
        "Parameters of type [] did not match signature [I32] -> []"
    );
    assert_eq!(
        func.call(&[Val::F32(0.0)]).unwrap_err().message(),
        "Parameters of type [F32] did not match signature [I32] -> []",
    );
    assert_eq!(
        func.call(&[Val::I32(0), Val::I32(1)])
            .unwrap_err()
            .message(),
        "Parameters of type [I32, I32] did not match signature [I32] -> []"
    );
    Ok(())
}

#[test]
#[cfg_attr(any(feature = "test-singlepass", feature = "test-llvm"), ignore)]
fn call_signature_mismatch() -> Result<()> {
    let store = get_store();
    let binary = r#"
        (module $a
            (func $foo
                i32.const 0
                call_indirect)
            (func $bar (param i32))
            (start $foo)

            (table 1 anyfunc)
            (elem (i32.const 0) 1)
        )
    "#;

    let module = Module::new(&store, &binary)?;
    let err = Instance::new(&module, &imports! {})
        .err()
        .expect("expected error");
    assert_eq!(
        format!("{}", err),
        "\
RuntimeError: indirect call type mismatch
    at foo (a[0]:0x30)\
"
    );
    Ok(())
}

#[test]
#[cfg_attr(any(feature = "test-singlepass", feature = "test-llvm"), ignore)]
fn start_trap_pretty() -> Result<()> {
    let store = get_store();
    let wat = r#"
        (module $m
            (func $die unreachable)
            (func call $die)
            (func $foo call 1)
            (func $start call $foo)
            (start $start)
        )
    "#;

    let module = Module::new(&store, wat)?;
    let err = Instance::new(&module, &imports! {})
        .err()
        .expect("expected error");

    assert_eq!(
        format!("{}", err),
        "\
RuntimeError: unreachable
    at die (m[0]:0x1d)
    at <unnamed> (m[1]:0x21)
    at foo (m[2]:0x26)
    at start (m[3]:0x2b)\
"
    );
    Ok(())
}

#[test]
fn present_after_module_drop() -> Result<()> {
    let store = get_store();
    let module = Module::new(&store, r#"(func (export "foo") unreachable)"#)?;
    let instance = Instance::new(&module, &imports! {})?;
    let func: Function = instance.exports.get_function("foo")?.clone();

    println!("asserting before we drop modules");
    assert_trap(func.call(&[]).unwrap_err());
    drop((instance, module));

    println!("asserting after drop");
    assert_trap(func.call(&[]).unwrap_err());
    return Ok(());

    fn assert_trap(t: RuntimeError) {
        println!("{}", t);
        // assert_eq!(t.trace().len(), 1);
        // assert_eq!(t.trace()[0].func_index(), 0);
    }
}
