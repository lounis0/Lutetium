# ION examples

These are standalone Rust programs using the `lutetium-ion` crate. They are **not** Lutetium `.lu` programs and do not call the language `gui_*` built-ins.

From the repository root, on supported Windows:

```powershell
cargo run -p lutetium-ion --example calculator
cargo run -p lutetium-ion --example component_gallery
```

Both commands open an interactive native window. The Windows backend uses Direct2D/DirectWrite when available and falls back to GDI. The examples are manual visual/input demos; the automated native-window test sends deterministic Win32 messages and does not prove physical mouse behavior or visual fidelity. See [`../../docs/ION_0.5.md`](../../docs/ION_0.5.md) for the implemented API and platform limits.
