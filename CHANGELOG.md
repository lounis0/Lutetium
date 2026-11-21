# Changelog

User-visible release notes for Lutetium. The independent `lutetium-ion` crate maintains a separate version stream and is called out separately where relevant.

## 0.4.4 — working tree (unreleased)

This section records the 0.4.4 changes and their validation limits. The commit
containing these changes is not a release tag or publication. Release notes
should be finalized only after the complete target-appropriate test suite is
verified.

### Improved
- Strengthened CLI `run`, `check`, and `build` flag validation for unknown, duplicate, missing, inline, and invalid values; added regression coverage.
- Added strict one-entry project scaffolding/workflows, native console and GUI-subsystem project builds, and source-line text in JSON diagnostics; project builds emit named files under `dist/`.
- Removed the inert comment-only native scratch test after checking repository references; retained scratch-named `.lu` files because they are meaningful iteration, equality, and diagnostic regression cases.
- Resolved all current workspace Clippy warnings and documented future `.LTM` integration boundaries without adding a parser or runtime.

### Version alignment
- Aligned the Lutetium workspace crates, CLI lockfile entries, standalone runtime manifest, and error catalog language version to 0.4.4. `lutetium-ion` keeps its independent version.

### Validation limitations
- See [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md). The full workspace test command is currently blocked in this environment by Cargo's failure to spawn the pinned GNU `dlltool`; focused compiler/VM/CLI and native integration checks are recorded separately.

## 0.4.3 — 2026-11-18

### Added
- Added the independent `lutetium-ion` 0.5.0 Rust UI crate to the Cargo workspace, preserving its separate package identity and the existing Lutetium language GUI ABI.
- Added ION calculator and component-gallery Rust examples, component/layout/theme/event unit coverage, and Windows native-window integration coverage for rendering lifecycle, input and selection events.

### Improved
- Native Windows integer division, remainder and negation now detect signed overflow consistently with the bytecode VM and report a runtime error instead of relying on target-specific machine behavior.
- Updated native linking for recent Rust Windows runtimes by linking the bundled MinGW synchronization import library.
- Added native executable regressions for `i64::MIN / -1`, `i64::MIN % -1` and negating `i64::MIN`.
- Rewrote the README for current capabilities, setup, supported targets, compiler flow, ION distinction, development, and roadmap; clarified historical audit snapshots.
- Updated CLI explanations and machine-readable error catalog for current native unsupported-feature and linker errors.

### Compatibility
- Bumped the Lutetium workspace, runtime package and CLI to 0.4.3. The standalone ION crate remains 0.5.0.
- The bytecode format remains LTBC v5. Existing `gui_window`, `gui_button`, `gui_run`, and `gui_close` contracts remain unchanged.

### Known limitations
- Lutetium, its native backend and both GUI surfaces remain experimental. Native code generation and GUI execution are Windows x86_64 GNU only.
- `lutetium-ion` uses Direct2D/DirectWrite with a GDI fallback. It does not implement native accessibility APIs, advanced widget systems, or non-Windows backends.
- Lutetium imports/modules, closures, structs/enums, generics and sandboxing are not implemented. Optimizer level 3 currently aliases level 2; native `--release` does not configure Cranelift optimization.

## 0.4.2

- Aligned Lutetium workspace crates, CLI and standalone runtime metadata to 0.4.2; retained the experimental native Windows GUI API.
- See Git history and `audit/TEST_RESULTS.md` for the historical 0.4.2 verification snapshot.

## 0.4.1

- Added record-style maps, map methods/field access, deep list equality, VM GC support and the experimental scalar Windows native backend.
- See Git history for detailed historical changes and constraints.
