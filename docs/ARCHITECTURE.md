# Lutetium architecture (0.4.2)

## Source-to-execution pipeline

```text
.lu source
  -> lexer/parser and AST (lu-syntax)
  -> resolver, type checker, typed register IR (lu-compiler)
  -> optimizer (IR levels 0–2; level 3 currently aliases level 2)
       -> bytecode codegen -> LTBC v5 -> verifier -> register VM (lu-vm)
       -> Cranelift COFF -> Windows GNU linker + embedded runtime
            -> console executable (`--target native`)
            -> GUI-subsystem executable (`--target gui`, ION prototype)
```

`compile_ir_with_opts` is the shared compiler path used by both bytecode and
native compilation. `compile_source` continues through bytecode codegen. CLI
`run` verifies bytecode before VM execution; bytecode `build` verifies before
serialization. Native targets compile the typed IR directly, so their safety
and semantics depend on the compiler's IR invariants and backend checks.

## Workspace crates

- `lu-diagnostics` — source spans, human-readable and JSON diagnostics.
- `lu-syntax` — lexer, recursive-descent parser, AST and type syntax.
- `lu-vm` — LTBC constants/types/opcodes, serialization, verifier, register
  interpreter, list/map heap and generational tracing collector.
- `lu-compiler` — resolver, typechecker/inference, typed IR, lowering,
  optimizer, bytecode codegen and compiler pipeline.
- `lu-native` — Cranelift x64 Windows GNU object generation and MinGW linking.
- `lutetium-cli` — `run`, `check`, `build`, `inspect`, `explain`, version/help.
- `crates/runtime` — standalone Rust runtime compiled as a static library by
  `crates/native/build.rs` and embedded into the CLI/native compiler artifact.

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
and links it with the object and rustup's self-contained MinGW-w64 components.
The generated executable has no runtime dependency on the Lutetium source tree.

Console native builds support the scalar operations/runtime calls implemented
by Cranelift and reject unsupported types/builtins with E6002. Native string
concatenation uses process-heap allocation and is not reclaimed. The CLI's
`--release` flag does not currently change generated Cranelift optimization
settings. The configured and tested native target is only
`x86_64-pc-windows-gnu`.

## Experimental ION GUI slice

The only advertised GUI source surface is `gui_window`, `gui_button`,
`gui_run`, and `gui_close` on `--target gui`. The runtime creates one native
Win32 top-level window and standard push button, routes `WM_COMMAND` /
`BN_CLICKED` for the notifying child HWND to a statically resolved Lutetium
function pointer, and can destroy the window from that callback.

The Windows integration test launches the executable, sends `BM_CLICK` to a
real button control, and observes callback-driven process shutdown. It does not
inject physical mouse input. Window destruction posts thread-wide `WM_QUIT`,
so this is a single-window/message-loop prototype, not independent multiwindow
management. Control IDs are 16-bit Win32 IDs and callers should use unique
in-range IDs even though dispatch distinguishes child HWNDs. Other GUI builtin
identifiers and runtime declarations are scaffolding, not supported API.

No component tree, custom drawing, general control system, layout, keyboard
handling, reactive state, DPI-aware scaling, or cross-platform GUI backend is
implemented. See [`LANGUAGE_SPEC.md`](LANGUAGE_SPEC.md),
[`STANDARD_LIBRARY.md`](STANDARD_LIBRARY.md), and
[`../audit/TEST_RESULTS.md`](../audit/TEST_RESULTS.md).
