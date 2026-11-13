# SESSION_LOG.md

## 2026-11-11 — session (0.4 lists/builtins completion + dev infra)

* Resumed from interrupted work; ~80 compile errors in lu-compiler fixed by
  completing the migration (not rolling back — no git repo).
* Implemented C1–C11 (see COMPLETED_TASKS.md); 117 tests green; release build
  clean; fmt clean.
* E3015 added and registered. Bytecode FORMAT_VERSION bumped to 2 with BUILTIN
  opcode documented.
* Wrote persistent development files per directive §2.1: ROADMAP.md, TASKS.md,
  CURRENT_STATE.md, DECISIONS.md, and `.development/*` (this set).
* Next: T11 install re-check, then T1 maps.

## 2026-11-12 - session (0.4.1 maps)

* T1 implemented fully: record maps with per-key field types (`CTy::R`),
  `{...}` literals, `.field`, `len/get/set`; bytecode format v3 (Map tag 7,
  MapNew/MapGet/MapSet kinds); E3016 registered; native gate extended.
* `CTy` changed from `Copy` to `Clone`-only; ~52 mechanical move-out fixes
  across typecheck.rs/lower.rs; all 117 pre-existing tests passed unchanged.
* Parser recovery bug found and fixed (stray `}` at module level = infinite
  diagnostic growth); named-test coverage via the new negative map tests.
* 149 tests green; fmt clean; release build clean; workspace version 0.4.1;
  install.ps1 re-run (global binary verified 0.4.1); make_dist.ps1 re-run.
* Fixed the intermittent test-suite failure at its root: `build_exe` wrote a
  shared `lutetium_runtime.a` that 11 native-exe tests wrote/linked
  concurrently; the runtime archive is now per-exe (`<stem>.runtime.a`).
  5x native suite + 5x CLI suite + 2x full workspace runs all green.
  Decision D-25 recorded (D-7 layout updated).
* K3 fully cleared: `crates/compiler/tests/scratch_dump.rs` and
  `crates/native/tests/scratch_ir.rs` both deleted.
* Next: T2 (`for x in lst` + list equality builtin).
