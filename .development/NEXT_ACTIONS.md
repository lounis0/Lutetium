# NEXT_ACTIONS.md

Ordered queue. Work top-down; when a task completes, move it to
COMPLETED_TASKS.md, update TASKS.md status, and shift the rest up.

1. ~~T1~~ DONE 2026-11-12: record maps `{name: "x"}` + `.field` access +
   `len/get/set`, per-key typed fields, bytecode v3 (Map tag, map_new/get/set
   kinds), E3016 registered, parser recovery bug fixed, 22 new tests,
   examples/maps.lu, docs synced, version 0.4.1 installed + dist zip rebuilt.
2. **T2** - `for x in lst` iteration lowering; list equality builtin; e2e
   tests incl. GC stress while iterating.
3. **T3** — CLI test asserting concrete `--gc-stats` counters (e.g.
   minor_collections > 0 under --gc-stress); document --gc-stats on error path.
4. **T4** — `join`, list slicing `lst[1..3]`, list equality builtin (if not
   landed with T2).
5. **T5** — Native backend list/builtin support (root-map GC per
   RUNTIME_ARCHITECTURE §3) or keep gate + document explicitly.
6. **T6** — `import` + module namespaces (math.sqrt); circular-import
   diagnostic.
7. **T7** — Optimizer level 3: size-bounded non-recursive inlining;
   differential tests.
8. **T8** — `lutetium fmt` deterministic formatter; idempotency tests.
9. **T9** — `lutetium test` runner (`test "name" { ... }`).
10. **T10** — `lutetium init/clean/bench`.

After each task: `cargo build --workspace && cargo test --workspace`,
update TASKS.md/CURRENT_STATE.md/CHANGELOG.md as needed, and append to
`.development/SESSION_LOG.md`.
