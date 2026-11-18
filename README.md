# Lutetium

Lutetium 0.4.2 is an experimental, statically typed programming language implemented in Rust. It provides a source compiler, verified register-based bytecode VM, a Windows x86_64 GNU native backend, and a minimal experimental ION GUI target. A native Win32 button click dispatches to a named Lutetium handler.

> **Status: experimental.** This is a minimal Windows-only GUI prototype, not a general GUI framework. The verified ION slice is limited to a window, native button, named callback, and callback-driven window close; rendering/layout/state and other controls are not supported by this claim.

## What is implemented

- Lexer, recursive-descent parser, AST, resolver and static type checker.
- Typed register IR with basic blocks and control-flow terminators.
- Versioned `.lbc` bytecode, verifier, and register-machine VM.
- Scalars, functions, conditionals, loops, lists, record-like maps, string/math/filesystem builtins in the bytecode VM.
- Precise generational tracing collection for VM list/map objects.
- IR optimizer (levels 0–2; level 3 currently behaves like level 2).
- Cranelift native code generation and MinGW linking for Windows x86_64 GNU scalar console programs.
- Experimental ION native GUI: Win32 top-level window, native button, `WM_COMMAND` click dispatch to a named Lutetium function, and callback-driven close/shutdown. The deterministic Windows integration test uses `BM_CLICK` on the native button; it does not automate physical mouse input.

Features such as imports, closures, structs/enums, a cross-platform native backend, GUI rendering beyond standard controls, layout, reactive state, and keyboard input are not implemented. The native backend and compiler are experimental and should not be treated as production-ready.

## Example

```lu
fn fibonacci(n: int) -> int {
    if n <= 1 {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    print(fibonacci(10))
}
```

Examples in `examples/` and the tests are the syntax authority. For the full implemented subset, see [`docs/LANGUAGE_SPEC.md`](docs/LANGUAGE_SPEC.md).

## Build prerequisites

- Rust stable toolchain pinned by `rust-toolchain.toml`.
- For native and GUI executable builds: Windows x86_64 GNU target and the rustup-bundled MinGW-w64 linker components.
- No MSVC or Visual Studio requirement in the configured toolchain. Bytecode/VM and native linking have only been verified in the configured Windows environment; no other host/target support is claimed.

The workspace target is forced in `.cargo/config.toml`, so this repository is currently configured for Windows x86_64 GNU rather than host-generic builds.

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo build --workspace
cargo test --workspace
cargo build --release
```

**Current verification:** Windows x86_64 GNU formatting, workspace checks/builds, release build, and workspace tests are recorded in [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md). The GUI integration test builds and launches a native executable, sends `BM_CLICK` to its actual Win32 button, dispatches the Lutetium callback, and observes clean shutdown. This does not verify automated physical mouse input.

## CLI

When built, the CLI source defines:

```text
lutetium run <file.lu> [--gc-stats] [--gc-stress]
lutetium build <file.lu> [--target bytecode|native|gui] [--opt 0-3] [--release]
lutetium check <file.lu> [--diagnostics json]
lutetium inspect <file.lbc> | <file.lu> --ir [--optimized]
lutetium explain <E-code>
```

`run` compiles to bytecode, verifies it, then executes it in the VM. Bytecode build verifies before writing `.lbc`. Native and GUI builds target Windows x86_64 GNU; GUI programs run only as generated executables (the bytecode VM reports GUI builtins unavailable). See [`examples/ion_button.lu`](examples/ion_button.lu) for the minimal ION sample; build it with `lutetium build examples/ion_button.lu --target gui`.

## Repository map

```text
crates/diagnostics  spans and human/JSON diagnostics
crates/syntax       lexer, parser and AST
crates/compiler     resolver, typechecker, typed IR, lowering, optimizer, bytecode codegen
crates/vm           LTBC format, verifier, register VM and GC heap
crates/native       Cranelift backend and Windows GNU linker
crates/runtime      static native runtime source compiled by crates/native/build.rs
examples/           Lutetium programs
scripts/            Windows install, uninstall and portable distribution scripts
docs/               language, bytecode and runtime documentation
audit/              current technical audit and work roadmap
```

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). Preserve the Windows GNU build configuration, verify bytecode before execution, add positive and negative tests for language behavior, and keep documentation aligned with actual implementation.

## Security

See [`SECURITY.md`](SECURITY.md) for responsible reporting. Do not include secrets or personal data in issues.

## License

MIT. See [`LICENSE`](LICENSE).
