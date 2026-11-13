# Lutetium

Lutetium is an experimental, statically typed, general-purpose programming language implemented in Rust. It provides a source compiler, a verified register-based bytecode VM, and a Windows x86_64 GNU native backend for scalar programs.

> **Status: experimental / not a first public release.** The audited checkout is version 0.4.1. At the time of this README refresh, the workspace build is blocked by incomplete GUI IR integration, and two compiler end-to-end tests fail. GUI support is scaffolding only; Lutetium cannot currently build working GUI applications. See [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md) and [`audit/KNOWN_ISSUES.md`](audit/KNOWN_ISSUES.md) for observed results.

## What is implemented

- Lexer, recursive-descent parser, AST, resolver and static type checker.
- Typed register IR with basic blocks and control-flow terminators.
- Versioned `.lbc` bytecode, verifier, and register-machine VM.
- Scalars, functions, conditionals, loops, lists, record-like maps, string/math/filesystem builtins in the bytecode VM.
- Precise generational tracing collection for VM list/map objects.
- IR optimizer (levels 0–2; level 3 currently behaves like level 2).
- Cranelift native code generation and MinGW linking for Windows x86_64 GNU scalar console programs. The native backend rejects VM-only collections/builtins; the current source tree also has a compile failure in the incomplete GUI IR path.

Features such as imports, closures, structs/enums, a cross-platform native backend, and a functioning GUI framework are not implemented. The native backend and compiler are experimental and should not be treated as production-ready.

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
- Target `x86_64-pc-windows-gnu` and the bundled rustup MinGW-w64 components for native executable linking.
- No MSVC or Visual Studio requirement in the configured toolchain.

The workspace target is forced in `.cargo/config.toml`, so this repository is currently configured for Windows x86_64 GNU rather than host-generic builds.

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo build --workspace
cargo test --workspace
cargo build --release
```

**Current known build status:** `cargo check/build/test --workspace` fails because `IrInstr::GuiButton` is not handled in the native backend. After addressing that blocker, `cargo test -p lu-compiler` currently reports 113 passed and 2 failed. Do not treat old test totals elsewhere in repository history as current results.

## CLI

When built, the CLI source defines:

```text
lutetium run <file.lu> [--gc-stats] [--gc-stress]
lutetium build <file.lu> [--target bytecode|native] [--opt 0-3] [--release]
lutetium check <file.lu> [--diagnostics json]
lutetium inspect <file.lbc> | <file.lu> --ir [--optimized]
lutetium explain <E-code>
```

`run` compiles to bytecode, verifies it, then executes it in the VM. Bytecode build verifies before writing `.lbc`. Native build is Windows-only and the current whole workspace must compile first.

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
