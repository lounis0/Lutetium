# Lutetium architecture (0.4.3)

## Source-to-execution pipeline

```text
.lu source
  -> lexer/parser and AST (lu-syntax)
  -> resolver, type checker, typed register IR (lu-compiler)
  -> optimizer (IR levels 0–2; level 3 currently aliases level 2)
       -> bytecode codegen -> LTBC v5 -> verifier -> register VM (lu-vm)
       -> Cranelift COFF -> Windows GNU linker + embedded runtime
            -> console executable (`--target native`)
            -> GUI-subsystem executable (`--target gui`, legacy GUI prototype)
```

`compile_ir_with_opts` is the shared compiler path used by both backends.
CLI `run` verifies bytecode before VM execution; bytecode `build` verifies
before serialization. Native targets compile typed IR directly, so their
semantics depend on compiler IR invariants and backend checks.

## Workspace crates

- `lu-diagnostics` — source spans and human-readable/JSON diagnostics.
- `lu-syntax` — lexer, recursive-descent parser, AST and type syntax.
- `lu-vm` — LTBC constants/types/opcodes, serialization, verifier, register
  interpreter, list/map heap and generational tracing collector.
- `lu-compiler` — resolver, typechecker/inference, typed IR, lowering,
  optimizer, bytecode codegen and compiler pipeline.
- `lu-native` — Cranelift x64 Windows GNU object generation and MinGW linking.
- `lutetium-cli` — `run`, `check`, `build`, `inspect`, `explain`, version/help.
- `crates/runtime` — standalone Rust runtime compiled as a static library by
  `crates/native/build.rs` and embedded into native executables.
- `lutetium-ion` (`crates/ion`) — independent version 0.5.0 Rust UI library,
  Windows GDI backend and native examples. It does not change the language GUI
  ABI.

## Frontend and typed IR

The frontend resolves top-level function names and entry points, then checks
scoped variables, inferred parameter/return types, control flow, builtins,
record-like maps and homogeneous lists. Only int-to-float widening is implicit.
The backend-independent IR uses explicit typed registers, basic blocks and
jump/branch/return terminators. Lowering shares this IR between bytecode and
Cranelift backends.

## Bytecode VM and verification

LTBC v5 stores constants, function signatures/register counts/return tags/code,
and an entry function name. The bytecode uses fixed-width little-endian
operands and a register-machine execution model. The verifier checks opcode
shapes and ranges, call/builtin arity, branch targets, register references and
read-before-write along its explored dataflow. Its register analysis merges
written sets by union, not must-intersection; the VM's uninitialized register
value is an additional runtime check for path-dependent reads. See
[`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md) for the exact container encoding.

The VM executes scalar values and builtins plus mutable list/map objects on a
precise generational tracing heap. Roots are registers in all live VM frames;
list/map mutations use a write barrier. GUI builtins are explicitly
unavailable in bytecode execution.

## Native Windows x64 GNU backend

The native backend translates typed IR to Cranelift WindowsFastcall functions,
then emits COFF. The runtime static library is built from `crates/runtime` and
embedded in the compiler binary; the linker materializes it beside the output
and links it with rustup's self-contained MinGW-w64 components. Generated
executables have no runtime dependency on the Lutetium source tree.

Console native builds support a subset of scalar operations/runtime calls and
reject unsupported types/builtins with E6002. Integer arithmetic, including
signed division, remainder and negation edge cases, is checked. Native string
concatenation uses process-heap allocation and is not reclaimed. The CLI's
`--release` flag does not currently change generated Cranelift optimization
settings. The configured and tested native target is only
`x86_64-pc-windows-gnu`.

## GUI projects

There are two separate experimental GUI APIs.

### Lutetium language GUI

The Lutetium language surface is `gui_window`, `gui_button`, `gui_run`, and
`gui_close` on `--target gui`. It creates a native Win32 window and standard
push button, routes `WM_COMMAND`/`BN_CLICKED` for the notifying child HWND to a
statically resolved Lutetium callback, and can destroy the window from that
callback. The VM explicitly reports GUI builtins as unavailable. This is a
single-window/message-loop prototype without state, layouts, or custom
rendering.

The Windows integration test launches a compiled GUI executable, sends
`BM_CLICK` to a real button, and observes callback-driven process shutdown. It
does not inject physical mouse input. Window destruction posts thread-wide
`WM_QUIT`; independently managed multiple windows are not supported.

### Separate Rust ION UI crate

`lutetium-ion` 0.5.0 provides a platform-neutral component tree, stack layout,
semantic themes, events/state, backend-neutral draw commands, and starter
controls. Its Windows backend uses Direct2D/DirectWrite with a GDI fallback,
per-monitor DPI handling and component-bounded text clipping; examples include
a calculator and component gallery. This API is Rust-only and does not alter
Lutetium syntax or the language GUI ABI. It does not currently provide
non-Windows backends or native accessibility integration. See
[`ION_0.5.md`](ION_0.5.md).
