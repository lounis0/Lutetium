# Lutetium architecture — source-verified snapshot

**Project version:** 0.4.2. **Target verified in this snapshot:** Windows x86_64 GNU. The repository was on branch `main` at base commit `1f86307` when this maintenance pass began; implementation changes were uncommitted then. This report describes the reviewed source state, not a claim of cross-platform or production readiness.

## 1. Crate architecture

```mermaid
flowchart TD
  diag[lu-diagnostics]
  syntax[lu-syntax]
  vm[lu-vm: LTBC + verifier + register VM + GC]
  compiler[lu-compiler: resolver/typecheck/IR/lowering/optimizer/codegen]
  native[lu-native: Cranelift + COFF + linker]
  cli[lutetium-cli]
  runtime[crates/runtime staticlib source]
  syntax --> diag
  compiler --> syntax
  compiler --> diag
  compiler --> vm
  native --> compiler
  native --> vm
  cli --> compiler
  cli --> vm
  cli --> native
  native -. build.rs compiles and embeds .-> runtime
```

`crates/runtime` is outside the Cargo workspace. `crates/native/build.rs`
invokes `rustc --crate-type staticlib` on its source and embeds the archive in
the native crate. Generated executables link that runtime; they do not require
the compiler or source tree at execution.

## 2. Source pipelines

```text
.lu -> lex -> parse/AST -> resolve -> type check/inference
    -> typed register IR -> optimizer (O0–O2; O3 currently aliases O2)
        -> bytecode codegen -> LTBC v5 -> verifier -> register VM
        -> Cranelift -> COFF -> bundled MinGW linker + runtime
             -> Windows console executable (`--target native`)
             -> Windows GUI-subsystem executable (`--target gui`)
```

CLI `run` compiles to bytecode, verifies, then executes it. Bytecode `build`
verifies before serialization. Console and GUI native targets compile typed IR
directly and rely on compiler IR invariants plus backend feature checks; they
do not pass through bytecode verification.

## 3. Frontend and compiler

The lexer/parser build the owned AST with source spans. The resolver validates
top-level function names and entry-point rules. The type checker handles
scopes, inference, control flow, numeric widening (int to float), homogeneous
lists, record-style maps and builtins. The shared typed IR is register-based
with explicit blocks and terminators. The lowering and optimizer feed both
backends.

The language and API contract is documented in `docs/LANGUAGE_SPEC.md` and
`docs/STANDARD_LIBRARY.md`. Notable limits include no modules/imports,
closures, structs/enums, or heterogeneous list values.

## 4. Bytecode, verifier, VM and GC

LTBC v5 has a magic/version header, constants, function records and entry
function name. Operands are fixed-width little-endian. The decoder requires an
exact version match. `docs/BYTECODE_FORMAT.md` describes the current encoding
and version history.

The verifier checks opcode shapes, operand and index bounds, call/builtin
arities, branch ranges, reachable control flow and reads-before-write along
its explored dataflow. The register definition merge is union-based, not a
must-analysis; the VM initializes registers to an uninitialized sentinel and
reports a runtime error if it is read.

The VM is a register machine with per-call-frame registers, a recursion limit,
scalar values, builtins, mutable lists/maps and a precise generational tracing
collector. Heap roots are the registers in all live VM frames. List/map
mutation uses a write barrier. GUI bytecode builtins return an explicit
unavailable error; GUI support is native-only.

## 5. Native Windows x64 GNU backend

The Cranelift backend emits WindowsFastcall functions and COFF objects. The
linker uses the self-contained MinGW-w64 toolchain bundled with the configured
rustup Windows GNU target. The native runtime provides scalar printing,
string operations and checked-arithmetic panic exits. Native mode rejects
unsupported list/map values and builtin operations with E6002. Native string
concatenation allocates with the process heap and is not reclaimed. The CLI's
`--release` flag does not currently alter generated Cranelift optimization
settings.

Native executable generation is configured/tested only for
`x86_64-pc-windows-gnu`. No other host/architecture target is claimed.

## 6. Experimental ION GUI slice

Only the following native GUI operations are supported and documented:

- `gui_window(title, width, height)` creates a Win32 top-level window.
- `gui_button(window, id, text, x, y, width, height, handler)` creates a
  standard Win32 push button.
- `gui_run(window)` shows the window and runs the thread's Win32 message loop.
- `gui_close(window)` destroys the window.

The source-level handler must be a statically named Lutetium function taking
one `window` and returning `unit`. Cranelift passes its function address using
the Windows x64 ABI. The runtime maps `(parent HWND, child HWND)` to that
callback; a `WM_COMMAND/BN_CLICKED` message identifies the actual notifying
button by `lParam`. The callback is invoked after releasing the registry lock,
so it may destroy its parent.

The native integration test builds and launches a GUI-subsystem executable,
checks its PE subsystem field, finds two native buttons that intentionally
share one 16-bit ID, sends `BM_CLICK` to each, and confirms the correct
callback behavior and clean process exit. This validates the control
notification/callback path, not physical mouse input. Other controls and GUI
builtin names remain scaffolding unless separately tested and documented.

This is a single-window/single-message-loop prototype. `WM_DESTROY` posts
thread-wide `WM_QUIT`; independent simultaneous window loops are not
implemented. IDs are clamped to the Win32 16-bit range, so callers should use
unique in-range IDs. The callback registry distinguishes HWNDs despite
colliding IDs, but other Win32 ID-based behavior may still collide. Other GUI
builtin names in shared metadata/backend/runtime are scaffolding, not API.
There is no custom renderer, layout engine, keyboard events, reactive UI
state, DPI-aware coordinates, or general component framework.

## 7. CLI commands

`lutetium` implements `run`, `check`, `build`, `inspect`, `explain`, `help`
and version reporting. `build` supports `bytecode`, `native`, and `gui`
targets. `gui` emits an executable; it does not launch the app. See the README
and CLI source for exact argument forms.
