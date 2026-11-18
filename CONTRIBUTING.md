# Contributing to Lutetium

Thanks for your interest. Lutetium is experimental. Check `audit/TEST_RESULTS.md` for the historical 0.4.2 validation snapshot and `audit/KNOWN_ISSUES.md` for current verified limitations before choosing work.

## Development environment

- Use the pinned Rust toolchain in `rust-toolchain.toml`.
- The workspace forces `x86_64-pc-windows-gnu` from `.cargo/config.toml`.
- Native linking depends on the bundled MinGW-w64 components from rustup. Do not introduce a Visual Studio/MSVC-only dependency.

## Architecture

- `lu-syntax`: lexer, parser and AST.
- `lu-diagnostics`: spans and diagnostics.
- `lu-compiler`: resolver, type checker, typed register IR, lowering, optimizer and bytecode generation.
- `lu-vm`: bytecode format, independent verifier, VM and VM heap/GC.
- `lu-native`: Cranelift x64 COFF generation and Windows GNU linker integration.
- `crates/runtime`: static native runtime source compiled by `crates/native/build.rs`.
- `crates/ion`: independent 0.5.0 Rust UI library and Windows GDI backend; separate from Lutetium language GUI built-ins.

Read `audit/ARCHITECTURE.md` and `docs/` before changing cross-crate interfaces. The VM must remain independent of compiler frontend crates; never run bytecode that failed `lu_vm::verify`.

## Checks

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo build --workspace
cargo test --workspace
cargo build --release
```

Before submitting a change, run the checks above and any focused tests. The checked-in full-workspace baseline is verified on Windows x86_64 GNU only; do not claim validation on other hosts. Do not change or delete tests solely to make checks pass. Native GUI tests use Win32 messages for deterministic event dispatch, not physical mouse automation. ION examples open interactive Windows windows and are not headless checks.

## Language changes

- Add one positive and one negative test for every language feature, normally in `crates/compiler/tests/end_to_end.rs`.
- Diagnostics use stable `E####` codes. Register new codes in `ERROR_CODES.json` and `crates/cli/src/explain.rs`, with tests.
- Bytecode format changes must bump `FORMAT_VERSION`, update `docs/BYTECODE_FORMAT.md`, verifier, VM, codegen and serialization tests together.
- Keep `docs/LANGUAGE_SPEC.md` limited to behavior that is implemented and tested.
- Builtin signatures and runtime implementation must agree; prefer tests that verify arity/types and execution.

For ION-specific validation, also run `cargo test -p lutetium-ion --examples`, `cargo build -p lutetium-ion --examples`, and `cargo check -p lutetium-ion --target x86_64-pc-windows-gnu --all-targets`.

## Pull requests

Use a focused branch such as `fix/compiler-...`, `feature/...`, or `docs/...`. Describe the issue, design choice, files changed, tests run and remaining limitations. Include screenshots only for actual GUI behavior (not mocks). Do not claim GUI support from linker subsystem flags or declarations alone.
