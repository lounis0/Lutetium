# Contributing to Lutetium

Thanks for your interest. Lutetium is experimental. Check `audit/TEST_RESULTS.md` for the current 0.4.4 worktree's verified and blocked checks, and `audit/KNOWN_ISSUES.md` for current verified limitations before choosing work.

## Development environment

- Use the pinned Rust toolchain in `rust-toolchain.toml`; install only the pinned Windows GNU target and required rustup components.
- The workspace forces `x86_64-pc-windows-gnu` from `.cargo/config.toml`. A local `x86_64-pc-windows-msvc` target can still be useful for running host-independent Rust tests, but it does not verify MinGW linking or Win32 integration tests.
- Native linking depends on the bundled MinGW-w64 components from rustup. Do not introduce a Visual Studio/MSVC-only dependency.

## Architecture

- `lu-syntax`: lexer, parser and AST.
- `lu-diagnostics`: spans and diagnostics.
- `lu-compiler`: resolver, type checker, typed register IR, lowering, optimizer and bytecode generation.
- `lu-vm`: bytecode format, independent verifier, VM and VM heap/GC.
- `lu-native`: Cranelift x64 COFF generation and Windows GNU linker integration.
- `crates/runtime`: static native runtime source compiled by `crates/native/build.rs`; its manifest is metadata and the package is intentionally outside the workspace.
- `crates/ion`: independently versioned Rust UI library and Windows Direct2D/DirectWrite backend with GDI fallback; separate from Lutetium language GUI built-ins.

Read [`docs/AGENT_GUIDE.md`](docs/AGENT_GUIDE.md), current `audit/KNOWN_ISSUES.md`, and `audit/TEST_RESULTS.md` before changes. Dated `.development/` notes and historical `audit/` snapshots may be stale; do not use their prior task/test claims as current. Read [`docs/LTM_INTEGRATION.md`](docs/LTM_INTEGRATION.md) before proposing `.LTM` work; it is design-only. The VM must remain independent of compiler frontend crates; never run bytecode that failed `lu_vm::verify`.

## Checks

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo build --workspace
cargo test --workspace
cargo build --release
```

Before submitting a change, run the checks above and any focused tests. The target is pinned to Windows x86_64 GNU. The current worktree's `cargo check` and focused tests have passed, but a full workspace test is blocked in this environment when Cargo fails to spawn the pinned `dlltool`; see `audit/TEST_RESULTS.md` before claiming validation. Do not treat a filtered or focused test run as a full-workspace result, and do not change or delete tests solely to make checks pass. Native GUI tests use Win32 messages for deterministic event dispatch, not physical mouse automation. ION examples open interactive Windows windows and are not headless checks.

## Language changes

- Add one positive and one negative test for every language feature, normally in `crates/compiler/tests/end_to_end.rs`.
- Diagnostics use stable `E####` codes. Register new codes in `ERROR_CODES.json` and `crates/cli/src/explain.rs`, with tests.
- Bytecode format changes must bump `FORMAT_VERSION`, update `docs/BYTECODE_FORMAT.md`, verifier, VM, codegen and serialization tests together.
- Keep `docs/LANGUAGE_SPEC.md` limited to behavior that is implemented and tested.
- Builtin signatures and runtime implementation must agree; prefer tests that verify arity/types and execution.
- The project manifest is intentionally not a full TOML implementation. Keep its accepted `name`, `entry`, and `kind` fields strict and test project-relative paths against traversal and symlink escapes.

For ION-specific validation, also run `cargo test -p lutetium-ion --examples`, `cargo build -p lutetium-ion --examples`, and `cargo check -p lutetium-ion --target x86_64-pc-windows-gnu --all-targets`.

## Repeatable correctness benchmark

Run `scripts/benchmark_agent.ps1 -Repeat 3` after `cargo build -p lutetium-cli`. This checks fixed VM outputs, GC-stress stats, JSON diagnostics, and that unsupported maps are rejected by the native backend. It reports elapsed smoke-test time but is not a performance or agent-quality benchmark. The current worktree's recorded one-iteration run passed all five cases; see `audit/TEST_RESULTS.md`.

## Pull requests

Use a focused branch such as `fix/compiler-...`, `feature/...`, or `docs/...`. Describe the issue, design choice, files changed, tests run and remaining limitations. Include screenshots only for actual GUI behavior (not mocks). Do not claim GUI support from linker subsystem flags or declarations alone.
