# Historical prioritized roadmap

> **Historical snapshot, superseded.** P0 items below were written before Git provenance and the Windows ION slice were restored. The compiler/native/GUI path now builds and the workspace tests pass on Windows x86_64 GNU. Current remaining issues are in [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) and current test evidence in [`TEST_RESULTS.md`](TEST_RESULTS.md). Do not treat the old P0 blockers below as current status.

This roadmap records concrete gaps identified in the earlier snapshot. Each future task should land with positive/negative tests and synchronized docs; do not treat a roadmap item as shipped until acceptance tests pass.

## Completed since this snapshot

- The repository is on `main` with Git history and an existing remote.
- The minimal native Windows ION path is integrated, documented, and covered by a GUI-subsystem/real-button callback test.
- The two compiler tests were corrected to match the implemented no-capture and homogeneous-list rules.
- Formatting, workspace builds, release build, and all 175 workspace tests pass on Windows x86_64 GNU.

The P1–P4 items below remain future recommendations where relevant. Re-evaluate each against current source before implementation.

## P1 — Make project docs and metadata accurate

6. Synchronize README/AGENTS/CHANGELOG/ROADMAP/TASKS/CURRENT_STATE/`docs/*`/`ERROR_CODES.json`/`explain.rs`/format version for the current 0.4.2 source. Clearly state native support boundaries and runtime errors.
7. Add missing stable runtime/native error code registrations for E5009 and E6002/E6003 if these codes are part of user-visible behavior, with explain entries and tests. Remove stale E2005/E6001 explanations only with a compatibility decision.
8. Add static audit/test command CI (Windows GNU) and protect release/test claims.

## P2 — GUI Foundation (only after architecture/API ADR)

9. **Define narrow runtime ABI**: typed opaque window/widget handles, controlled create/update/destroy/render/event functions, deterministic error reporting, and callback registry IDs. Keep Win32 bindings isolated in runtime. Avoid encoding OS handles as arbitrary integers exposed to user.
10. **Build initial custom renderer or native control backend consistently.** Requirement includes text + rectangle + rounded rectangle; ordinary static controls do not satisfy custom rounded shape rendering. If using Direct2D/DirectWrite, add explicit linker imports and lifetime/resource handling; if GDI, document rendering/typography tradeoffs. Do not claim framework-quality UI from a few built-in window calls.
11. **Implement host lifetime**: class registration once, per-window state, WndProc, create/destroy, message pump, error handling, QuitMessage, re-entrancy policy. Callbacks into Lutetium must stay on the VM/native UI thread or marshal safely.
12. **Rendering primitives and basic layout**: coordinate conventions, resize invalidation, logical-vs-physical pixels, DPI aware process setup, font selection, rounded rectangles, clipping, repaint handling. Keep UI core backend-independent (component tree, layout result, render commands, event types) while Windows backend performs Win32/Direct2D work.
13. **Input and callbacks**: click, key, text input, focus at minimum; test callback dispatch, nested event behavior, handler errors/panics and shutdown. Callback function signatures must be type-checked and associated with windows/widgets without raw function pointers.
14. **Tests**: separate pure tests for tree/layout/style/event mappings; Windows process integration to build and launch actual GUI. Headless CI needs a clear skip/runner policy and not fake user-input verification. Add a GUI smoke app only after event/render system is implemented.

## P3 — Language/native correctness before application expansion

15. Add native ABI checks/e2e for all supported scalar operations and diagnostics; function prototypes/runtime signatures should be generated from a shared builtin declaration source or validated automatically.
16. Add native recursion limit, reliable float formatting equivalence, runtime allocation management (not necessarily GC immediately), checked string pointer construction, and consistent runtime diagnostics.
17. Improve VM verifier must-analysis and malformed bytecode robustness; make deserialize/count limits explicit and fuzz container/instruction parsing.
18. Add benchmark harness with methodology and repeatable bytecode vs native runs before making performance claims.

## P4 — Framework expansion (future, depends on P2)

19. Build component properties/styles/themes and state/event propagation on top of audited renderer/layout core; avoid one bespoke Win32 path per component.
20. Add input/layout primitives incrementally (stack/grid/flex constraints/scrolling) with deterministic snapshots and resize tests.
21. Add animation, SVG/vector icons, advanced widgets only after frame scheduling, resource disposal and reduced motion semantics exist.
22. Portability: split native backend behind platform abstraction; implement/test separate Linux/macOS target/linker/runtime backends. Windows x64 GNU is the only currently configured target; ARM64 and mobile/web require independent feasibility milestones.

## Milestone exit criteria

A milestone is complete only when its public syntax is documented, checker rejects invalid signatures, generated standalone application links/runs, behavior is driven by Lutetium code, input/output and shutdown are tested, full pre-existing tests pass, and unsupported behavior is listed explicitly. GUI-subsystem linking alone is not an exit criterion.
