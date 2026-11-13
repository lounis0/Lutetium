# BYTECODE_FORMAT.md

Container magic `LTBC`, format version 3 (`FORMAT_VERSION` in
`crates/vm/src/bytecode.rs`). Version 2 added the `List` return-type tag and
the BUILTIN opcode; version 3 adds the `Map` return-type tag and the map
builtin kinds. All integers are little-endian. Readers must reject
unknown tags/opcodes; nothing is executed before verification.

## Container layout

```text
magic            4 bytes  "LTBC"
version          u16
const_count      u32
constants[]      tag u8:
                   1: i64
                   2: f64
                   3: u32 byte-length + UTF-8 bytes
                   4: u8 (0/1)
                   5: unit
func_count       u32
functions[]      name  (u32 len + bytes)
                   arity u8
                   nregs u16
                   ret tag u8  (BcTy: 1 int, 2 float, 3 bool, 4 str, 5 unit, 6 list, 7 map)
                   code_len u32
                   code[]  (instruction stream)
entry_name       (u32 len + bytes) — looked up in the function table
```

## Instruction encoding

Every instruction: opcode u8 followed by a fixed operand pattern:

| Operand pattern | Encoding |
|---|---|
| Const | u32 constant index, u16 dst register |
| Unit  | u16 dst register |
| Move  | u16 src, u16 dst |
| Ternary (arith/cmp/concat) | u16 a, u16 b, u16 dst |
| Binary (unary ops) | u16 a, u16 dst |
| Sink (PRINT) | u16 src |
| Branch (JUMP_IF_FALSE) | u16 cond, i32 absolute target |
| Jump (JUMP) | i32 absolute target |
| Call | u16 func index, u8 argc, argc x u16 argument registers, u16 dst |
| Builtin | u8 kind, u16 dst, u8 argc, argc x u16 argument registers |
| Ret | u16 src |
| RetUnit | — |

## Opcode table

```text
0x01 CONST_INT   0x02 CONST_FLOAT 0x03 CONST_BOOL 0x04 CONST_STR
0x05 CONST_UNIT  0x06 MOVE
0x08 ADD_INT  0x09 SUB_INT  0x0A MUL_INT  0x0B DIV_INT  0x0C MOD_INT
0x0D ADD_FLOAT 0x0E SUB_FLOAT 0x0F MUL_FLOAT 0x10 DIV_FLOAT
0x11 INT_TO_FLOAT  0x12 CONCAT_STR
0x14 EQ_INT 0x15 NE_INT 0x16 LT_INT 0x17 LE_INT 0x18 GT_INT 0x19 GE_INT
0x1A EQ_FLOAT 0x1B NE_FLOAT 0x1C LT_FLOAT 0x1D LE_FLOAT 0x1E GT_FLOAT 0x1F GE_FLOAT
0x20 EQ_STR 0x21 NE_STR 0x22 EQ_BOOL 0x23 NE_BOOL
0x24 NOT 0x25 NEG_INT 0x26 NEG_FLOAT
0x27 JUMP  0x28 JUMP_IF_FALSE
0x29 CALL  0x2A RET  0x2B RET_UNIT  0x2C PRINT  0x2D BUILTIN
```

### BUILTIN kinds (`BuiltinKind` in `crates/vm/src/bytecode.rs`)

Kind numbers are stable. The static signature table (`sig()`) and the checker
(`lu-compiler`) must agree; the verifier enforces the arity.

```text
0x01 list_new   (elements...)      0x10 str_len      (string)
0x02 list_len   (list)             0x11 str_upper    (string)
0x03 list_push  (list, value)      0x12 str_lower    (string)
0x04 list_pop   (list)             0x13 str_trim     (string)
0x05 list_get   (list, index)      0x14 str_contains (string, needle)
0x06 list_set   (list, index, val) 0x15 str_replace  (string, from, to)
                                   0x16 str_split    (string, separator)
0x20 abs_int   (int)               0x26 sqrt   (number)
0x21 abs_float (number)            0x27 pow    (a, b)
0x22 min_int   (int, int)          0x28 floor  (number)
0x23 min_float (number, number)    0x29 ceil   (number)
0x24 max_int   (int, int)          0x2A round  (number)
0x25 max_float (number, number)    0x2B sin    (number)
                                   0x2C cos    (number)
                                   0x2D tan    (number)
                                   0x2E log    (number)
0x30 read_text   (path) -> str
0x31 write_text  (path, content) -> unit
0x32 file_exists (path) -> bool
0x40 map_new  (key, value, ...)   even argc; interleaved (key, value) pairs;
                                  keys must be strings; a later duplicate
                                  key overwrites the earlier one
0x41 map_get  (map, key)          runtime E5009 when the key is absent
0x42 map_set  (map, key, value)   inserts or overwrites; unit result
```

Source-level names: `sqrt`, `pow`, `floor`, `ceil`, `round`, `abs`, `min`,
`max`, `sin`, `cos`, `tan`, `log`, `read_text`, `write_text`,
`file_exists`. `abs`/`min`/`max` are overloaded: all-int arguments select the
int variants, otherwise the float variants. Methods (`l.push(v)`,
`s.upper()`, `m.get("k")`, ...) resolve through the checker to the same kinds
with the receiver passed as the first argument.

## Execution model

Register machine: each frame owns `nregs` registers. Parameters arrive in
`r0..r{arity-1}`. Locals are registers assigned by the compiler; temporaries
live above them. `CALL` copies argument registers into the callee frame; `RET`
writes the value into the caller's designated destination register.

## Verification contract

`lu-vm::verify` checks, per function:

* opcodes are known and operand-shaped,
* constant indices, function indices and jump targets are in range,
* `CALL` arity matches the callee's arity,
* every register is written before it is read on the verifier's explored
  paths (dataflow fixpoint; parameters are pre-defined),
* control flow cannot fall off the end.

Registers the verifier cannot prove are still safe at runtime: the VM fills
frames with an "uninitialized" value and raises a clean E5004 if one is read.
