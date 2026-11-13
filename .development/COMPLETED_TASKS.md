# COMPLETED_TASKS.md

Append-only ledger of finished tasks. Move entries here from CURRENT_TASK.md /
NEXT_ACTIONS.md when they pass their acceptance criteria and the full suite.

## 2026-11-11 session (0.4.0)

* **C1** Restore compiling tree: finished the interrupted lists/builtins
  migration (CTy::L, BuiltinKind, Ty::List + IrInstr::Builtin in IR, lowering,
  codegen, optimizer, native gate). VERIFIED.
* **C2** Parser: list literals in expression position. VERIFIED.
* **C3** Checker fixes: push/set element-type params, list element unify
  binding, str_split verifier signature (2 args). VERIFIED.
* **C4** Overload resolution for abs/min/max (int vs float). VERIFIED.
* **C5** Native backend feature gate for lists/builtins (E6002). VERIFIED.
* **C6** GC diagnostics: --gc-stats/--gc-stress in CLI, GcStats.freed_bytes.
  VERIFIED.
* **C7** E3015 registered (ERROR_CODES.json + explain.rs). VERIFIED.
* **C8** GC test invariant fix; CLI build-test race fixed with BUILD_LOCK.
  VERIFIED.
* **C9** Docs: LANGUAGE_SPEC 0.4, BYTECODE_FORMAT v2, GRAMMAR.ebnf,
  RUNTIME_ARCHITECTURE native gate, new STANDARD_LIBRARY.md. VERIFIED.
* **C10** 19 new end-to-end tests + CLI gc-stats test. VERIFIED.
* **C11** Workspace version 0.4.0 + CHANGELOG entry. VERIFIED (release build).
* Created persistent development files: ROADMAP.md, TASKS.md,
  CURRENT_STATE.md, DECISIONS.md, `.development/` ledger. VERIFIED.

## 2026-11-12 session (0.4.1)

* **T11** Release engineering: install.ps1 ran (global binary 0.4.1),
  make_dist.ps1 produced `dist/lutetium-windows-x64.zip`, installed-example
  run verified. VERIFIED.
* **T1** Maps: record literals `{name: "Ada"}` + field access `.field`
  (parser: expression-position `{` is always a map literal; E2005 field
  rejection removed). Checker: `CTy::R(Vec<(String, BcTy)>)` per-key typed
  record maps (sorted, deduped; later duplicate key wins); `CTy` became
  `Clone`-only (~52 mechanical fixes; all 117 pre-existing tests pass).
  Lowering/codegen/IR: `Ty::Map`, `TExpr::Map/Field`. Bytecode v3: `BcTy::Map`
  tag 7, builtin kinds `MapNew` 0x40 (even argc, verifier-checked),
  `MapGet` 0x41, `MapSet` 0x42; runtime dedup + GC collect trigger.
  Methods `len/get/set` (get/set keys must be string literals; new keys →
  E3010). E3016 (literal > 127 entries) registered in ERROR_CODES.json +
  explain.rs. Native gate rejects List|Map. Parser recovery bug fixed: stray
  `}` at module level forced one-token progress (found by new negative
  tests). 22 new tests (7 parser, 15 e2e incl. GC stress + optimizer
  equivalence). examples/maps.lu. Docs: BYTECODE_FORMAT v3, LANGUAGE_SPEC
  (Maps section), GRAMMAR.ebnf, STANDARD_LIBRARY (map methods), CHANGELOG
  0.4.1. VERIFIED.

## Totals after this session

149 tests passing (94 e2e, 22 parser, 8 GC, 2 native units,
11 native-exe, 12 CLI); `cargo build --workspace`, `cargo build --release`,
and `cargo fmt --all -- --check` all clean. Workspace + installed binary +
dist zip all 0.4.1.
