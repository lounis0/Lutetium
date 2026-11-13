# Changelog

This file records repository changes that are represented in the project history documents. It is not a release certification; versions below are source workspace versions, and old test/build claims are not current test results.

## 0.4.1 — current workspace version

- Record-style map literals and field access; list/map heap support in the bytecode VM.
- VM generational tracing collector and GC diagnostics.
- Scalar Windows x64 GNU native backend, Cranelift object generation and MinGW linker integration.
- IR optimization and inspection commands.
- **Current limitations:** imports/modules, language-level GUI framework, cross-platform native targets and native collection support are not implemented. Current audited source has a workspace compile error in incomplete GUI IR integration and two failing compiler end-to-end tests; see `audit/TEST_RESULTS.md`.

## Historical development

Earlier project milestones and decisions are preserved in `DECISIONS.md` and `.development/SESSION_LOG.md`. Those documents describe work as it was recorded at the time and may not reflect current verification status. The current source audit is in `audit/`.
