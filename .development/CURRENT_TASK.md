# CURRENT_TASK.md

Updated: 2026-11-12

## Task

**T2 - `for x in lst` iteration lowering** (TASKS.md priority HIGH) - STARTING.

Lower `for x in lst { ... }` (list iteration, distinct from the existing
`for i in a..b` range form) to checked index-based iteration over
`lst.get(i)` with runtime bounds errors kept sane, plus a list equality
builtin (`lst == lst` currently E3010).

## Acceptance

* `for x in lst` iterates in order, element variable is checked as the
  list's element type (widening rules consistent).
* Empty list iterates zero times; iterating over a non-list is E3001/E3010.
* List equality: `==`/`!=` on two lists with the same element type
  (bytecode kind + verifier + docs in same commit).
* e2e positive + negative tests, incl. GC stress while iterating (allocation
  inside the loop must not break the iterator).
* Full `cargo test --workspace` stays green (baseline 149 tests).

## Status

NOT STARTED (this file updated just before starting).

## Notes

* Parser already accepts `for x in expr block` — the `a..b` range special
  case lives in lowering/typecheck; `for x in lst` likely currently errors
  in the checker (verify with a scratch run before coding).
* Keep the native-backend E6002 gate in sync if list equality adds a new
  builtin kind.
