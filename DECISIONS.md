# DECISIONS.md

Architectural/design decisions worth remembering, newest first. A decision
here is binding for future work unless a new entry supersedes it.

## 2026-11-11 — 0.4 lists/builtins

* **D-11**: Builtins compile to a dedicated `BUILTIN` bytecode instruction
  (operands: `kind u8, dst u16, argc u8, args u16×argc`) instead of
  synthesizing call frames. This operand order is shared by codegen, the
  verifier, the VM, and the disassembler — **do not change it without bumping
  `FORMAT_VERSION`** and updating all four sites plus
  `docs/BYTECODE_FORMAT.md` in the same commit.
* **D-12**: Lists have reference semantics (heap `Obj::List`), a strict
  one-element-type rule, and negative indices read from the end. Out-of-range
  indexing and popping an empty list are runtime error E5009 with the kind
  name; read failures of `read_text` surface the same way.
* **D-13**: `abs`/`min`/`max` are overloaded: all-int arguments resolve to the
  Int variants so integer math stays integer; any float argument widens.
  `pow`/`sqrt` etc. remain float-only (see TASKS.md K1).
* **D-14**: The native backend **gates** features it cannot yet compile:
  `check_supported` rejects lists/builtins up front with E6002 rather than
  silently miscompiling. Lifting the gate (TASKS.md T5) requires root-map GC
  support in the native runtime first (RUNTIME_ARCHITECTURE §3).
* **D-15**: Type errors for list/builtin misuse use the existing diagnostic
  family (E300x/E500x); only genuinely new diagnostics get new `E####` codes,
  which must be registered in `ERROR_CODES.json` + `crates/cli/src/explain.rs`
  with a test (E3015 added for a bad builtin arity).
* **D-16**: The AST keeps spans on nodes (`args[i].span()`, `e.span()`); do
  not add a `TExpr::span()` unless typed-IR restructuring is explicitly
  undertaken.
* **D-17**: Workspace version is now 0.4.0, but the globally installed
  `lutetium.exe` is still 0.1.0. Rule: never overwrite the installed binary
  except via `scripts/install.ps1` after the release build and full test
  suite pass.
* **D-18**: CLI integration tests that write into the shared `build/`
  directory must serialize on the `BUILD_LOCK` mutex
  (`crates/cli/tests/cli.rs`); parallel test threads raced on build artifacts.
* **D-19**: GC tests assert post-collection invariants (reachability +
  bounded allocation), not raw allocation counts, which are
  implementation-detail-sensitive.
* **D-20**: `method_call_parses` replaced the obsolete 0.1 negative test
  `method_call_reports_clear_error` - method calls are now a supported
  feature, so "parser must reject them" is wrong.

## 0.4.1 era

* **D-21**: Maps are **record maps**: `CTy::R(Vec<(String, BcTy)>)` with
  per-key field types (sorted, deduped; a later duplicate literal key wins).
  The key set is fixed at literal construction; field access / `get` / `set`
  are checked against it at compile time (unknown key or added key → E3010;
  dynamic keys → E3010). No `==` on maps (E3010, consistent with lists); no
  `map` type annotation (consistent with lists). Nesting (list/map values)
  is rejected E3001; >127 entries is E3016 (BUILTIN argc is a u8).
* **D-22**: `CTy` is `Clone`-only (not `Copy`) since `R`/`L` own data;
  helper methods take `&self`. Do not reintroduce `Copy`.
* **D-23**: Parser recovery must guarantee forward progress: the module
  loop bumps one token when a failed statement's recovery made no progress
  (previously a stray `}` spun forever appending diagnostics).
* **D-24**: Workspace version is 0.4.1; the globally installed binary and
  `dist/lutetium-windows-x64.zip` were refreshed in the same session. Rule
  D-17 (never overwrite the install except via install.ps1 after release
  build + full suite) remains in force for every future bump.
* **D-25**: The native build's runtime staticlib is written next to the exe
  as `<stem>.runtime.a` (was shared `lutetium_runtime.a`). Root cause of
  intermittent test failures: 11 native-exe tests wrote and linked one
  shared archive concurrently — a truncated read could fail the link.
  Fixed by removing the shared file, not by locking.

## Earlier (0.1 era, still in force)

* **D-1**: Diagnostics have stable `E####` codes; every code is registered in
  `ERROR_CODES.json` and `explain.rs` and has at least one test.
* **D-2**: The VM never executes bytecode that failed `lu-vm::verify`; the VM
  crate depends on nothing from the compiler frontend.
* **D-3**: No MSVC: toolchain is pinned GNU (`rust-toolchain.toml` /
  `.cargo/config.toml`); new dependencies must not require MSVC.
* **D-4**: Bytecode format changes bump `FORMAT_VERSION` and update the
  verifier + `docs/BYTECODE_FORMAT.md` in the same commit.
* **D-5**: Every language feature gets ≥1 positive and ≥1 negative test in
  `crates/compiler/tests/end_to_end.rs` (or the owning crate); regressions
  get a test named after the bug.
* **D-6**: Never fake features: docs describe only the implemented subset;
  planned work lives in CHANGELOG/TASKS, never in LANGUAGE_SPEC.
* **D-7**: `lutetium build --target native` writes `build/<name>.exe`,
  `<name>.obj`, and `<name>.runtime.a` relative to the current directory
  (keep CLI tests using relative paths + BUILD_LOCK). The runtime archive
  name is per-exe so concurrent native builds in one directory never share
  a mutable file (D-25).
