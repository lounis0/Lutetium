# CURRENT_TASK.md

Updated: 2026-11-20

> Historical task record. T2 was completed before the current 0.4.4 work; the active status is in `../audit/TEST_RESULTS.md` and `../CHANGELOG.md`.

## Task

**T2 - `for x in lst` iteration lowering** - COMPLETED (existing source/tests).

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
* Full `cargo test --workspace` passes before marking the task complete; current environment does not establish this due the linker tool process-launch blocker.

## Status

The checked-in compiler already implements list iteration and deep list equality, including GC-stress coverage and negative type tests. The compiler suite passes 116 tests in the current 0.4.4 worktree. The full workspace test has not been verified due the pinned GNU `dlltool` launch issue; see `../audit/TEST_RESULTS.md`.

## Notes

* Historical implementation notes retained for context only; inspect current compiler tests before acting on this area again.
