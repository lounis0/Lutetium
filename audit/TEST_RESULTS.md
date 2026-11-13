# Test results — current repository preparation

Audit results were collected against the source snapshot before repository-cleanup edits. Cleanup changed documentation, file locations for scratch examples, and removed generated binaries; no compiler, VM, runtime or test source was changed during cleanup. The earlier audit also added a GC stress helper before the audit-only task began; it remains in `crates/compiler/tests/end_to_end.rs`.

## Environment and tool availability

- OS/toolchain is expected Windows GNU from `.cargo/config.toml` and `rust-toolchain.toml`.
- `git rev-parse --is-inside-work-tree` returned `fatal: not a git repository` before initialization.
- Several shell probes (`rustc --version`, `cargo --version`, `lutetium --version`, `gh --version`) were rejected by the command reviewer with “too much or invalid context for automatic review”; they were not verified through the shell here. `.cargo` and toolchain configuration are inspected directly.
- No `lutetium.exe` is currently tracked/provided as a root executable; existing distribution executable was removed as generated artifact. `lutetium --version` could not be verified.

## Build and test commands

| Command | Observed result |
|---|---|
| `cargo fmt --all -- --check` | Failed. Rustfmt reported formatting differences in GUI scaffolding/compiler tests and some source files. Formatter was not run in write mode. |
| `cargo check --workspace` | Failed compiling `lu-native`; E0004 missing match arm for `IrInstr::GuiButton` in `crates/native/src/backend.rs:349`. |
| `cargo build --workspace` | Failed with the same E0004 native backend error. |
| `cargo test --workspace` | Failed during native crate compilation with the same E0004; full workspace tests did not run. |
| `cargo test -p lu-syntax` | Passed: 24 passed, 0 failed; doc-tests 0. |
| `cargo test -p lu-diagnostics` | Passed: 0 tests; doc-tests 0. |
| `cargo test -p lu-vm` | Passed: 8 passed, 0 failed (heap/GC unit tests); doc-tests 0. |
| `cargo test -p lu-compiler` | Failed: 113 passed, 2 failed out of 115 compiler e2e tests. |
| `cargo test -p lu-native --no-run` | Failed with E0004; native test binaries could not compile. |
| `cargo test -p lutetium-cli` | Failed with native compile error; CLI tests did not run. |
| `cargo build --release` | Not completed in this execution; workspace native compilation blocker applies. |

### Compiler test failures

1. `for_in_iterable_evaluated_once`: compilation failed with `E2001: expected '{', found identifier` on a range/list loop iterable that is a named function call (`mk()`).
2. `list_equality_bool_str_elements`: program uses heterogeneous list `[true, "a"]`; checker reports `E3001` twice because lists enforce homogeneous element types except int-to-float widening. Existing test expectation and current language contract disagree.

## Coverage facts from test source

- Syntax: lexer/parser tests for literals/operators/comments/diagnostics, control-flow, lists/maps, field/method/index grammar.
- Compiler e2e: language positive/negative tests, diagnostics, functions/control flow, collections, builtin execution, verifier/container serialization, GC stress, optimizer equivalence.
- VM: 8 GC heap tests; VM opcodes/runtime are primarily exercised via compiler e2e.
- Native: 11 console executable integration tests and 2 unit tests exist, but cannot compile now.
- CLI: 12 integration tests exist, but cannot compile now.
- No GUI-specific tests, apps or executable-launch tests exist.

Do not use previous `CURRENT_STATE.md` claims of 149 green tests as the current result. Those documents were historical and contradicted by these directly observed runs.
