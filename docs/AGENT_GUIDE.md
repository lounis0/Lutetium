# Lutetium developer and AI-agent guide

This guide summarizes source-of-truth boundaries and a safe, repeatable workflow for contributors and coding agents. It documents the current tree; it does not promise that every toolchain-specific integration suite is runnable in every environment.

## Start here

1. Read [`../README.md`](../README.md), [`ARCHITECTURE.md`](ARCHITECTURE.md), [`LANGUAGE_SPEC.md`](LANGUAGE_SPEC.md), and [`STANDARD_LIBRARY.md`](STANDARD_LIBRARY.md).
2. Read [`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md) and [`RUNTIME_ARCHITECTURE.md`](RUNTIME_ARCHITECTURE.md) before changing VM, serialization, GC, or native ABI.
3. For Lutetium ION, read [`ION_0.5.md`](ION_0.5.md). ION is a Rust crate, not the Lutetium `gui_*` API.
4. Check [`../audit/KNOWN_ISSUES.md`](../audit/KNOWN_ISSUES.md) and the current evidence in [`../audit/TEST_RESULTS.md`](../audit/TEST_RESULTS.md). The dated `.development/` logs and audit history may be stale; do not treat old task notes or pass counts as current test results.

## Architecture boundaries

- `crates/syntax`: lexer, parser, and AST. No runtime or backend policy.
- `crates/diagnostics`: spans, renderers, and JSON diagnostics shared by crates.
- `crates/compiler`: name resolution, type checking, typed IR, lowering, optimization, and bytecode code generation.
- `crates/vm`: LTBC format, independent bytecode verifier, register VM, and traced heap. Keep this crate independent of the frontend; verify before execution.
- `crates/native`: Cranelift object code and Windows x64 GNU linking. Unsupported IR must be rejected, not partially compiled.
- `crates/runtime`: standalone Rust static runtime source embedded by `crates/native/build.rs`; its manifest is documentation/tooling metadata and it is deliberately not a workspace member.
- `crates/cli`: CLI argument handling, project manifest, scaffolding, builds, diagnostics, and dispatch. Project entry paths are canonicalized for containment checks and normalized before being passed to the bundled Windows GNU linker.
- `crates/ion`: independent Rust native Windows UI foundation. Direct2D/DirectWrite is the preferred backend when available, with GDI fallback. Do not conflate this library with the language-level Win32 GUI prototype.

Imports/modules, `.LTM`, tensor/agent execution, sandboxing, accessibility APIs, and a package manager are not implemented. Do not add placeholder runtime behavior or describe roadmap notes as shipped features.

## Change discipline

- Preserve existing working-tree edits and inspect `git status` before changing or removing files.
- Keep changes narrow; do not delete historical audit snapshots, migration notes, or reproducible test fixtures without checking references and history.
- Every language behavior change needs positive and negative tests. A bytecode encoding change must bump the format version and update codegen, verifier, VM, serialization tests, and [`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md) together.
- Keep error codes stable and update both [`../ERROR_CODES.json`](../ERROR_CODES.json) and `crates/cli/src/explain.rs` when adding one.
- Project manifests support only `name`, `entry`, and `kind`, and do not create multi-file compilation. Preserve traversal/symlink checks.
- Never claim Windows GNU linking, Win32 behavior, or visual interaction was tested based only on `cargo check`, MSVC tests, a GUI-subsystem flag, or a source inspection.
- Do not commit, tag, publish, install, or push unless the user explicitly authorizes it.

## Validation

The repository pins stable `x86_64-pc-windows-gnu`. Run checks from the repository root, preferably in this order:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test -p lu-vm --offline
cargo test -p lu-compiler --offline
cargo test -p lutetium-cli --offline --test cli
cargo test --workspace --offline
```

Run targeted native executable, legacy GUI, and ION native-window tests on an appropriate Windows GNU host with the linker helpers working. If a command fails before compiling or launching the test binary, record it as blocked, not failed behavior or a pass. Report the exact command, target, and environmental error. Current known local limitation: workspace test dependency builds may fail when Cargo's `windows-*` dependency build scripts cannot create a process for the pinned `dlltool.exe`; focused GNU native executable and GUI integration tests have run successfully in this environment, but that does not prove the full workspace suite.

## Repeatable correctness benchmark

Use [`../scripts/benchmark_agent.ps1`](../scripts/benchmark_agent.ps1) to validate a fixed set of language, diagnostics, and native target contracts. It checks exact output/exit status, records tool versions and run durations, and optionally repeats the pass. It is a correctness/smoke benchmark—not a performance benchmark, AI-quality leaderboard, or proof of full-workspace/visual GUI coverage. See [`../audit/TEST_RESULTS.md`](../audit/TEST_RESULTS.md) for results actually run in the current worktree.
