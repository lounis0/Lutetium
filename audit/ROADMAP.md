# Prioritized roadmap from current audited state

This roadmap is limited to concrete work implied by verified gaps. Each task should land with positive/negative tests and synchronized docs. Do not treat a roadmap item as shipped until its acceptance tests pass.

## P0 — Restore a reviewable, compiling baseline

1. **Establish provenance and freeze GUI edits.** This checkout has no `.git`; initialize/use proper version control outside the audit if needed so incomplete GUI changes can be reviewed without losing prior functionality. Do not reset source blindly.
2. **Decide intended GUI milestone and public API first.** Current unimplemented GUI builtin names/signatures (`BuiltinKind`, checker AST, partial IR) conflict with each other and have no documented contract. Decide whether widgets are native controls or custom-rendered components; choose how immutable callback IDs bridge to Lutetium function dispatch; specify UI thread/state/ownership.
3. **Either remove/quarantine broken GUI scaffolding or complete the compiler integration as a separately tested slice.** Currently `IrInstr::GuiButton` is missing in native translator; bytecode codegen panics; VM errors. Do not add more identifiers before an end-to-end ABI exists. Preserve console and VM behavior.
4. **Resolve compiler suite failures.** Fix parser acceptance for `for x in mk()` or adjust intended semantics/test; settle heterogeneous list test vs homogeneous-list language contract. Verify parser and checker agree.
5. **Restore clean build, tests and formatting.** Acceptance: `cargo fmt --all -- --check`, `cargo build --workspace`, `cargo test --workspace`, `cargo build --release`. Publish the observed exact count/results, not old report values.

## P1 — Make project docs and metadata accurate

6. Synchronize README/AGENTS/CHANGELOG/ROADMAP/TASKS/CURRENT_STATE/`docs/*`/`ERROR_CODES.json`/`explain.rs`/format version for actual 0.4.1 source. Clearly state native support boundaries and runtime errors.
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
