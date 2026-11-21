# Lutetium

**Lutetium 0.4.4** is an experimental, statically typed programming language and compiler implemented in Rust. It aims to offer a compact, readable language for small programs while making the path from source to execution explicit: a hand-written frontend, typed register IR, verified bytecode and VM, plus an experimental Windows native backend.

Lutetium is written in Rust; it is not Rust syntax or a Python implementation. Its syntax favors familiar, readable expressions, while using explicit types where useful, braces for blocks, and static checking. The VM and compiler use Rust, and generated native executables are standalone Windows programs.

- **Repository:** [lounis0/Lutetium](https://github.com/lounis0/Lutetium)
- **Issues:** [report a bug or request a feature](https://github.com/lounis0/Lutetium/issues)
- **Releases:** [GitHub releases](https://github.com/lounis0/Lutetium/releases)
- **License:** MIT

> **Status: experimental.** The supported execution path is the bytecode VM. The native backend and both GUI projects are narrow, platform-specific experiments; this project is not a security sandbox or a production-ready toolchain.

## Capabilities today

### Implemented

- Lexer, recursive-descent parser, source spans, diagnostics, and static type checking.
- Functions, inferred parameter/return types, top-level statements, scalar values, conditionals, loops, and short-circuit logic.
- Homogeneous lists (with integer-to-float widening), record-like maps with fixed keys, and deep list equality.
- String, math, filesystem and collection built-ins in the bytecode VM.
- LTBC v5 bytecode serialization, an independent verifier, a register-machine VM, and a generational tracing collector for VM list/map values.
- CLI commands: `new`, `run`, `build`, `check`, `inspect`, `explain`, `doctor`, and help/version output.
- Experimental Windows x86_64 GNU native console backend for a subset of scalar programs. Unsupported native operations are rejected instead of silently treated as supported.

### Experimental

- **Language-level `gui_*` API:** Windows x86_64 GNU native GUI-subsystem executable with a Win32 window, standard push buttons, named `window -> unit` callbacks, a message loop, and close. This remains a small independent prototype. Its native integration test uses Win32 `BM_CLICK` to validate notification/callback dispatch; it does not test physical mouse input. The bytecode VM reports GUI APIs as unavailable.
- **ION Rust UI crate (`lutetium-ion`):** separate from Lutetium syntax and the legacy `gui_*` API. Provides a Windows Direct2D/DirectWrite renderer with a GDI fallback, per-monitor-DPI-aware geometry and input, component-clipped text, basic row/column layout, themes, events/state and starter controls, with calculator and component-gallery examples. The Direct2D backend uses the default render-target type (hardware acceleration where available with a supported software fallback); GDI is the fallback if setup or drawing fails. This is not a GPU-acceleration guarantee, nor a general-purpose or cross-platform UI framework. See [`docs/ION_0.5.md`](docs/ION_0.5.md).

### Not implemented

Imports/modules, closures, structs/enums, generics, async, a package manager, a standard library module tree, and cross-platform native code generation are not implemented. ION does not currently provide native accessibility integration, advanced widget systems, or non-Windows backends. These are future work, not current capabilities.

## Getting started

### Create a project

The CLI can scaffold a console app or the existing experimental native Win32 GUI prototype:

```powershell
lutetium new MyApp
cd MyApp
lutetium check
lutetium run
lutetium build
```

A console project builds to `dist/MyApp.exe` using the current Windows x64 GNU native backend. The generated console program can run with `lutetium run` or as its executable. To create the native GUI prototype, use `lutetium new MyApp --template gui`; `lutetium build` selects its GUI subsystem target and writes `dist/MyApp.exe`. The GUI template uses the implemented `gui_window`/`gui_button`/`gui_run`/`gui_close` API; it is not the separate Rust `lutetium-ion` component framework. Projects contain `lutetium.toml` and `src/main.lu`. This initial manifest supports only `name`, `entry`, and `kind`; it does not add module imports or multi-file compilation.

`run`, `check`, and `build` accept a project directory or an individual `.lu` file. Individual-file commands keep their existing behavior and output locations. Project builds write named artifacts into `dist/`; console project builds default to the native console executable, while GUI project builds select the GUI subsystem.

### Prerequisites and supported platform

The repository pins the **stable `x86_64-pc-windows-gnu` Rust toolchain** in `rust-toolchain.toml` and forces that target in `.cargo/config.toml`. Use Windows x86_64 with rustup and Cargo; the bundled MinGW-w64 target components are used for native executable linking. MSVC/Visual Studio is not required. Bytecode and language behavior are implemented in Rust, but the checked-in workspace configuration and release validation target Windows GNU; other host/target configurations are not claimed as supported.

From PowerShell, in a checkout:

```powershell
cargo build --workspace
cargo run -p lutetium-cli -- --version
cargo run -p lutetium-cli -- run examples/hello.lu
```

The `cargo run` invocation is a development CLI. After `cargo build --release`, the executable is at `target\x86_64-pc-windows-gnu\release\lutetium.exe`.

### Windows install scripts

After installing Rust via rustup and cloning this repository, run:

```powershell
.\scripts\install.ps1
```

This builds the release CLI, installs it under `%LOCALAPPDATA%\Programs\Lutetium\bin`, and adds that directory to the **user** PATH. Open a new terminal and check `lutetium --version`. The separate `scripts/make_dist.ps1` script builds a portable Windows x64 ZIP locally; it does not upload or publish an artifact. `scripts/uninstall.ps1` removes the user-level install after confirmation.

### Run a Lutetium program

```powershell
lutetium run examples\fibonacci.lu
```

Example source:

```lu
fn fibonacci(n: int) -> int {
    if n <= 1 {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    print(fibonacci(10))
}
```

`lutetium check examples\fibonacci.lu` type-checks without executing. `lutetium run examples\maps.lu --gc-stress --gc-stats` runs the program with frequent VM collections and prints GC counters.

## Compiler and runtime

`lutetium run` reads source, lexes/parses it, resolves names, type-checks and lowers it to typed IR, generates LTBC bytecode, verifies the bytecode, then executes it in the register VM. `lutetium build <file.lu>` defaults to verified bytecode at `build/<name>.lbc`. `lutetium build <file.lu> --target native` compiles supported typed IR through Cranelift and links a Windows x64 GNU executable. The native backend supports a subset of scalar operations; lists/maps and unsupported builtins fail with a diagnostic. `--release` does not currently change Cranelift's generated-code optimization settings; `--opt 0..3` controls the compiler IR optimizer, and optimizer level 3 currently behaves like level 2.

The VM verifies bytecode before execution, but **Lutetium is not a sandbox**: filesystem built-ins have the current process's file permissions, and native output is ordinary executable code. Do not run untrusted programs expecting isolation.

## ION native GUI

ION currently means two separate things in this repository:

1. The existing Lutetium `gui_window`, `gui_button`, `gui_run`, and `gui_close` language built-ins. They remain experimental and backward-compatible; build them with `lutetium build app.lu --target gui`. They only work in Windows x86_64 GNU GUI-subsystem executables, not in the VM.
2. The independent Rust package `lutetium-ion`, which provides Direct2D/DirectWrite-rendered native windows with a GDI fallback, a small reusable component/layout/theme/event foundation, and Rust examples:

   ```powershell
   cargo test -p lutetium-ion
   cargo build -p lutetium-ion --examples
   cargo run -p lutetium-ion --example calculator
   cargo run -p lutetium-ion --example component_gallery
   ```

   The last two commands open interactive native windows. Direct2D uses the platform default render-target type, with supported software fallback; GDI is used if Direct2D setup/drawing fails. Do not assume GPU execution. These examples do not use the Lutetium `gui_*` built-ins. Detailed scope, API and limitations are in [`docs/ION_0.5.md`](docs/ION_0.5.md).

## Architecture

```text
crates/diagnostics  spans and human/JSON diagnostics
crates/syntax       lexer, parser and AST
crates/compiler     resolution, type checking, typed IR, lowering and optimizer
crates/vm           LTBC bytecode, verifier, register VM and VM heap/GC
crates/native       Cranelift backend and Windows GNU linker
crates/runtime      static native runtime source embedded by crates/native
crates/cli          Lutetium command-line executable and command library
crates/ion          independent Rust native UI crate (ION 0.5)
examples/           Lutetium programs and Rust ION demos
scripts/            Windows install, uninstall and local distribution scripts
docs/               language, bytecode, architecture, runtime and ION docs
audit/              source review, known limitations, test results and roadmap
```

Further detail: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`docs/LANGUAGE_SPEC.md`](docs/LANGUAGE_SPEC.md), [`docs/STANDARD_LIBRARY.md`](docs/STANDARD_LIBRARY.md), [`docs/BYTECODE_FORMAT.md`](docs/BYTECODE_FORMAT.md), and [`docs/RUNTIME_ARCHITECTURE.md`](docs/RUNTIME_ARCHITECTURE.md).

## Development and validation

Run at the repository root with the pinned Windows GNU toolchain:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test -p lu-vm --offline
cargo test -p lu-compiler --offline
cargo test -p lutetium-cli --offline --test cli
cargo test --workspace --offline
cargo build --workspace --release --offline
```

The pinned GNU toolchain test run may be blocked if Cargo cannot spawn its bundled `dlltool`; report the precise result rather than substituting an MSVC check. Native GUI integration uses deterministic Win32 `BM_CLICK` dispatch, not physical mouse automation. The interactive ION examples open windows and need visual/manual checks. See [`CONTRIBUTING.md`](CONTRIBUTING.md), [`docs/AGENT_GUIDE.md`](docs/AGENT_GUIDE.md), and [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md) for commands and verified versus blocked coverage. Language examples use `.lu`; `examples/regression/` contains known compiler regression fixtures, not only runnable programs.

## Roadmap

The 0.4.3 release added ION and native arithmetic fixes. Current 0.4.4 worktree updates and verification limits are described in [`CHANGELOG.md`](CHANGELOG.md) and [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md). Future priorities include hardening bytecode decoding/verifier invariants, improving native runtime allocation/recursion/error behavior, and separately investigating portability and accessibility. `.LTM` integration is documentation-only; see [`docs/LTM_INTEGRATION.md`](docs/LTM_INTEGRATION.md). The native console backend, legacy GUI API, and ION remain experimental. See [`audit/ROADMAP.md`](audit/ROADMAP.md).

## Lutetium 0.4.4

This working tree aligns the Lutetium crates to **0.4.4**; it is not a claim that a 0.4.4 release has been published. See [`CHANGELOG.md`](CHANGELOG.md) and [`audit/TEST_RESULTS.md`](audit/TEST_RESULTS.md) for the change summary and the exact current validation limits. Check [GitHub releases](https://github.com/lounis0/Lutetium/releases) for published release records and assets.
