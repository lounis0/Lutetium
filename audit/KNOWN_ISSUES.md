# Known issues and limitations — Lutetium 0.4.3

This document describes implementation scope and verified limitations. Lutetium,
the native backend and both GUI projects are experimental; passing tests do not
imply production readiness.

## Lutetium language GUI

The Lutetium language still supports only `gui_window`, `gui_button`, `gui_run`,
and `gui_close` in Windows x86_64 GNU GUI-subsystem executables. GUI built-ins
are unavailable in the bytecode VM. The callback is a named static Lutetium
function taking one window and returning unit; closures/captured state are not
supported. The prototype manages only one independently run window/message
loop. Button IDs occupy Win32's 16-bit namespace; use unique IDs in range
1..=65535. The Windows integration test checks PE subsystem and callback
routing by sending `BM_CLICK` to actual Win32 buttons; this is deterministic
control-notification testing, not physical mouse input. GUI CLI builds create
an executable; they do not launch it.

The language GUI API has no custom drawing, layouts, input controls, keyboard
or text events, reactive state, DPI support, accessibility abstraction, or
general widget/component framework. Those features should not be inferred from
shared builtin metadata or runtime declarations.

## Separate Rust ION crate

`lutetium-ion` 0.5.0 is a separate Rust package. It supplies a limited
component/layout/theme/event foundation and a Windows GDI renderer with
starter controls and examples. It is not GPU accelerated or cross-platform.
DPI-aware geometry, native accessibility integration, advanced widgets, and
non-Windows backends are not implemented. See `../docs/ION_0.5.md`.

## Other language/native limitations

- The Windows x86_64 GNU native backend supports only a subset of the VM;
  collections and unsupported built-ins are rejected.
- Native string-concatenation allocations use the process heap and are not
  reclaimed. Native recursion lacks the VM's call-depth guard.
- Integer arithmetic, including native signed division, remainder, and unary
  negation overflow, is checked; new native regression tests cover those cases.
- CLI `--release` does not currently change Cranelift optimization settings.
  Optimizer level 3 currently aliases level 2.
- Lists are homogeneous (with int-to-float widening); maps are records with a
  fixed key set and scalar values. Imports, closures, structs/enums, generics,
  async and sandboxing are not implemented.
- The bytecode verifier's register-definition merge is union-based rather than
  must-analysis; the VM's uninitialized-value check is a runtime safety net.
  The bytecode decoder requires an exact format version but does not reject
  trailing bytes after the entry name.

## Historical test source corrections

Two older compiler end-to-end cases were adjusted to match existing language
rules, not broaden them: functions do not capture top-level locals, so an
iterable is passed explicitly; lists are homogeneous, so bool and string
list-equality checks are separate. Historical 0.4.2 validation results remain
in `audit/TEST_RESULTS.md`; see the 0.4.3 changelog and current GitHub Actions
run for current release verification.
