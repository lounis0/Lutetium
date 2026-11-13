# Verified issues and limitations

Severity labels are audit triage, not security CVSS scores. All are based on source inspection or commands run on the audited working tree.

## Blocking / broken now

1. **Workspace does not compile — GUI IR exhaustiveness.** `crates/compiler/src/ir.rs` adds `IrInstr::GuiButton`; `crates/native/src/backend.rs` `FnTranslator::instr` match has no corresponding arm. `cargo build --workspace`, `cargo test --workspace`, `cargo test -p lu-native --no-run`, and `cargo test -p lutetium-cli` fail with Rust E0004 at `backend.rs:349`. This blocks native/CLI target and all tests depending on native.
2. **Bytecode codegen panics on GUI button IR.** `crates/compiler/src/codegen.rs` explicitly uses `unreachable!` for `IrInstr::GuiButton` (codegen around line 120). This is not a diagnostic path. If user GUI syntax reaches `compile_source`, the compiler can panic instead of returning an unsupported-target diagnostic.
3. **GUI runtime absent.** `crates/runtime/src/lib.rs` has Win32 extern declarations/constants/types only. No window procedure, `RegisterClassExW` / `CreateWindowExW` calls, drawing, event loop, callback dispatch, layout, DPI, or destruction implementation. VM GUI builtins return an error in `crates/vm/src/vm.rs`. No GUI executable can currently work.
4. **GUI callback lowering is incomplete/inconsistent.** `TExpr::GuiCallback` lowers callback index into integer args, but builtin `sig` GUI callback arities describe only `(window, callback)` (and one special GUI key handler is inconsistently named); `GuiKeyHandler` is not grouped in checker filter correctly if surface names don't match; no native callback dispatch exists. `GuiButton` `TExpr` carries handler index but `IR` carries args and handler; lowering even pushes then pops a handler temp, leaving dead emitted constant and bypassing callback transport. Even once exhaustiveness fixed, this is not a functional callback architecture.
5. **No GUI build target in CLI.** `crates/cli/src/lib.rs::cmd_build` accepts only `bytecode|native`, and native path calls `build_exe`, never `build_gui_exe`. Linker helper alone merely changes PE subsystem.
6. **Two end-to-end tests fail.** `for_in_iterable_evaluated_once` reports E2001 when the iterable is a function call. `list_equality_bool_str_elements` conflicts with homogeneous list typing and fails E3001. See `TEST_RESULTS.md`.

## GUI capability matrix (current tree)

| Capability | Classification | Evidence |
|---|---|---|
| Window creation | Missing | Win32 declarations only in runtime; no calls or WndProc. |
| Native text rendering | Missing | No Direct2D, DirectWrite, GDI drawing, text output APIs for a window, or render loop. |
| Rectangle / rounded rectangle rendering | Missing | `GuiRoundRect` enum/signature partial only; no runtime or IR rendering command. |
| Native controls | Missing | Names/signatures for label/input/button; no Win32 control creation. |
| Component model | Missing | No tree/node/component types or lifecycle. |
| Layout engine / resizing | Missing | No layout calculations or resize event handling. |
| Mouse events | Missing | No input message handling. |
| Keyboard events | Missing | Builtin name stubs only; no key dispatch.
| Callback execution in Lutetium | Placeholder | Checker resolves named callback indexes and IR stores one handler index; native runtime never invokes it. |
| UI state / reactive update | Missing | No state/runtime mechanism.
| Clean shutdown | Missing | `PostQuitMessage` declared but unused; no message loop or close path.
| DPI awareness | Missing | No DPI API or scaling logic.
| GUI PE subsystem linker option | Implemented but untested in app | `link_gui_exe` passes `--subsystem,windows`; no call sites reach a functioning GUI runtime and build currently fails. |
| GUI compilation | Broken | Missing native backend match arm; GUI bytecode path panics.
| GUI test executable | Missing | No GUI integration tests or `.lu` demo in tree.

No Direct2D/DirectWrite/DWM imports, crates, code, or tests were found.

## Other compiler/language issues

7. **Stale sources/docs contradict implementation.** `docs/ARCHITECTURE.md`, README, AGENTS, `ROADMAP.md`, `TASKS.md`, `CURRENT_STATE.md`, `ERROR_CODES.json`, `crates/cli/src/explain.rs`, `docs/LANGUAGE_SPEC.md`, `docs/BYTECODE_FORMAT.md`, and `docs/RUNTIME_ARCHITECTURE.md` have stale version statements/feature lists. Examples: format source is 5 while BYTECODE_FORMAT says 3; language docs say 0.4 scalar/list/map and omit Window/Widget; README says native planned in introductory text and 0.3 while Cargo says 0.4.1; error explain still calls E2005 method unsupported and E6001 native backend unavailable; current error registry lacks current E5009/E6002/E6003 entries, and EXPLANATIONS lacks E5009/E6002/E6003.
8. **Version/documentation drift.** `ERROR_CODES.json` says `language_version: 0.1.0`; runtime/bytecode source has versions 0.4.1/v5; old reports claim 149 green tests. Stale claims should not be relied upon.
9. **For-list parser limitation.** Parser takes `for` expression with `parse_add()` to distinguish range vs list. `for x in mk()` fails observed `for_in_iterable_evaluated_once`; call expressions require parsing through postfix/primary, but range-header strategy doesn't currently permit the call case.
10. **Homogeneous lists are actual contract.** Typechecker rejects distinct element types (except int→float widening) and rejects nested list/map values, while one test expects heterogenous bool/string list. This is either test/spec bug or intended-feature mismatch; decision needed before changing either.
11. **Function return checking is syntactic-tail based.** In `check_fn`, value-returning function requires `body.last()` to be `Return`, not control-flow proof that all paths return. Conservative/limited behavior; `if` exhaustiveness is not tracked.
12. **Builtin checker/evaluator can drift.** `BuiltinKind::sig()` and checker's parameter/return matches are maintained separately from VM `exec_builtin`; the historical str_split sig mismatch noted in changelog illustrates risk. GUI metadata already diverges.
13. **Bytecode container parsing lacks a trailing-byte rejection.** `deserialize` reads `entry_name` and returns without requiring reader position equals input length. Oversized count/string inputs can drive allocations/processing; `Reader::take` uses `self.p + n` which can overflow `usize` in adversarial extreme input (though practical byte slice length bounds this on 64-bit).
14. **Verifier's register analysis uses union, not must intersection.** Source comments acknowledge path-dependent uninitialized reads. Runtime `Uninit` handles cleanly, but “verifier proves register written on all paths” would be overstated. It rejects registers never written on any explored path, not reads written on only some incoming paths.
15. **Native function call-depth guard absent.** VM caps at 1024; native recursion can exhaust process stack. Existing docs mention this as deferred.
16. **Native runtime string format mismatch at extremes.** Native `format_f64` uses Rust Debug, VM uses Display plus `.0` append; cross-backend output may differ for extreme exponent/NaN/infinity formats.
17. **Native allocations leak.** Runtime concat uses `HeapAlloc`; no frees/GC/root maps. Console examples with many concatenations can grow process heap.
18. **Range loop overflow edge (exclusive max).** Inclusive loops stop on equality before increment, but an exclusive range with end `i64::MAX` and start below may increment beyond `i64::MAX`, producing checked overflow (native panic / VM E5005) after the last intended in-range iteration depending on condition/increment ordering. The exact lowerer has increment path after body before retest; no dedicated max exclusive test observed.
19. **`read_text` whole-file memory and no size limit.** VM reads entire file into a string; no streaming/capability guard. Paths are unrestricted process-relative/absolute filesystem accesses; this is a language capability, not an escape vulnerability by itself.
20. **Map operations linear.** Map lookup/update traverses insertion-ordered Vec; complexity O(n) each. Language limits literal entries to 127, but runtime `MapSet` can add entries for forged bytecode (and record map checker only fixed keys) with no runtime cap.

## Security/stability observations

- Safe Rust is used for compiler/parser/typechecker/VM logic except native runtime FFI. `crates/runtime/src/lib.rs` has unsafe pointer operations for string headers, Win32 buffers and allocations. `str_parts` trusts non-null, suitably aligned, valid string pointers and lengths; these are compiler-generated ABI inputs but there is no defensive header validation.
- `Heap::get/get_mut` trust a u32 handle and panic on invalid index/dangling slot. Verified source does not directly produce handles, but malformed bytecode can produce a `Value::List` only via builtins, not constant tags; VM runtime invariants protect typical use. Corruption/internal bugs can panic.
- `deserialize` decodes types and tables but assumes valid entry indexing later in serialize/disassemble; `serialize` directly indexes `m.funcs[m.entry]` and can panic for a caller-constructed invalid module.
- The parser's `peek` assumes token vector non-empty; public `parse(Vec<Token>)` can be called with empty vector and then panic (`self.toks.len()-1` underflows). Normal pipeline always passes lexer-produced EOF token.
- `print` typechecker diagnoses wrong arity but lowerer accesses `args[0]`; compile returns errors before lowering under normal pipeline. No language-specific GUI APIs are usable safely.
- `scripts/make_dist.ps1` and installer/uninstaller mutate distribution/install/PATH paths; they are user-invoked and not part of build tests.

## Environment / verification blockers

- No `.git`; working tree delta cannot be isolated.
- Current build error blocks full suite, CLI tests and native tests.
- No GUI executable exists to launch. Even if GUI subsystem PE could be produced, runtime/UI requirements are absent.
- Tests used for audit were run on the pinned Windows GNU target; no cross-platform target builds were attempted because project config/toolchain pins Windows and native linker is hard-coded.
