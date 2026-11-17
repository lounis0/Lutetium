# Current worktree audit — Lutetium 0.4.1

## Snapshot and scope

This audit inventories the current working tree relative to `HEAD` on `main` at
`1f86307` (`docs: record repository preparation status`). All changes listed
below are uncommitted and unpushed. The separate website task is paused; no
`lutetium-website/` files or dependencies have been created.

## Changed files

### User docs and project metadata

- `README.md` — accurately describes the experimental Windows GUI slice,
  commands, target scope and deterministic `BM_CLICK` test semantics.
- `CHANGELOG.md` — replaces stale build-blocker statement with current
  Windows ION prototype status and limitations.
- `docs/LANGUAGE_SPEC.md`, `docs/STANDARD_LIBRARY.md`, `docs/GRAMMAR.ebnf` —
  document opaque GUI types, four supported APIs, named callback contract,
  Windows-only target and limitations.
- `docs/ARCHITECTURE.md` — describes bytecode/native paths and current GUI
  boundary.
- `docs/BYTECODE_FORMAT.md` — corrects stale v3 claim to LTBC v5 and records
  current container/opcode/builtin encoding, decoder/verifier behavior and
  explicit VM GUI unavailability.

### Audit docs

- `audit/ARCHITECTURE.md` — source-verified architecture and Windows ION
  implementation boundary.
- `audit/KNOWN_ISSUES.md` — current GUI scope, runtime and native limitations.
- `audit/TEST_RESULTS.md` — Windows test results and a clear distinction
  between native `BM_CLICK` notification testing and physical mouse input.
- `audit/CURRENT_WORKTREE_AUDIT.md` — this inventory and status.
- `audit/FULL_CODEBASE_AUDIT.md`, `audit/REPOSITORY_INVENTORY.md`, and
  `audit/ROADMAP.md` are retained historical audit snapshots; they predate
  this feature and still contain stale blocker/test claims. Do not treat them
  as current status reports without a future refresh.

### CLI and examples

- `crates/cli/src/lib.rs` — recognizes `--target gui` and selects the GUI
  subsystem build path.
- `crates/cli/tests/cli.rs` — builds the example through the GUI CLI target.
- `examples/ion_button.lu` (new) — small native Win32 button/callback example.

### Compiler and VM

- `crates/compiler/src/typecheck.rs` — validates the eight-source-argument
  button form, statically named one-window/`unit` callback, and GUI call types.
- `crates/compiler/src/lower.rs`, `crates/compiler/src/ir.rs` — carry the
  resolved callback function index in native button IR.
- `crates/compiler/src/codegen.rs` — keeps bytecode generation total for GUI
  button IR; VM execution reports GUI unavailable rather than panicking.
- `crates/compiler/tests/end_to_end.rs` — corrects two older tests to honor
  noncapturing function scope and homogeneous-list rules.
- `crates/vm/src/bytecode.rs` — corrects shared GUI builtin signature metadata.
- `crates/vm/src/vm.rs` — formats the explicit GUI-unavailable runtime path.

### Native backend/runtime and integration

- `crates/native/Cargo.toml` — includes builtin metadata dependency.
- `crates/native/src/backend.rs` — lowers supported window/run/close builtins
  and statically resolved button callbacks through the native runtime ABI.
- `crates/native/src/linker.rs` — retains the GUI subsystem linker option and
  required Win32 libraries.
- `crates/native/src/lib.rs` — rejects GUI IR passed to the console-exe API.
- `crates/runtime/src/lib.rs` — creates Win32 windows and push buttons,
  dispatches `WM_COMMAND/BN_CLICKED` by `(parent HWND, child HWND)`, and
  supports the basic message loop and destruction path.
- `crates/native/tests/native_gui.rs` (new) — checks GUI PE subsystem,
  rejects console linking of GUI IR, launches the executable, uses two
  same-ID native buttons with distinct handlers, sends `BM_CLICK`, and checks
  callback selection and shutdown.

## Current verified capability

Lutetium 0.4.1 has a compiler, bytecode VM, and Windows x64 GNU native
executable backend. The ION addition is only a native GUI prototype:

- `gui_window(title, width, height)` creates one Win32 top-level window.
- `gui_button(window, id, text, x, y, width, height, handler)` creates a
  standard push button. `handler` is a named Lutetium function taking one
  `window` and returning `unit` (inference is accepted).
- `gui_run(window)` starts the message loop; `gui_close(window)` destroys the
  window and exits it.
- `lutetium build <file.lu> --target gui` links a GUI-subsystem executable.

The native integration test checks `IMAGE_SUBSYSTEM_WINDOWS_GUI` in the PE
header, opens two real child button controls that intentionally share a
16-bit ID, sends each control `BM_CLICK`, and verifies only the selected
callback closes the app. It tests the native control notification/callback
path; it does not simulate physical mouse input. Button IDs are clamped to
Win32's 16-bit namespace, so use unique IDs in range. `WM_DESTROY` posts
thread-wide `WM_QUIT`, so independent multi-window loops are not supported.
Other GUI builtin names/types/runtime declarations are scaffolding, not
supported API. GUI calls remain unavailable in the bytecode VM.

The GUI work does not establish custom drawing, layouts, keyboard input,
reactive state, DPI support, broad widget lifecycles, or cross-platform GUI
support. Native output is configured/tested only for `x86_64-pc-windows-gnu`.

## Verification status

Commands run successfully on Windows x86_64 GNU:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo build --workspace`
- `cargo build --release --workspace`
- `cargo test --workspace` — 175 passed, 0 failed
- `git diff --check`
- `cargo test -p lu-native --test native_gui -- --nocapture` — repeated
  successfully, including duplicate-ID dispatch regression and GUI PE check.
- `cargo test -p lutetium-cli --test cli -- --nocapture` — 13 passed.
- `cargo run -p lutetium-cli -- check examples/ion_button.lu` — passed.
- CLI builds for `--target gui`, `native`, and `bytecode` — passed.

See `audit/TEST_RESULTS.md` for historical test counts and the important
physical-input caveat. Refresh it after the final full-workspace run in this
working session if anything changes.

## Git state and next steps

The working tree is based on `main` at `1f86307`; no changes have been
committed or pushed. Before any commit/push, inspect the full diff and
staged/untracked inventory. No build/test verification here establishes
cross-platform support. Keep ION scope explicitly experimental; add one
feature at a time with Windows integration coverage. Review the callback ABI,
resource cleanup and runtime error/lifetime behavior before expanding the API.
