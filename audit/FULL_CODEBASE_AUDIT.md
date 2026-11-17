# Historical Lutetium full codebase technical audit

> **Historical snapshot, superseded.** This report describes an earlier pre-GUI working-tree state and is retained for provenance only. It is not current project status: since then the Windows ION prototype was implemented and verified, Git state was established, and current results/inventory are in [`CURRENT_WORKTREE_AUDIT.md`](CURRENT_WORKTREE_AUDIT.md), [`ARCHITECTURE.md`](ARCHITECTURE.md), [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md), and [`TEST_RESULTS.md`](TEST_RESULTS.md). Do not use the stale capability claims below as current facts.

**Original audit date:** 2026-11-12 (working tree snapshot). The original environment reported the project directory was not a Git repository; that statement is historical and no longer current.

Companion reports:

- [`REPOSITORY_INVENTORY.md`](REPOSITORY_INVENTORY.md) — file-by-file inventory and package dependencies.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate, compilation, VM, GC, native, GUI Mermaid diagrams.
- [`TEST_RESULTS.md`](TEST_RESULTS.md) — actual commands/results.
- [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) — verified failures, GUI feature matrix, risk observations.
- [`ROADMAP.md`](ROADMAP.md) — prioritized and dependency-aware work plan.

The original audit command results were collected before release-preparation cleanup. Cleanup did not change compiler, VM, native runtime, or test source beyond the prior GC test helper; scratch example moves and generated-file removals are recorded in `FINAL_REPOSITORY_REPORT.md`.

## Executive assessment

Lutetium is a Rust workspace implementing a compact statically typed language with a recursive-descent frontend, typed register IR, bytecode compiler, verifier, register VM, list/map heap and generational collector, CLI, and an x86_64 Windows GNU Cranelift backend for scalar console programs. This is evidenced in source and historical native integration tests, but the current workspace cannot be built due to incomplete GUI IR integration. Two compiler end-to-end tests also fail when that crate is tested in isolation.

The current GUI work is not a GUI implementation. It is compiler/bytecode/type scaffolding and Win32 declarations plus a linker subsystem flag. There is no actual Win32 window creation call, renderer, text drawing, shape drawing, message procedure, event loop, input handling, callback trampoline, layout engine, DPI support, resource lifecycle, GUI demo or GUI test. No Direct2D/DirectWrite/DWM subsystem exists. The native backend's instruction match does not compile with the new `GuiButton` IR (`crates/native/src/backend.rs`, translator match). The VM has no Window/Widget values and explicitly returns “GUI functions are available only in native Windows GUI executables” (`crates/vm/src/vm.rs`, `exec_builtin`). The CLI cannot request the GUI build helper (`crates/cli/src/lib.rs`, `cmd_build`).

**Current capability summary:**

- Bytecode programs: intended and broadly implemented; 113/115 compiler e2e passed in this audit, but two failed tests remain.
- VM GC: implemented for VM list/map objects and 8 heap tests passed.
- Native console executables: scalar compiler/runtime/linker code and existing end-to-end tests exist, but tests were not buildable in this current tree because of GUI IR compile error.
- Native GUI applications: not currently achievable.
- Cross-platform native executables: not implemented; fixed Windows x64 GNU path.

## 1. Repository inventory and language architecture

Complete file/directory inventory is in `REPOSITORY_INVENTORY.md`. At a subsystem level:

### Lexer

`crates/syntax/src/lexer.rs` defines `Tok` and `Token{tok,span}` and `lex(&str) -> (Vec<Token>, Vec<Diagnostic>)`. It preserves newline separators (collapsing repeated blank lines), ignores `#` through newline, recognizes ASCII numeric forms with `_` separators, basic decimal floats, single-line quoted strings with six escapes (`\n`, `\t`, `\r`, `\0`, `\\`, `\"`), identifiers/keywords, delimiters, ranges, assignments, arithmetic/comparisons/short-circuit logic. It emits E1001 for unknown characters/escapes/unterminated strings and E1101 for int overflow. It does not support exponent notation, chars, multiline strings, interpolation, Unicode identifier policy beyond `char::is_alphabetic`/alphanumeric, bitwise operators, or import syntax. Lexical errors return tokens too, but pipeline rejects if any lexer diagnostic exists.

### Parser and AST

`crates/syntax/src/parser.rs` constructs `Module{funcs,top}` and uses recursive descent by precedence. It supports functions (optional parameter annotations and return annotation), blocks, let/let-mut, assignment/compound assignment, if/else-if, while, range `for`, list `for-in`, return/break/continue, top-level statements, if expressions, calls, list/map literals, indexing, method and field postfix. E2001 syntax and E2002 expression diagnostics are emitted with spans; parser synchronizes to statement boundaries and module parser has forward-progress guard. It currently fails the existing function-call iterable test (`for x in mk()` → E2001), so parser capability should not be claimed broadly without this caveat. AST owns children using `Vec`, `Box`, and `String`; source expressions/statements retain spans. The only lambdas/closures, declarations with globals, modules/imports, structs/enums, generic types, pattern matching, exceptions, and arbitrary UI-builder blocks are absent. The UI example with nested declarative component/callback blocks is illustrative only and is not the language grammar.

`TypeKind` includes primitive kinds plus current GUI `Window/Widget` annotations, but `docs/GRAMMAR.ebnf` and language/error docs do not reflect those additions.

### Typechecker and resolver

`crates/compiler/src/resolve.rs` builds function signatures, checks duplicates and entrypoint, converts annotations. It recognizes Window/Widget through GUI scaffolding. Resolver validates `main` only for no params and annotated return is unit; no module namespaces.

`crates/compiler/src/typecheck.rs` runs a bounded fixpoint (roughly number of functions + 2, min 3) for inferred return and unannotated parameter types. `CTy` represents scalar bytecode types, list element type, record map field sets/types, and `Unknown`. It applies int→float widening, typechecks locals with nested `HashMap` scopes and mutability, resolves function names/calls, methods/free builtins, loops, conditions, returns, list/map constraints, and stable diagnostic codes. List literals must have a single homogeneous scalar element type except int→float widening; empty lists cannot infer type; nested collections are rejected. Maps are record-like, identifier-keyed, per-key typed scalar values, dynamic map key methods are rejected; duplicate key type sorting behavior has special later-wins logic. Function return analysis is tail/statement oriented and not a full control-flow analysis.

GUI checker work is partial: `gui_button` special-case expects seven arguments and last arg a named handler typed `(widget)->unit`; callback builtins resolve named callback symbols; GUI free builtins are added in param/result maps but label/button/key signatures are inconsistent. The source does not show a complete public GUI API with consistent type rules. No negative/positive GUI checker tests.

## 2. Compiler architecture and source-to-executable

Pipeline in `crates/compiler/src/pipeline.rs`: `lex → parse → resolve → check → lower → optimize`. `compile_ir_with_opts` outputs typed IR; `compile_source` calls bytecode codegen. IR has explicitly numbered registers, typed function signatures, blocks and structured jump/branch/return terminators. Lowerer maps source locals to register slots and allocates temps; short circuit and conditional values become blocks; loops become branches/counters. The optimizer is real but bounded: level 1 folding/unreachable blocks; level 2 dead temporary and block-local CSE; level 3 same as level 2.

Bytecode codegen emits constants, math/compare ops, jumps, calls, print and Builtin operands. Bytecode format uses LTBC magic; current source `FORMAT_VERSION=5`. `docs/BYTECODE_FORMAT.md` remains version 3, hence stale. GUI-only `IrInstr::GuiButton` currently triggers `unreachable!` in bytecode codegen, making CLI `check` (which calls compile_source) and bytecode build unsafe if GUI checker output is reached. A compiler should return a normal unsupported-target diagnostic instead of panic.

Native backend in `crates/native/src/backend.rs` lowers IR to Cranelift x64, emits COFF and calls runtime imports. It implements scalar constants, arithmetic/comparisons, strings, function calls, print, and guards checked integer arithmetic. `check_supported` rejects list/map types and all Builtin instructions before translation. Only `x86_64-pc-windows-gnu` is configured/selected. Current GUI instruction makes Rust compile fail before any native command can be run. The CLI `--release` is passed to `build_native` but doesn't change Cranelift settings; `--opt` is implemented at IR stage; cargo release profile concerns the compiler binary.

## 3. Bytecode VM and memory management

`crates/vm/src/bytecode.rs` defines scalar/container/gui bytecode return tags, arithmetic/branch/call/return/print and BUILTIN plus equality ops. Builtin kinds cover list, string, math, filesystem, map and GUI names. Current `sig()` is central for VM verifier arities, but checker has separate signature tables. Serialization encodes constants, functions and entry name; deserialization checks magic/version, bounded reads and UTF-8. It requires exact version; v5 current source, docs stale.

`crates/vm/src/verify.rs` decodes shapes, validates bounds/calls/builtin arity/jumps/register usage and control flow. Dataflow merges written registers by union, so can only prove no register is never written on explored paths; `Value::Uninit` runtime check handles some path-dependent cases. Verifier is frontend-independent as intended.

`crates/vm/src/vm.rs` dispatches bytecode using `Vec<Frame>` with one `Vec<Value>` register bank each; recursion limit 1024; string `Rc<str>`; list/map `u32` heap handles. Arithmetic checks int overflow and division/mod by zero; strings/list/map operations and builtins execute in VM. Runtime errors include E5001–E5009 by source, though registry/explain docs incomplete. Output callback receives one newline-terminated print string. GUI BuiltinKinds all return a runtime error.

`crates/vm/src/heap.rs` implements a generational collector: Slot table, Young/Old, ages, remembered set, write barriers, minor/major collection, roots passed as all frame register vectors. Objects are mutable List/Map; trace clones child values into worklists; strings are Rc not traced. Eight unit tests passed in this audit. No standalone performance benchmarks were found. `Heap::get/get_mut` expect valid handles; malformed internal references panic rather than return VmError. GC is implemented only in VM; native string concat uses process HeapAlloc and leaks.

## 4. Native runtime / linker / platform

`crates/runtime/src/lib.rs` imports `kernel32` APIs `GetStdHandle`, `WriteFile`, process heap allocation and `ExitProcess`, and exports scalar print, string concat/eq, div0/overflow panic functions as Windows system ABI. Strings use pointer-to-length-header representation. Unsafe code trusts pointers/lengths. New GUI FFI declarations in user32 include class/window/message/text APIs and struct layouts, but are unused. No Direct2D/DirectWrite; no window creation/drawing/event calls.

`crates/native/build.rs` builds the runtime staticlib from source with plain rustc and embeds it. `linker.rs` locates rustup MinGW, passes crt2 and system libs and chooses console or windows subsystem. GUI helper is not connected through CLI and cannot produce complete UI behavior. Current native target support is Windows x64 GNU only. Windows x64 MSVC, Windows ARM64, Linux, macOS, Android, iOS and WASM are not actually supported by this native pipeline.

## 5. Standard library and CLI

There is no importable stdlib module tree. `docs/STANDARD_LIBRARY.md` documents checker/VM builtins: math (`sqrt`, `pow`, `floor`, `ceil`, `round`, trig, `log`, overloaded `abs/min/max`), file text (`read_text`, `write_text`, `file_exists`), list/string/map methods and print. Implementations live mainly in checker kind maps and VM `exec_builtin`; native backend rejects builtins. GUI names were added to builtin tables but have no runtime implementation. The standard-library documentation excludes GUI and contains stale string length/split details relative to source (`StrLen` counts Unicode chars, not bytes; `StrSplit` takes separator). No `import` syntax/system.

CLI `crates/cli/src/lib.rs`: run/check/build/inspect/explain/version. Run verifies bytecode then executes VM. Bytecode build writes `build/<stem>.lbc`; native build writes `build/<stem>.exe` plus object/runtime archive. GUI build target absent. CLI supports GC stats/stress. Error explanations are manually authored and stale.

## 6. Testing and reliability

Full measured outputs and coverage are in `TEST_RESULTS.md`. Summary: `cargo test --workspace` and workspace build fail at E0004 native match; syntax 24/24; diagnostics 0 tests; VM 8/8; compiler e2e 113/115; CLI/native tests not compilable. `cargo fmt --all -- --check` fails on several GUI-related changes and compiler test formatting. No GUI tests or examples. Historical 149-pass assertion in `CURRENT_STATE.md` is not current proof.

## 7. Dependencies and performance

Direct external dependencies only serve Cranelift native backend/target selection. Frontend/VM crates use standard library plus internal dependencies. Cargo lock has Cranelift 0.120.2 transitive stack (object/serde/hashbrown/allocator/codegen helpers). Workspace build requires network/cache for crates.io on first build and rustup target/toolchain MinGW driver for native linking. No MSVC requirement per configured toolchain.

No benchmarks are present. Performance claims should therefore be limited: VM executes a bytecode dispatch loop and clones some `Value`s; map lookups are linear; GC clones object children during tracing; native build incurs Cranelift translation and external linker process; generated native programs use system runtime and process heap. Measured artifacts: local portable exe about 19 MB; zip about 6.2 MB; checked-in `examples/build/hello.exe` 4.7 MB, runtime archive 15 MB (these are specific existing build artifacts, not general size guarantees).

## 8. Security/stability

Compiler and VM are mostly safe Rust. Unsafe code is concentrated in native runtime ABI and pointer operations. The VM has runtime type/bounds errors and arithmetic checks; parser/bytecode public entry points have assumptions (non-empty parser token vector, valid `BytecodeModule.entry` in serializer, valid heap handles). The language's file builtins can read/write arbitrary paths available to process; this is ordinary process authority and should be documented if a sandbox is intended. GUI host does not yet exist, so no event callback isolation/resource safety can be assessed.

## 9. Production readiness

- **Usable conceptually but currently build-blocked:** compiler frontend, bytecode VM, maps/lists/GC, CLI. A green build/test baseline is required before release claims.
- **Experimental/platform-specific:** scalar Cranelift native backend and Windows runtime/linker. Tests exist and historically launch generated executables, but current native/CLI suites cannot compile because of GUI IR match change.
- **Incomplete:** documentation/version consistency, GUI types/builtin scaffolding, GUI linker subsystem helper.
- **Missing:** GUI framework/rendering/event loop/layout/components/state/DPI, cross-platform native backends, imports/package manager, advanced language features, benchmark suite.

See `ROADMAP.md` for prioritized unblock sequence and `KNOWN_ISSUES.md` for individual findings.
