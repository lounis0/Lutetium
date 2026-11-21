# NEXT_ACTIONS.md

> Historical development queue retained for context. Its entries were written in November 2026 before the 0.4.3/0.4.4 changes and are not a current project roadmap. Consult `../README.md`, `../audit/KNOWN_ISSUES.md`, and `../audit/TEST_RESULTS.md` for current scope and verified status before selecting work.

This queue is preserved verbatim except for being explicitly marked historical; do not treat any prior completion claim or release action below as current authorization.

1. ~~T1~~ DONE 2026-11-12: record maps `{name: "x"}` + `.field` access +
   `len/get/set`, per-key typed fields, bytecode v3 (Map tag, map_new/get/set
   kinds), E3016 registered, parser recovery bug fixed, 22 new tests,
   examples/maps.lu, docs synced, version 0.4.1 installed + dist zip rebuilt.
2. **T2** - `for x in lst` iteration lowering; list equality builtin; e2e
   tests incl. GC stress while iterating. (Implemented; check current source/tests.)
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

All entries require source inspection and user-approved scope before implementation. Historical release/install wording is not an instruction to install, package, commit, or publish anything.