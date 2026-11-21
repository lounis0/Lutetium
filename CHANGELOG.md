# Changelog

User-visible release notes for Lutetium. The independent `lutetium-ion` crate maintains a separate version stream and is called out separately where relevant.

## 0.4.4 — 2026-11-21

Lutetium 0.4.4 aligns the workspace, CLI, standalone native runtime metadata,
and error catalog. `lutetium-ion` remains independently versioned. The release
adds strict single-entry project scaffolding and workflows, improves CLI flag
validation and JSON diagnostics, and documents future `.LTM` boundaries without
adding a parser or runtime.

### Improved
- Strengthened CLI `run`, `check`, and `build` flag validation for unknown, duplicate, missing, inline, and invalid values; added regression coverage.
- Added strict one-entry project scaffolding/workflows, native console and GUI-subsystem project builds, and source-line text in JSON diagnostics; project builds emit named files under `dist/`.
- Removed the inert comment-only native scratch test after checking repository references; retained scratch-named `.lu` files because they are meaningful iteration, equality, and diagnostic regression cases.
- Resolved all current workspace Clippy warnings and documented future `.LTM` integration boundaries without adding a parser or runtime.

### Version alignment
- Aligned the Lutetium workspace crates, CLI lockfile entries, standalone runtime manifest, and error catalog language version to 0.4.4. `lutetium-ion` keeps its independent version.

### Validation
- GitHub Actions Windows x86_64 GNU CI passed formatting, workspace check/build/test, and release build for the release commit. The complete-workspace test result and focused local results are recorded in [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md). A local attempt to run the workspace suite remains blocked because this machine's pinned GNU `dlltool.exe` fails to spawn a child process.

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
