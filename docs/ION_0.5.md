# Lutetium ION 0.5 foundation

ION 0.5 is introduced as a separate `lutetium-ion` Rust crate. It does not add compiler intrinsics or change the existing Lutetium `gui_window`, `gui_button`, `gui_run`, and `gui_close` ABI. The legacy language GUI slice remains experimental and independently usable.

## Architecture

- `component`, `layout`, `theme`, `event`, and `renderer`: platform-neutral component model, stack layout, semantic design tokens, input/application events, and draw-command frame generation.
- `backend`: platform selection and native application integration. The Windows backend in `backend/windows.rs` consumes backend-neutral draw frames and uses Direct2D/DirectWrite with a GDI fallback.
- `examples/ion`: calculator and reusable component gallery implemented against the Rust library.

The Windows renderer uses Direct2D/DirectWrite (default render-target acceleration where available) and falls back to Win32 GDI if renderer setup or drawing fails. Direct2D antialiases vector geometry, uses ClearType text, and draws rounded borders slightly inset to avoid clipping their antialiased edge. Pointer motion avoids duplicate-position work and repaints only when hover or callback-driven UI state changes; Direct2D targets survive minimization and are recreated after device loss. Both paths support text clipping to component bounds; the window is per-monitor DPI aware and layout/input coordinates use logical units. GDI is not GPU accelerated. SVG paths, general clipping layers, shadows/gradients, native accessibility APIs, window maximize/minimize policies, and non-Windows backends are not implemented. GDI text uses the configured font family and size/weight; DirectWrite caches brushes and text formats.

## Current first slice

- Native window creation, resize and close event handling, client-area paint and message loop.
- Backend-neutral fill/stroke/rounded-rectangle/line/text draw commands, with per-component text bounds and clipping.
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

Run the automated suite on Windows with the installed MSVC Rust target, or cross-check compilation against the GNU target. The GNU linker on some systems requires a working `dlltool` to run linked test binaries. The two `run` commands open native interactive windows and are not headless tests. A Windows native-window integration test uses Win32 window messages to exercise click, text-input, selection callbacks and shutdown; it does not automate physical mouse input.
