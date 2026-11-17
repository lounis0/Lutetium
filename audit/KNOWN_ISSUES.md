# Known issues and limitations — Lutetium 0.4.1

This document describes the current source and verified behavior. The language,
compiler, native backend and ION GUI are experimental; passing tests do not
imply production readiness.

## ION GUI scope

On Windows x86_64 GNU, the only supported GUI source operations are:

- `gui_window(title, width, height)` creates one centered native Win32 window.
- `gui_button(window, id, text, x, y, width, height, handler)` creates a
  standard push button. `handler` is a statically named Lutetium function
  taking one `window` and returning `unit` (return inference is supported).
- `gui_run(window)` shows the window and starts the message loop.
- `gui_close(window)` destroys the window and ends the loop.
- `lutetium build <file.lu> --target gui` builds a GUI-subsystem executable.

`crates/native/tests/native_gui.rs` checks the executable's PE GUI subsystem,
launches the program, and sends `BM_CLICK` to two real native buttons with the
same ID but different callbacks. It verifies HWND-based callback selection
and callback-driven process shutdown. This is deterministic control
notification testing, not physical mouse input. See `audit/TEST_RESULTS.md`.

## GUI limitations

- The workspace/native backend is configured for `x86_64-pc-windows-gnu`;
  no other GUI platform/architecture is established.
- Only one window/message loop is supported. Destroying a window posts
  thread-wide `WM_QUIT`, so multiple independent windows are not managed.
- Control IDs are clamped to Win32's 16-bit range. Use unique IDs in
  1..=65535; HWND-based callback mapping prevents ID collisions from choosing
  the wrong Lutetium callback, but other Win32 ID-based behavior may collide.
- Callbacks are named, static function pointers without closures/captured
  state. The ABI is an experimental native boundary.
- GUI builtins are unavailable in the bytecode VM.
- GUI CLI builds produce an executable; they do not launch it.
- Labels, inputs, custom drawing/rendering, shapes, clipping, layouts,
  resizing-aware behavior, keyboard/text events, accessibility abstraction,
  themes, reactive state, animation, DPI-aware coordinates, and a general
  widget/component framework are not implemented as supported API. GUI names
  in shared builtin metadata and runtime declarations are not proof of support.

## Other project limitations

- Native support covers a subset of the VM; collections and many builtins are
  rejected. Native string-concatenation allocations use the process heap and
  are not reclaimed. Native recursion lacks the VM's call-depth guard.
- Lutetium CLI `--release` does not currently change generated Cranelift
  optimization settings. O3 currently aliases O2.
- Lists are homogeneous except for int-to-float widening. Maps are
  record-like with a fixed key set and scalar values. Imports, closures,
  structs/enums, generics and async are not implemented.
- The bytecode verifier's register-definition merge is union-based, not a
  must-analysis; the VM's uninitialized-value check remains a runtime safety
  net. The bytecode decoder requires an exact version but does not reject
  trailing bytes after the entry name (see `docs/BYTECODE_FORMAT.md`).

## Test source corrections

Two compiler e2e cases were corrected to match existing language rules, not to
broaden them: functions do not capture top-level locals, so iterable-once
passes a mutable list explicitly; lists are homogeneous, so bool/string
list equality is checked as separate lists. The full Windows x64 GNU test
results are recorded in `audit/TEST_RESULTS.md`.
