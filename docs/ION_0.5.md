# Lutetium ION 0.5 foundation

ION 0.5 is introduced as a separate `lutetium-ion` Rust crate. It does not add compiler intrinsics or change the existing Lutetium `gui_window`, `gui_button`, `gui_run`, and `gui_close` ABI. The legacy language GUI slice remains experimental and independently usable.

## Architecture

- `component`, `layout`, `theme`, `event`, and `renderer`: platform-neutral component model, stack layout, semantic design tokens, input/application events, and draw-command frame generation.
- `backend`: platform selection and native application integration. The initial backend lives in `backend/windows.rs` and consumes the backend-neutral draw frame.
- `examples/ion`: calculator and reusable component gallery implemented against the Rust library.

The first renderer uses Win32 GDI. It is a genuine native renderer and is not GPU accelerated. Direct2D/DirectWrite, SVG paths, clipping/layers, shadows/gradients, DPI-aware geometry, native accessibility APIs, window maximize/minimize policies, and non-Windows backends are not implemented. GDI fonts currently use Segoe UI; typography tokens affect point sizing/weight, while font-family substitution is not yet exposed.

## Current first slice

- Native window creation, resize and close event handling, client-area paint and message loop.
- Backend-neutral fill/stroke/rounded-rectangle/line/text draw commands.
- Stable component IDs, nested row/column stacks, padding/gap, minimum/preferred sizing and flex allocation.
- Shared light/dark/customizable semantic tokens.
- Text/heading, themed buttons, text/number input, checkbox/switch selection, card and divider components.
- Mouse movement/press/release, key press/release, text input, focus changes, component activation and resize events; Tab cycles focus.
- Input values, button styles, live theme changes, and callbacks on the event loop.

The controls in this first slice are drawn by the shared renderer. Selection toggles are currently checkbox-like rather than a polished native switch; they share the same interaction and theme contract. No claim is made for screen-reader support. Components should have unique IDs within the tree. Advanced controls, icon paths, validation presentation, password masking, sliders, modal/popover/menu handling, and animation are future work.

## API example

```rust
use lutetium_ion::{Application, Component, Event, Theme};

let root = Component::column("root")
    .padding(24.0)
    .gap(12.0)
    .push(Component::heading("title", "Hello ION"))
    .push(Component::button("continue", "Continue"));

Application::new("Example")
    .size(720, 480)
    .theme(Theme::light())
    .content(root)
    .on_event(|event, _components, _theme| {
        if let Event::ButtonPressed(id) = event { println!("{id}"); }
    })
    .run()?;
```

Build/test the library and examples using the pinned Windows GNU toolchain:

```powershell
cargo test -p lutetium-ion
cargo test -p lutetium-ion --examples
cargo run -p lutetium-ion --example calculator
cargo run -p lutetium-ion --example component_gallery
```

The two `run` commands open native interactive windows and are not headless tests. A Windows native-window integration test uses Win32 window messages to exercise the click event and shutdown path; it does not automate physical mouse input.
