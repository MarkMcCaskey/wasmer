# uname only works in *Unix like systems
ifneq ($(OS), Windows_NT)
	ARCH := $(shell uname -m)
	UNAME_S := $(shell uname -s)
else
	# We can assume, if in windows it will likely be in x86_64
	ARCH := x86_64
	UNAME_S := 
endif

compilers :=

ifeq ($(ARCH), x86_64)
	# In X64, Cranelift is enabled
	compilers += cranelift
	# LLVM could be enabled if not in Windows
	ifneq ($(OS), Windows_NT)
		# Singlepass doesn't work yet on Windows
		compilers += singlepass
		# Autodetect LLVM from llvm-config
		ifneq (, $(shell which llvm-config))
			LLVM_VERSION := $(shell llvm-config --version)
			# If findstring is not empty, then it have found the value
			ifneq (, $(findstring 10,$(LLVM_VERSION)))
				compilers += llvm
			endif
		else
			ifneq (, $(shell which llvm-config-10))
				compilers += llvm
			endif
		endif
	endif
endif

compilers := $(filter-out ,$(compilers))

ifneq ($(OS), Windows_NT)
	bold := $(shell tput bold)
	green := $(shell tput setaf 2)
	reset := $(shell tput sgr0)
endif


$(info Available compilers: $(bold)$(green)${compilers}$(reset))

compiler_features_spaced := $(foreach compiler,$(compilers),$(compiler))
compiler_features := --features "$(compiler_features_spaced)"


############
# Building #
############

bench:
	cargo bench $(compiler_features)

build-wasmer:
	cargo build --release --manifest-path lib/cli/Cargo.toml $(compiler_features)

build-wasmer-debug:
	cargo build --manifest-path lib/cli/Cargo.toml $(compiler_features)

WAPM_VERSION = v0.5.0
build-wapm:
	git clone --branch $(WAPM_VERSION) https://github.com/wasmerio/wapm-cli.git
	cargo build --release --manifest-path wapm-cli/Cargo.toml --features "telemetry update-notifications"

build-docs:
	cargo doc --release $(compiler_features) --document-private-items --no-deps --workspace

build-docs-capi:
	cd lib/c-api/ && doxygen doxyfile

# We use cranelift as the default backend for the capi for now
build-capi: build-capi-cranelift

build-capi-singlepass:
	cargo build --manifest-path lib/c-api/Cargo.toml --release \
		--no-default-features --features jit,singlepass,wasi

build-capi-cranelift:
	cargo build --manifest-path lib/c-api/Cargo.toml --release \
		--no-default-features --features jit,cranelift,wasi

build-capi-llvm:
	cargo build --manifest-path lib/c-api/Cargo.toml --release \
		--no-default-features --features jit,llvm,wasi


###########
# Testing #
###########

test: $(foreach compiler,$(compilers),test-$(compiler)) test-packages test-examples test-deprecated

test-singlepass:
	cargo test --release $(compiler_features) --features "test-singlepass"

test-cranelift:
	cargo test --release $(compiler_features) --features "test-cranelift"

test-llvm:
	cargo test --release $(compiler_features) --features "test-llvm"

test-packages:
	cargo test -p wasmer --release
	cargo test -p wasmer-vm --release
	cargo test -p wasmer-types --release
	cargo test -p wasmer-wasi --release
	cargo test -p wasmer-object --release
	cargo test -p wasmer-engine-native --release --no-default-features

test-capi-singlepass: build-capi-singlepass
	cargo test --manifest-path lib/c-api/Cargo.toml --release \
		--no-default-features --features jit,singlepass,wasi -- --nocapture

test-capi-cranelift: build-capi-cranelift
	cargo test --manifest-path lib/c-api/Cargo.toml --release \
		--no-default-features --features jit,cranelift,wasi -- --nocapture

test-capi-llvm: build-capi-llvm
	cargo test --manifest-path lib/c-api/Cargo.toml --release \
		--no-default-features --features jit,llvm,wasi -- --nocapture

test-capi: test-capi-singlepass test-capi-cranelift test-capi-llvm

test-wasi-unit:
	cargo test --manifest-path lib/wasi/Cargo.toml --release

test-examples:
	cargo test --release $(compiler_features) --features wasi --examples

test-deprecated:
	cargo test --manifest-path lib/deprecated/runtime-core/Cargo.toml -p wasmer-runtime-core --release
	cargo test --manifest-path lib/deprecated/runtime/Cargo.toml -p wasmer-runtime --release
	cargo test --manifest-path lib/deprecated/runtime/Cargo.toml -p wasmer-runtime --release --examples

#############
# Packaging #
#############

package-wapm:
	mkdir -p "package/bin"
ifeq ($(OS), Windows_NT)
	echo ""
else
	echo "#!/bin/bash\nwapm execute \"\$$@\"" > package/bin/wax
	chmod +x package/bin/wax
endif

package-wasmer:
	mkdir -p "package/bin"
ifeq ($(OS), Windows_NT)
	cp target/release/wasmer.exe package/bin/
else
	cp target/release/wasmer package/bin/
endif

# Comment out WAPM for now to speed up release process.
# cp ./wapm-cli/target/release/wapm package/bin/
# # Create the wax binary as symlink to wapm
# cd package/bin/ && ln -sf wapm wax && chmod +x wax

package-capi:
	mkdir -p "package/include"
	mkdir -p "package/lib"
	cp lib/c-api/wasmer.h* package/include
	cp lib/c-api/doc/index.md package/include/README.md
ifeq ($(OS), Windows_NT)
	cp target/release/wasmer_c_api.dll package/lib
	cp target/release/wasmer_c_api.lib package/lib
else
ifeq ($(UNAME_S), Darwin)
	cp target/release/libwasmer_c_api.dylib package/lib/libwasmer.dylib
	# cp target/release/libwasmer_c_api.a package/lib/libwasmer.a
	# Fix the rpath for the dylib
	install_name_tool -id "@rpath/libwasmer.dylib" package/lib/libwasmer.dylib
else
	cp target/release/libwasmer_c_api.so package/lib/libwasmer.so
	# cp target/release/libwasmer_c_api.a package/lib/libwasmer.a
endif
endif

package-docs: build-docs build-docs-capi
	mkdir -p "package/docs"
	mkdir -p "package/docs/c"
	cp -R target/doc package/docs/crates
	cp -R lib/c-api/doc/html package/docs/c-api
	echo '<!-- Build $(SOURCE_VERSION) --><meta http-equiv="refresh" content="0; url=rust/wasmer_vm/index.html">' > package/docs/index.html
	echo '<!-- Build $(SOURCE_VERSION) --><meta http-equiv="refresh" content="0; url=wasmer_vm/index.html">' > package/docs/crates/index.html

package: package-wapm package-wasmer package-capi
	cp LICENSE package/LICENSE
	cp ATTRIBUTIONS.md package/ATTRIBUTIONS
	mkdir -p dist
ifeq ($(OS), Windows_NT)
	iscc scripts/windows-installer/wasmer.iss
	cp scripts/windows-installer/WasmerInstaller.exe dist/wasmer-windows.exe
else
	cp LICENSE package/LICENSE
	cp ATTRIBUTIONS.md package/ATTRIBUTIONS
	tar -C package -zcvf wasmer.tar.gz bin lib include LICENSE ATTRIBUTIONS
	cp ./wasmer.tar.gz ./dist/$(shell ./scripts/binary-name.sh)
endif

#################
# Miscellaneous #
#################

# Updates the spectests from the repo
update-testsuite:
	git subtree pull --prefix tests/wast/spec https://github.com/WebAssembly/testsuite.git master --squash

RUSTFLAGS := "-D dead-code -D nonstandard-style -D unused-imports -D unused-mut -D unused-variables -D unused-unsafe -D unreachable-patterns -D bad-style -D improper-ctypes -D unused-allocation -D unused-comparisons -D while-true -D unconditional-recursion -D bare-trait-objects" # TODO: add `-D missing-docs`
lint:
	cargo fmt --all -- --check
	RUSTFLAGS=${RUSTFLAGS} cargo clippy $(compiler_features)

install-local: package
	tar -C ~/.wasmer -zxvf wasmer.tar.gz

publish-docs:
	git clone -b "gh-pages" --depth=1 https://wasmerbot:$(GITHUB_DOCS_TOKEN)@github.com/wasmerio/wasmer.git api-docs-repo
	cp -R package/docs/* api-docs-repo/
	cd api-docs-repo && git add index.html crates/* c-api/*
	cd api-docs-repo && (git diff-index --quiet HEAD || git commit -m "Publishing GitHub Pages")
	# cd api-docs-repo && git push origin gh-pages
