# RUNTIME_ARCHITECTURE.md — Milestone A audit + shared memory design

## 1. Bytecode VM (audited implementation, `crates/vm`)

* **Values** (`vm::Value`): `Uninit | Int(i64) | Float(f64) | Bool(bool) |
  Str(Rc<str>) | Unit`. Registers are the operand storage; there is no
  separate operand stack (register machine).
* **Registers/frames**: one `Vec<Value>` per call frame, sized `nregs`.
  Locals occupy `r0..r{nlocals-1}`, temporaries above. `CALL` copies argument
  registers into the callee frame; `RET` writes the caller's destination
  register. Frames are a `Vec<Frame>`; depth capped at 1024.
* **Strings**: `Rc<str>` — reference-counted, immutable, shared by clone.
  Rc cannot form cycles, so strings alone cannot leak; mutable containers
  (introduced in 0.4) are what need tracing.
* **Heap allocation today**: none of Lutetium's own; string payloads come from
  the Rust allocator via `Rc`. Runtime errors: E5001–E5005 (stack overflow,
  div/mod zero, corrupt bytecode, type error, overflow), each a clean
  `VmError` with function/offset context.
* **Globals**: none yet (top-level statements compile into the
  `__lutetium_init` entry function with local slots).
* **Temporaries**: registers, single-assignment per statement; the verifier
  proves read-before-write.

## 2. Native runtime (audited implementation, `crates/native` + `crates/runtime`)

* **ABI**: all functions use `CallConv::WindowsFastcall`; the module entry is
  emitted as `main` with the `int main(void)` signature.
* **Values**: `int`=i64, `float`=f64, `bool`=i8, `str`=i64 pointer to
  `[len: u64][bytes..]`, `unit`=no value.
* **Runtime functions** (`crates/runtime`, `extern "system"`): print_i64 /
  print_f64 / print_bool / print_str, concat (HeapAlloc), str_eq, panic_div0,
  panic_overflow. The staticlib is compiled once by `build.rs` and embedded
  in the compiler; `link_exe` links it with every generated object.
* **Allocation**: `HeapAlloc(GetProcessHeap(), ...)`; **nothing is reclaimed
  yet** — strings/concat results leak by design until the GC milestone
  (documented in DEVELOPMENT_REPORT 0.3.0).
* **Roots**: none exist yet in native mode. Precise native GC requires
  compiler-generated root maps / explicit registration — designed in §3,
  implemented in a later milestone.
* **Dependencies of generated exes**: Windows OS only (kernel32/msvcrt via
  import libs); no compiler or source-tree dependency.

## 3. Shared memory model (design implemented in 0.4 for the VM)

Principle: **immutable values are cheap and cannot form cycles; mutable
containers can, and get a precise tracing collector.**

| Value kind | VM representation | Native representation |
|---|---|---|
| int/float/bool/unit | immediate in register | immediate in register |
| string | `Rc<str>` (refcounted, immutable) | pointer to `[len][bytes]` header |
| list | GC heap object (traced) | `HeapAlloc` `[len][8-byte slots]` (leaks until native GC) |
| map | GC heap object (traced) | not yet (needs heterogeneous value design) |
| struct/closure/tensor | future: GC heap objects | future: GC heap objects |

* **Object model**: `Heap` slot table; each slot holds `Option<GcObject>`
  (`List(Vec<Value>)` or `Map(Vec<(Value, Value)>)`), a mark bit, an age
  counter, a generation tag, and an approximate byte size. Handles
  (`Ref(u32)`) index the slot table, so references never move: promotion
  between generations and sweeping never invalidate handles.
* **Root discovery (VM, precise)**: at any allocation point the VM owns all
  live values in `frames[].regs` — the GC receives every frame's register
  vector plus container contents are reached through the heap itself. No
  stack scanning: roots are exact by construction.
* **Generations**: young generation (allocation target), old generation.
  Minor collection: mark from roots + remembered set, sweep dead young slots,
  promote survivors (age ≥ 2). Major collection: full mark-sweep over both
  generations. **Write barrier**: every mutation of an old container with a
  young value records the old slot in a sticky remembered set.
* **Statistics**: allocations, live slots/bytes, minor/major collections,
  promotions; exposed via `lutetium run --gc-stats`; `--gc-stress` collects
  on every allocation.
* **Native GC**: deferred. Requires generated root maps (each function's
  frame slots that may hold references) or shadow-stack registration; the
  VM's design carries over once that exists. Native lists documented to leak
  until then. Language-visible semantics (bounds checks, error codes) are
  identical in both backends.

## 4. Method calls and builtins (0.4)

* Front end maps `recv.method(args)` and `recv[i]` through a checker-side
  builtin table keyed by the receiver's static type; lowering emits a single
  `Builtin { kind, args, dst }` IR instruction.
* Bytecode: one `BUILTIN` opcode + a kind table (argument count, dst written)
  shared with the verifier; the VM dispatches at run time.
* Native: the backend currently rejects modules that use lists or built-in
  calls (`check_supported` in `crates/native/src/backend.rs`, reported as
  E6002 with a "run with the bytecode VM instead" message). Per-kind native
  runtime imports are the planned follow-up; scalar-only programs compile
  and run natively unchanged.
