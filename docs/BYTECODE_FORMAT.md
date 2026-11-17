# Lutetium bytecode format (LTBC v5)

The current container version is `FORMAT_VERSION = 5` in
`crates/vm/src/bytecode.rs`. The decoder requires an exact version match; it
does not migrate older files. Version history recorded by the implementation:

- **v2** added the list return-type tag and `BUILTIN` opcode.
- **v3** added map return-type tags and map builtin kinds.
- **v4** added deep list equality opcodes `EQ_LIST` and `NE_LIST`.
- **v5** added opaque `window`/`widget` return-type tags and GUI builtin kinds.

The v5 GUI kinds are metadata in the shared bytecode vocabulary. GUI operations
are currently supported only by the Windows native GUI target; the bytecode VM
returns an explicit unavailable error for them. Do not infer GUI VM support
from the presence of these kind tags.

All multi-byte integers are little-endian. Strings in the container are a
`u32` UTF-8 byte count followed by exactly that many UTF-8 bytes. A decoder
rejects bad magic, unsupported versions, truncated fields, invalid UTF-8,
unknown constant/type/builtin/opcode tags, malformed builtin arities, and
invalid executable instruction structure reported by the verifier. The current
container decoder does not reject trailing bytes after the entry name.

## Container layout

Fields occur in this order, with no padding:

```text
magic                 4 bytes: ASCII "LTBC"
version               u16: exactly 5
constant_count        u32
constants             constant_count entries (see tags below)
function_count        u32
functions             function_count entries (see function record below)
entry_function_name   string (u32 UTF-8 byte length + bytes)
```

Constant entries:

| Tag | Payload |
|---:|---|
| `1` | `i64` little-endian |
| `2` | `f64` IEEE-754 bit pattern, little-endian |
| `3` | string: `u32` byte length + UTF-8 bytes |
| `4` | one byte; serializer writes `0` or `1` (decoder interprets zero as false, nonzero as true) |
| `5` | no payload (`unit`) |

Function records:

```text
name          string (u32 byte length + UTF-8 bytes)
arity         u8
registers     u16 (nregs)
return_type   u8 (type tag below)
code_length   u32 (bytes)
code          code_length bytes
```

Return-type tags are `1=int`, `2=float`, `3=bool`, `4=str`, `5=unit`,
`6=list`, `7=map`, `8=window`, `9=widget`. List/map element or field types
are compiler type-checker information and are not encoded in these tags. The
entry point is stored by function name, not index; deserialization looks up
the first function with that name. Compiler-produced modules use a valid entry
index/name.

The container has no checksum, signature, compression, section table, or
forward-compatible unknown-field mechanism. Treat `.lbc` files as untrusted
input: deserialize and verify them before VM execution. Version equality is
strict, so a runtime supporting v5 rejects v4 and earlier containers rather
than guessing how to interpret their type/builtin tables.

## Instruction encoding

Each function's `code` is a byte stream. Register operands are `u16` register
indices. Constant and function indices are encoded as `u32` and `u16`
respectively. Branch destinations are signed `i32` absolute byte offsets from
the start of that function's code. There is no alignment or padding.

| Instruction family | Operand bytes after opcode |
|---|---|
| Constant (`CONST_INT/FLOAT/BOOL/STR`) | `u32 constant_index`, `u16 dst` |
| `CONST_UNIT` | `u16 dst` |
| `MOVE` | `u16 src`, `u16 dst` |
| Ternary arithmetic/comparison/concat/list-equality | `u16 a`, `u16 b`, `u16 dst` |
| Unary/conversion (`INT_TO_FLOAT`, `NOT`, `NEG_*`) | `u16 src`, `u16 dst` |
| `PRINT` | `u16 src` |
| `JUMP_IF_FALSE` | `u16 cond`, `i32 absolute_target` |
| `JUMP` | `i32 absolute_target` |
| `CALL` | `u16 function_index`, `u8 argc`, `argc * u16 arg_register`, `u16 dst` |
| `BUILTIN` | `u8 kind`, `u16 dst`, `u8 argc`, `argc * u16 arg_register` |
| `RET` | `u16 src` |
| `RET_UNIT` | no operands |

`CALL` copies arguments into the callee's parameter registers and writes its
return value to `dst`. `BUILTIN` writes a value to `dst`, including a dummy
unit value for side-effecting builtins. Branch targets must be in range;
compiler output targets instruction boundaries.

### Opcode assignments

```text
0x01 CONST_INT       0x02 CONST_FLOAT     0x03 CONST_BOOL      0x04 CONST_STR
0x05 CONST_UNIT      0x06 MOVE
0x08 ADD_INT         0x09 SUB_INT          0x0A MUL_INT         0x0B DIV_INT
0x0C MOD_INT         0x0D ADD_FLOAT        0x0E SUB_FLOAT       0x0F MUL_FLOAT
0x10 DIV_FLOAT       0x11 INT_TO_FLOAT    0x12 CONCAT_STR
0x14 EQ_INT          0x15 NE_INT           0x16 LT_INT          0x17 LE_INT
0x18 GT_INT          0x19 GE_INT           0x1A EQ_FLOAT        0x1B NE_FLOAT
0x1C LT_FLOAT        0x1D LE_FLOAT         0x1E GT_FLOAT        0x1F GE_FLOAT
0x20 EQ_STR          0x21 NE_STR           0x22 EQ_BOOL         0x23 NE_BOOL
0x24 NOT             0x25 NEG_INT          0x26 NEG_FLOAT
0x27 JUMP            0x28 JUMP_IF_FALSE    0x29 CALL            0x2A RET
0x2B RET_UNIT        0x2C PRINT            0x2D BUILTIN
0x2E EQ_LIST         0x2F NE_LIST
```

`EQ_LIST`/`NE_LIST` perform deep list equality in the VM, including numeric
int/float element widening; different list lengths compare unequal.

### Builtin kind assignments

The kind byte is the stable value returned by `BuiltinKind::kind_byte()`.
The verifier checks the argument count using `BuiltinKind::sig()` (with
variable-count exceptions for `list_new` and pairwise `map_new`). Argument
meaning and result types are checked by the compiler/runtime.

| Kind | Name | Arity |
|---:|---|---:|
| `0x01` | `list_new` | variable |
| `0x02`–`0x06` | `list_len`, `list_push`, `list_pop`, `list_get`, `list_set` | 1, 2, 1, 2, 3 |
| `0x10`–`0x16` | `str_len`, `str_upper`, `str_lower`, `str_trim`, `str_contains`, `str_replace`, `str_split` | 1, 1, 1, 1, 2, 3, 2 |
| `0x20`–`0x25` | `abs_int`, `abs_float`, `min_int`, `min_float`, `max_int`, `max_float` | 1, 1, 2, 2, 2, 2 |
| `0x26`–`0x2E` | `sqrt`, `pow`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `log` | 1, 2, 1, 1, 1, 1, 1, 1, 1 |
| `0x30`–`0x32` | `read_text`, `write_text`, `file_exists` | 1, 2, 1 |
| `0x40`–`0x42` | `map_new`, `map_get`, `map_set` | even/variable, 2, 3 |
| `0x50`–`0x56` | `gui_window`, `gui_label`, `gui_input`, `gui_button`, `gui_set_text`, `gui_run`, `gui_close` | 3, 6, 6, 7, 2, 1, 1 |
| `0x57`–`0x5E` | `gui_control`, `gui_get_text`, `gui_get_id`, `gui_on_click`, `gui_on_key`, `gui_text`, `gui_round_rect`, `gui_key_handler` | 2, 1, 1, 2, 2, 6, 8, 2 |

For `map_new`, arguments are interleaved `(key, value)` pairs; keys are
strings, duplicate keys use the later value. GUI `gui_button`'s bytecode kind
has seven value operands. The Lutetium source-level `gui_button` call takes an
eighth named-handler argument, which the compiler stores separately as a
statically resolved callback function index in native GUI IR; its native
Cranelift lowering passes that function address to the runtime. Bytecode
compilation remains structurally defined, but VM execution rejects GUI
builtins as unavailable.

## Register and verification model

Each function has `nregs` registers; parameters begin in `r0..r{arity-1}`.
The VM initializes registers as uninitialized, executes instructions in a
register frame, and reports a runtime error if an uninitialized value is
read. The verifier checks known opcode shapes, constant/function/register
indices, call and builtin arity, branch ranges, reachable control flow and
register reads-before-write along its explored dataflow. Its register merge is
a union analysis rather than a proof that each read is initialized on every
incoming path; the runtime uninitialized check remains a safety net. The CLI
verifies generated bytecode before writing it and verifies bytecode before VM
execution.
