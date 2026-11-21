# Historical Lutetium repository inventory

> **Historical snapshot, superseded.** This inventory predates Lutetium 0.4.3/0.4.4 updates and must not be used as current file inventory, scope, or test status. Current scope is in [`../README.md`](../README.md), [`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md), [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md), and [`TEST_RESULTS.md`](TEST_RESULTS.md).

**Original snapshot:** repository inspected 2026-11-12; release-preparation updates performed afterward.

## Root files

| Path | Role and current note |
|---|---|
| `Cargo.toml` | At the 0.4.2 snapshot: six crates, version 0.4.2, Rust 2021; release uses thin LTO, one codegen unit, strip. Current workspace now includes ION and is aligned to 0.4.4; see current README and test results. |
| `Cargo.lock` | Locked dependency graph; preserve for reproducible builds. |
| `rust-toolchain.toml` | Pins stable Windows GNU toolchain. |
| `.cargo/config.toml` | Forces `x86_64-pc-windows-gnu`; real platform limitation. |
| `.gitignore` | Rust, Windows, IDE, generated outputs and local environment ignore rules. |
| `README.md` | Refreshed landing page: real supported subset, getting started, commands, known failure status, audit links. |
| `LICENSE` | MIT license with neutral “Lutetium contributors” attribution; no individual author attribution was discoverable in existing project files. |
| `CONTRIBUTING.md` | Build, architecture, test and language-change contribution guidance. |
| `SECURITY.md` | Experimental/untrusted-code warnings and responsible reporting guidance; no private contact invented. |
| `CHANGELOG.md` | Current package version and selected historical development facts; explicitly points to audit for actual status. |
| `ERROR_CODES.json` | Machine-readable diagnostic metadata; current worktree language version aligned to 0.4.4. |
| `DECISIONS.md` | Useful historical implementation/design decisions; retained. |
| `DEVELOPMENT_AUDIT.md`, `DEVELOPMENT_REPORT.md` | Superseded milestone reports; removed after their useful facts were captured in `audit/`. |
| `CURRENT_STATE.md` | Superseded status snapshot with stale green-test claims; removed. |
| `ENVIRONMENT_REPORT.md` | Old machine-specific inventory, not necessary for public project operation; removed. |
| `ROADMAP.md`, `TASKS.md` | Historical task plans contradicted by newer source; consolidated into `audit/ROADMAP.md` and removed. |
| `AGENTS.md` | Old agent instructions with version/test assertions that are stale. Removed to avoid conflicting repository guidance; conventions now in CONTRIBUTING. |
| `gc_test_out.txt` | Unclassified generated log; removed. |
| `FINAL_REPOSITORY_REPORT.md` | Final local preparation, test and publication status report. |

## Directories

| Directory | Purpose / state |
|---|---|
| `crates/diagnostics` | Spans, diagnostics, terminal/human renderer, JSON diagnostics (`lu-diagnostics`). |
| `crates/syntax` | Lexer, parser, AST (`lu-syntax`). |
| `crates/compiler` | Resolver, typechecker, typed IR, lowering, optimizer, bytecode codegen (`lu-compiler`). |
| `crates/vm` | LTBC format, verifier, register VM, collections and GC (`lu-vm`). |
| `crates/native` | Cranelift backend, native build facade and Windows GNU linker. |
| `crates/runtime` | Runtime source compiled by native `build.rs` using a relative path; must remain here unless build script changes in same tested change. |
| `examples/` | Lutetium programs, concise syntax, Rust ION examples, and the legacy language GUI example. Scratch-named `.lu` files are retained where they contain meaningful iteration/equality/diagnostic regression cases. |
| `docs/` | Existing language, grammar, architecture, bytecode, runtime and builtin docs. |
| `audit/` | Exactly the six requested verified audit/roadmap files. |
| `scripts/` | Windows installer, uninstaller and distribution builder. |
| `.development/` | Historical task ledger and session notes; retained because it records useful implementation history, but not authoritative current status. |
| `.github/workflows/` | CI workflow for configured Windows GNU target. No release workflow added because current build is known broken and a real release artifact cannot be validated. |
| `dist/` | Generated distribution directory and zip removed; scripts can reproduce them after a healthy build. |
| `target/` | Cargo-generated local build output; excluded from Git. |

## Workspace crates and source responsibilities

### `lu-diagnostics` — `crates/diagnostics`

- `src/lib.rs`: `Span`, `Severity`, `Diagnostic`, `SourceFile` line index, human diagnostic renderer, JSON escaping and `diagnostics_json`.
- `Cargo.toml`: package metadata only; no dependencies.
- Tests: none in this crate.

### `lu-syntax` — `crates/syntax`

- `src/lexer.rs`: token enum/data, keywords, lexical scanning, source spans, E1001/E1101; lexer unit tests.
- `src/parser.rs`: recursive-descent parser, precedence, recovery, syntax AST construction; parser unit tests.
- `src/ast.rs`: owned module/function/block/statement/expression/type nodes with spans.
- `src/lib.rs`: module declarations and re-exports.
- `Cargo.toml`: `lu-diagnostics` path dependency.

### `lu-compiler` — `crates/compiler`

- `src/resolve.rs`: function table, duplicate/main checks, annotation conversion.
- `src/typecheck.rs`: type lattice/checker, scoped locals/slots, parameter/return inference, casts, builtin typing, typed AST; currently contains incomplete GUI-specific branches.
- `src/ir.rs`: typed register-based IR and disassembler; includes partial GUI types/instruction.
- `src/lower.rs`: typed AST to IR, basic blocks/loops/short circuit/collection and builtin lowering; includes partial GUI lowering.
- `src/optimizer.rs`: constant folds, unreachable blocks, dead temps, block-local CSE; level 3 same as 2.
- `src/codegen.rs`: IR to bytecode, interning/patching; GUI button path intentionally `unreachable!` and therefore unsafe for GUI source to call.
- `src/pipeline.rs`: lex→parse→resolve→check→lower→opt and public compile entrypoints.
- `src/lib.rs`: public module and API re-exports.
- `tests/end_to_end.rs`: compile/verify/run tests for language and VM behavior. Current observed: 113 passed, 2 failed.
- `Cargo.toml`: path deps to diagnostics, syntax, VM.

### `lu-vm` — `crates/vm`

- `src/bytecode.rs`: LTBC version 5 constants, type tags, opcodes, builtins, serialization/deserialization/disassembly. Docs currently lag version.
- `src/verify.rs`: independent bytecode operand/control-flow/register verifier.
- `src/vm.rs`: register-frame execution, builtins, errors, output, GUI calls explicitly unavailable.
- `src/heap.rs`: generational collector for List/Map with remembered set and stats; 8 unit tests passed.
- `src/lib.rs`: exports public runtime APIs.
- `Cargo.toml`: no dependencies.

### `lu-native` — `crates/native`

- `build.rs`: compiles `../runtime/src/lib.rs` with plain `rustc` staticlib and sets embedded artifact environment path. This relative reference makes `crates/runtime` location significant.
- `src/backend.rs`: Cranelift x64 scalar IR translation, runtime imports, COFF output, unsupported-feature gate; current missing `GuiButton` instruction arm blocks compilation.
- `src/linker.rs`: locates rustup's Windows GNU MinGW driver and system libraries; console/GUI PE subsystem switch.
- `src/lib.rs`: target ISA, object build, console and GUI-subsystem executable facade. GUI helper is not connected to a functioning frontend/runtime/CLI path.
- `tests/native_exe.rs`: 11 generated console-executable integration tests, presently prevented from compiling by native match error.
- `tests/scratch_ir.rs`: inert comment-only placeholder at the original snapshot; removed in the 2026-11-20 cleanup.
- `Cargo.toml`: mandatory Cranelift crates and target-lexicon, compiler path dependency; VM dev dependency.

### `lu-runtime` — `crates/runtime` (outside workspace)

- `src/lib.rs`: scalar output/string/panic ABI plus unused Win32 GUI function declarations/constants/types. No actual UI window or rendering code.
- `Cargo.toml`: staticlib metadata; actual build via native build script, not normal workspace membership.

### `lutetium-cli` — `crates/cli`

- `src/lib.rs`: command parsing/implementation for run/build/check/inspect/explain; no GUI target.
- `src/main.rs`: thin process wrapper.
- `src/explain.rs`: hand-written E-code explanations, some stale.
- `tests/cli.rs`: 12 integration tests, currently cannot compile due native crate blocker.
- `Cargo.toml`: path dependencies to all crates including native.

## Tests and examples

Test surfaces: syntax lexer/parser units (24 passed); VM heap units (8 passed); compiler e2e (115 attempted, 113 pass); native exe tests (11 in source, build-blocked); native unit tests (2 in source, build-blocked); CLI tests (12 in source, build-blocked). No GUI tests exist.

Examples include scalar/functions/strings/maps, concise syntax, legacy language GUI, and separate Rust ION apps. Scratch-named compiler fixtures remain where they provide regression coverage; generated outputs are ignored.

## External packages and build constraints

Local path crates form most dependencies. External direct dependencies: Cranelift codegen/frontend/module/object 0.120 (locked to 0.120.2) and `target-lexicon` 0.13. Native build requires the pinned target and rustup bundled MinGW-w64; not portable to other OS/architectures without separate backend work. Cargo.lock is preserved.
