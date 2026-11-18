# Test results — Lutetium 0.4.2 ION GUI slice

Verification was run on the configured Windows x86_64 GNU environment. These results cover the 0.4.2 release candidate.

## Workspace verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed. |
| `cargo check --workspace` | Passed. |
| `cargo build --workspace` | Passed. |
| `cargo build --release --workspace` | Passed. |
| `cargo test --workspace` | 175 passed, 0 failed; doc-tests passed (0 tests). |
| `cargo test -p lu-compiler --test end_to_end` | 116 passed, 0 failed. |
| `cargo test -p lu-native --test native_gui -- --nocapture` | Passed, including repeat runs; validates PE GUI subsystem and Win32 control callback dispatch. |
| `cargo test -p lutetium-cli --test cli -- --nocapture` | 13 passed, 0 failed, including GUI executable build. |
| `git diff --check` | Passed. |
| CLI version smoke check | `Lutetium 0.4.2`. |

Workspace test count: compiler end-to-end 116, native 14 (2 unit, 11
console, 1 GUI), syntax 24, VM 8, CLI 13; total 175. This is a Windows-host
result only and does not verify other hosts or targets.

## CLI target smoke checks

- `cargo run -p lutetium-cli -- check examples/ion_button.lu` — passed.
- `cargo run -p lutetium-cli -- build examples/ion_button.lu --target gui` —
  passed; wrote a Windows GUI executable.
- `cargo run -p lutetium-cli -- build examples/fibonacci.lu --target native`
  — passed; wrote a Windows console executable.
- `cargo run -p lutetium-cli -- build examples/fibonacci.lu --target bytecode`
  — passed; wrote an LTBC bytecode file.
- The native GUI integration test verifies that console-target linking refuses
  GUI IR and directs the caller to `--target gui`.

## GUI behavior verified

The integration test compiles and launches a Windows GUI-subsystem program,
checks `IMAGE_SUBSYSTEM_WINDOWS_GUI` in the generated PE, finds two native
Win32 `BUTTON` child controls with the same 16-bit ID, and sends `BM_CLICK` to
each. The first callback must leave the process running; the second calls
`gui_close(window)`, exits the message loop and cleanly terminates the process.
This validates Win32 window/control creation, `BN_CLICKED`/`WM_COMMAND`
notification, callback ABI/dispatch by child HWND, and shutdown.

The final repeatable test does **not** inject physical mouse down/up. An
experimental `SendInput` version succeeded in some runs but depended on
foreground activation and intermittently failed; it was removed in favor of
deterministic `BM_CLICK`. Do not describe `BM_CLICK` as physical mouse input.

## Scope

Verified source API: `gui_window(title, width, height)`,
`gui_button(window, id, text, x, y, width, height, handler)`, `gui_run(window)`,
and `gui_close(window)`. The named handler takes one `window` and returns
`unit`. Button IDs are clamped to Win32's 16-bit range; callers should provide
unique IDs in range. `WM_DESTROY` posts thread-wide `WM_QUIT`, so only one
independently run window/message loop is supported. VM GUI builtins remain
unavailable. No result establishes custom rendering, layouts, keyboard
handling, reactive state, DPI behavior, broad control support, physical mouse
automation, or a general-purpose GUI framework.

Two older compiler test programs were adjusted without broadening language
rules: functions do not capture top-level locals (the mutable list is passed
explicitly), and lists remain homogeneous (bool and string equality are
checked independently).
