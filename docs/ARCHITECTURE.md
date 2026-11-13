# ARCHITECTURE.md

## Pipeline

```text
Source (.lu)
    |
    v
Lexer (crates/syntax)
    |
    v
Parser -> AST (crates/syntax)
    |
    v
Resolver: function table, duplicates, entry point (crates/compiler::resolve)
    |
    v
Type checker: scoped bindings, inference, implicit int->float (crates/compiler::typecheck)
    |
    v
Lowering -> typed IR (crates/compiler::lower)
    |
    v
Bytecode codegen (crates/compiler::codegen)
    |
    v
.lbc  <->  Verifier (crates/vm)  ->  Register-machine VM
```

## Crate boundaries

* **lu-diagnostics** — `Span`, `Diagnostic`, `SourceFile` rendering, JSON
  output. No dependencies.
* **lu-syntax** — lexer, parser, AST. Depends on diagnostics.
* **lu-vm** — bytecode container (serialize/deserialize), verifier,
  interpreter. No dependencies: bytecode produced by any front end must be
  verifiable here.
* **lu-compiler** — resolver, type checker, lowering, IR, codegen, pipeline.
  Depends on diagnostics/syntax/vm.
* **lutetium-cli** — the `lutetium` binary (library + thin `main`) with
  integration tests.

## Key design points

### Typed IR

Backend-independent: functions with explicit register operands, basic blocks,
structured terminators (`Jmp`, `Br`, `Ret`, `RetUnit`). Locals occupy
`r0..r{nlocals-1}`; temporaries are allocated above. This is the surface the
future Cranelift backend (milestone 2) will lower from.

### Verification before execution

`lu-vm::verify` proves per function that opcodes are known and shaped,
operands in range, calls match arity, and no register is read before being
written on the verifier's explored paths. The VM additionally initializes all
registers to an "uninitialized" value so any residual path-dependent read is a
clean runtime error (E5004), never undefined behavior. `lutetium run/build/
inspect` always verify.

### Type system (0.1)

Static, with inference: `let` infers from the initializer; function return
types are annotated or inferred by a fixpoint across the function table.
The only implicit conversion is `int` -> `float` (never narrowing). See
`docs/LANGUAGE_SPEC.md`.

### Memory management

0.1 values are register-resident (`i64`, `f64`, `bool`, interned strings in
the constant pool / `Rc<str>`). The precise generational tracing GC applies
when heap objects (lists, maps, structs) land in a later milestone; the VM's
value representation was chosen to slot cleanly in front of it.

## Build configuration

`rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu`; `.cargo/config.toml`
forces the GNU target. The GNU toolchain links self-contained (rustup's
`rust-mingw` component), so MSVC/Visual Studio is never needed.
