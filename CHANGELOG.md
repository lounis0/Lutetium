# Changelog

This file records repository changes that are represented in the project history documents. It is not a release certification; versions below are source workspace versions, and old test/build claims are not current test results.

## 0.4.2 — current workspace version

- Release alignment: workspace crates and CLI now report version 0.4.2; the
  0.4.1 implementation and documented experimental GUI scope are retained.

## 0.4.1 — previous workspace version

- Record-style map literals and field access; list/map heap support in the bytecode VM.
- VM generational tracing collector and GC diagnostics.
- Scalar Windows x64 GNU native backend, Cranelift object generation and MinGW linker integration.
- IR optimization and inspection commands.
- Experimental ION native Windows GUI slice: Win32 window/button, statically named `window -> unit` callback, and callback-driven close. The deterministic integration test uses `BM_CLICK`; physical mouse automation is not claimed. See `audit/TEST_RESULTS.md`.
- **Current limitations:** imports/modules, a general GUI framework, cross-platform native targets, and native collection support are not implemented. The native backend and GUI remain experimental; see `audit/KNOWN_ISSUES.md`.

## Historical development

Earlier project milestones and decisions are preserved in `DECISIONS.md` and `.development/SESSION_LOG.md`. Those documents describe work as it was recorded at the time and may not reflect current verification status. The current source audit is in `audit/`.
