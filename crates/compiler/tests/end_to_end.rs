//! End-to-end tests: source -> compile -> verify -> execute.
//!
//! These exercise the whole pipeline the way `lutetium run` does, plus
//! negative cases for diagnostics, verification and the bytecode container.

use lu_compiler::compile_source;
use lu_vm::{
    deserialize, run, run_with_options, serialize, verify, BytecodeModule, Value, VmOptions,
};

fn run_src(src: &str) -> Result<String, String> {
    let module = compile_source("test.lu", src).map_err(|ds| {
        ds.iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    verify(&module).map_err(|e| e.to_string())?;
    let mut out = String::new();
    match run(&module, |line| out.push_str(line)) {
        Ok(_) => Ok(out),
        Err(e) => Err(format!("runtime: {}", e)),
    }
}

fn run_gc_stress(src: &str) -> Result<String, String> {
    let module = compile_source("test.lu", src).map_err(|ds| {
        ds.iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    verify(&module).map_err(|e| e.to_string())?;
    let mut out = String::new();
    let (_, stats) = run_with_options(
        &module,
        |line| out.push_str(line),
        VmOptions { gc_stress: true },
    )
    .map_err(|e| format!("runtime: {}", e))?;
    assert!(stats.minor_collections > 0, "stress mode should collect");
    Ok(out)
}

fn err_code(src: &str) -> String {
    match compile_source("test.lu", src) {
        Ok(_) => panic!("expected compile error for: {}", src),
        Err(ds) => ds[0].code.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Positive cases
// ---------------------------------------------------------------------------

#[test]
fn hello_world() {
    assert_eq!(
        run_src("fn main() {\n    print(\"Hello from Lutetium\")\n}\n").unwrap(),
        "Hello from Lutetium\n"
    );
}

#[test]
fn gui_builtins_fail_cleanly_on_bytecode_vm() {
    let error = run_src(
        "fn on_click(window: window) {}\nfn main() {\n    let window = gui_window(\"VM check\", 320, 240)\n    gui_button(window, 1, \"Close\", 8, 8, 100, 30, on_click)\n}\n",
    )
    .expect_err("GUI calls must remain unavailable in the bytecode VM");
    assert!(
        error.contains("GUI functions are available only in native Windows GUI executables"),
        "unexpected error: {error}"
    );
}

#[test]
fn fibonacci_recursion() {
    assert_eq!(
        run_src(
            "fn fibonacci(n: int) -> int {\n    if n <= 1 {\n        return n\n    }\n    return fibonacci(n - 1) + fibonacci(n - 2)\n}\n\nfn main() {\n    print(fibonacci(10))\n}\n"
        )
        .unwrap(),
        "55\n"
    );
}

#[test]
fn arithmetic_precedence() {
    assert_eq!(run_src("fn main() {\n    print(2 + 3 * 4)\n    print((2 + 3) * 4)\n    print(10 / 3)\n    print(10 % 3)\n    print(0 - 5)\n}\n").unwrap(), "14\n20\n3\n1\n-5\n");
}

#[test]
fn float_math_and_widening() {
    assert_eq!(
        run_src("fn main() {\n    print(1.5 + 2)\n    print(2.0)\n    print(7 / 2.0)\n}\n")
            .unwrap(),
        "3.5\n2.0\n3.5\n"
    );
}

#[test]
fn string_concat_and_comparison() {
    assert_eq!(
        run_src("fn main() {\n    print(\"a\" + \"b\" + \"c\")\n    print(\"x\" == \"x\")\n    print(\"x\" != \"y\")\n}\n").unwrap(),
        "abc\ntrue\ntrue\n"
    );
}

#[test]
fn while_loop_and_mutation() {
    assert_eq!(
        run_src(
            "fn main() {\n    let mut total = 0\n    let mut i = 1\n    while i <= 10 {\n        total += i\n        i += 1\n    }\n    print(total)\n}\n"
        )
        .unwrap(),
        "55\n"
    );
}

#[test]
fn break_and_continue() {
    assert_eq!(
        run_src(
            "fn main() {\n    let mut i = 0\n    let mut hits = 0\n    while true {\n        i += 1\n        if i % 2 == 0 {\n            continue\n        }\n        if i > 7 {\n            break\n        }\n        hits += 1\n    }\n    print(i)\n    print(hits)\n}\n"
        )
        .unwrap(),
        "9\n4\n"
    );
}

#[test]
fn inferred_return_types() {
    assert_eq!(
        run_src(
            "fn inc(x: int) {\n    return x + 1\n}\n\nfn twice(x: int) {\n    return inc(inc(x))\n}\n\nfn main() {\n    print(twice(5))\n}\n"
        )
        .unwrap(),
        "7\n"
    );
}

#[test]
fn self_recursive_inference_with_annotation() {
    assert_eq!(
        run_src(
            "fn fact(n: int) -> int {\n    if n <= 1 {\n        return 1\n    }\n    return n * fact(n - 1)\n}\n\nfn main() {\n    print(fact(5))\n}\n"
        )
        .unwrap(),
        "120\n"
    );
}

#[test]
fn short_circuit_logic() {
    assert_eq!(
        run_src(
            "fn boom() -> bool {\n    return 1 / 0 == 0\n}\n\nfn main() {\n    print(false && boom())\n    print(true || boom())\n    print(true && false)\n    print(!false)\n}\n"
        )
        .unwrap(),
        "false\ntrue\nfalse\ntrue\n"
    );
}

#[test]
fn shadowing_in_blocks() {
    assert_eq!(
        run_src(
            "fn main() {\n    let x = 1\n    if true {\n        let x = 2\n        print(x)\n    }\n    print(x)\n}\n"
        )
        .unwrap(),
        "2\n1\n"
    );
}

#[test]
fn negative_numbers() {
    assert_eq!(
        run_src("fn main() {\n    let a = -4\n    print(a / 2)\n    print(-1.5 * 2.0)\n}\n")
            .unwrap(),
        "-2\n-3.0\n"
    );
}

#[test]
fn compare_mixed_int_float() {
    assert_eq!(
        run_src("fn main() {\n    print(1 < 1.5)\n    print(2 == 2.0)\n}\n").unwrap(),
        "true\ntrue\n"
    );
}

// ---------------------------------------------------------------------------
// Compile-time diagnostics
// ---------------------------------------------------------------------------

#[test]
fn type_errors() {
    assert_eq!(
        err_code("fn main() {\n    let x: int = \"s\"\n}\n"),
        "E3001"
    );
    assert_eq!(err_code("fn main() {\n    print(x)\n}\n"), "E3005");
    assert_eq!(err_code("fn main() {\n    nosuch(1)\n}\n"), "E3006");
    assert_eq!(
        err_code("fn f(a: int) -> int {\n    return a\n}\n\nfn main() {\n    print(f())\n}\n"),
        "E3007"
    );
    assert_eq!(
        err_code("fn main() {\n    let x = 1\n    x = 2\n}\n"),
        "E3008"
    );
    assert_eq!(
        err_code("fn main() {\n    if 1 {\n        print(1)\n    }\n}\n"),
        "E3009"
    );
    assert_eq!(err_code("fn main() {\n    print(true + 1)\n}\n"), "E3010");
    assert_eq!(err_code("fn main() {\n    break\n}\n"), "E3012");
    assert_eq!(
        err_code("fn f() -> int {\n    print(1)\n}\n\nfn main() {\n    print(f())\n}\n"),
        "E3003"
    );
    assert_eq!(
        err_code("fn f() {\n    return 1\n    return \"x\"\n}\n\nfn main() {\n    print(1)\n}\n"),
        "E3002"
    );
    assert_eq!(
        err_code("fn f(x: banana) {\n    print(x)\n}\n\nfn main() {\n    print(1)\n}\n"),
        "E3004"
    );
    assert_eq!(err_code("fn main() {\n    print()\n}\n"), "E3007");
    assert_eq!(err_code("fn f() {\n}\n"), "E2102");
    assert_eq!(err_code("fn main() {\n}\n\nfn main() {\n}\n"), "E2101");
}

#[test]
fn good_diagnostic_hints() {
    let module_err = compile_source("test.lu", "fn main() {\n    print(missing)\n}\n").unwrap_err();
    assert!(module_err[0].hint.is_some());
}

// ---------------------------------------------------------------------------
// Runtime errors
// ---------------------------------------------------------------------------

#[test]
fn division_by_zero() {
    let e = run_src("fn main() {\n    print(1 / 0)\n}\n").unwrap_err();
    assert!(e.contains("E5002"), "{}", e);
}

#[test]
fn integer_overflow_is_checked() {
    let e = run_src("fn main() {\n    let big = 9223372036854775807\n    print(big + 1)\n}\n")
        .unwrap_err();
    assert!(e.contains("E5005"), "{}", e);
}

// ---------------------------------------------------------------------------
// Concise syntax (Milestone B)
// ---------------------------------------------------------------------------

#[test]
fn top_level_hello() {
    assert_eq!(
        run_src("print(\"Hello from Lutetium\")\n").unwrap(),
        "Hello from Lutetium\n"
    );
}

#[test]
fn top_level_and_main_run_in_order() {
    assert_eq!(
        run_src("print(1)\n\nfn main() {\n    print(2)\n}\n").unwrap(),
        "1\n2\n"
    );
}

#[test]
fn implicit_declarations_and_reassignment() {
    assert_eq!(
        run_src("name = \"Lutetium\"\ncount = 10\ncount += 5\nprice = 19.99\nactive = true\nprint(name)\nprint(count)\nprint(price)\nprint(active)\n").unwrap(),
        "Lutetium\n15\n19.99\ntrue\n"
    );
}

#[test]
fn implicit_declaration_respects_inferred_type() {
    let e = run_src("count = 10\ncount = \"x\"\n").unwrap_err();
    assert!(e.contains("E3001"), "{}", e);
}

#[test]
fn let_is_still_immutable() {
    let e = run_src("x = 1\nx = 2\nprint(x)\nlet y = 3\ny = 4\n").unwrap_err();
    assert!(e.contains("E3008"), "{}", e);
}

#[test]
fn for_range_inclusive_and_exclusive() {
    assert_eq!(
        run_src("sum = 0\nfor i in 1..=5 {\n    sum += i\n}\nprint(sum)\n").unwrap(),
        "15\n"
    );
    assert_eq!(
        run_src("sum = 0\nfor i in 1..5 {\n    sum += i\n}\nprint(sum)\n").unwrap(),
        "10\n"
    );
}

#[test]
fn for_range_empty_and_bounds() {
    assert_eq!(
        run_src("hits = 0\nfor i in 5..5 {\n    hits += 1\n}\nprint(hits)\n").unwrap(),
        "0\n"
    );
    assert_eq!(
        run_src("hits = 0\nfor i in 5..=5 {\n    hits += 1\n}\nprint(hits)\n").unwrap(),
        "1\n"
    );
}

#[test]
fn for_range_nested_break_continue() {
    assert_eq!(
        run_src(
            "total = 0\nfor i in 1..=3 {\n    for j in 1..=3 {\n        if j == 2 {\n            continue\n        }\n        if j == 3 {\n            break\n        }\n        total += 1\n    }\n}\nprint(total)\n"
        )
        .unwrap(),
        "3\n"
    );
}

#[test]
fn for_range_no_overflow_at_max() {
    // The inclusive stop must not increment past the bound.
    assert_eq!(
        run_src("hits = 0\nfor i in 9223372036854775806..=9223372036854775807 {\n    hits += 1\n}\nprint(hits)\n")
            .unwrap(),
        "2\n"
    );
}

#[test]
fn for_loop_variable_scoped() {
    let e = run_src("for i in 1..3 {\n    print(i)\n}\nprint(i)\n").unwrap_err();
    assert!(e.contains("E3005"), "{}", e);
}

#[test]
fn for_over_float_is_rejected() {
    let e = run_src("for i in 1.0..3.0 {\n    print(i)\n}\n").unwrap_err();
    assert!(e.contains("E3010"), "{}", e);
}

#[test]
fn expression_bodied_functions() {
    assert_eq!(
        run_src("fn square(x) {\n    x * x\n}\n\nfn add(a, b) {\n    a + b\n}\n\nprint(square(12))\nprint(add(10, 20))\n").unwrap(),
        "144\n30\n"
    );
}

#[test]
fn inferred_params_across_call_sites() {
    assert_eq!(
        run_src("fn twice(x) {\n    x * 2\n}\n\nfn main() {\n    print(twice(21))\n    print(twice(2))\n}\n").unwrap(),
        "42\n4\n"
    );
}

#[test]
fn conflicting_call_sites_rejected() {
    let e = run_src("fn twice(x) {\n    x * 2\n}\n\nfn main() {\n    print(twice(21))\n    print(twice(\"a\"))\n}\n").unwrap_err();
    assert!(e.contains("E3001"), "{}", e);
}

#[test]
fn uninferable_param_needs_annotation() {
    let e = run_src("fn f(x) {\n    return x\n}\n\nfn main() {\n    print(1)\n}\n").unwrap_err();
    assert!(e.contains("E3002"), "{}", e);
}

#[test]
fn if_expression() {
    assert_eq!(
        run_src("score = 72\nresult = if score >= 50 {\n    \"pass\"\n} else {\n    \"fail\"\n}\nprint(result)\n").unwrap(),
        "pass\n"
    );
    assert_eq!(
        run_src("score = 7\nresult = if score >= 50 {\n    \"pass\"\n} else {\n    \"fail\"\n}\nprint(result)\n").unwrap(),
        "fail\n"
    );
}

#[test]
fn if_expression_else_if_chain_and_numbers() {
    assert_eq!(
        run_src("n = 5\nlabel = if n < 0 {\n    0 - 1\n} else if n == 0 {\n    0\n} else {\n    1\n}\nprint(label)\n").unwrap(),
        "1\n"
    );
}

#[test]
fn if_expression_without_else_rejected() {
    let e = run_src("x = 1\nr = if x > 0 {\n    1\n}\nprint(r)\n").unwrap_err();
    assert!(e.contains("E2006"), "{}", e);
}

#[test]
fn if_expression_branch_type_mismatch_rejected() {
    let e = run_src("r = if true {\n    1\n} else {\n    \"x\"\n}\nprint(r)\n").unwrap_err();
    assert!(e.contains("E3001"), "{}", e);
}

#[test]
fn return_outside_function_rejected() {
    let e = run_src("return 1\n").unwrap_err();
    assert!(e.contains("E3014"), "{}", e);
}

#[test]
fn explicit_and_concise_forms_are_equivalent() {
    let explicit = "fn main() {\n    let mut total = 0\n    for i in 1..=10 {\n        total += i\n    }\n    print(total)\n}\n";
    let concise = "total = 0\nfor i in 1..=10 {\n    total += i\n}\nprint(total)\n";
    assert_eq!(run_src(explicit).unwrap(), "55\n");
    assert_eq!(run_src(concise).unwrap(), "55\n");
}

// ---------------------------------------------------------------------------
// IR optimizer (Phase E)
// ---------------------------------------------------------------------------

fn run_src_opt(src: &str, opt: u8) -> Result<String, String> {
    let module = lu_compiler::compile_ir_with_opts("test.lu", src, opt)
        .map_err(|ds| {
            ds.iter()
                .map(|d| d.code.to_string())
                .collect::<Vec<_>>()
                .join(";")
        })
        .map(|ir| lu_compiler::codegen::codegen(&ir))?;
    lu_vm::verify(&module).map_err(|e| e.to_string())?;
    let mut out = String::new();
    lu_vm::run(&module, |l| out.push_str(l)).map_err(|e| e.to_string())?;
    Ok(out)
}

#[test]
fn optimizer_preserves_behavior_at_all_levels() {
    let programs = [
        "print(2 + 3 * 4)\nprint(10 % 3)\nprint(1.5 + 2)\n",
        "sum = 0\nfor i in 1..=10 {\n    sum += i\n}\nprint(sum)\n",
        "fn fib(n: int) -> int {\n    if n <= 1 {\n        return n\n    }\n    return fib(n - 1) + fib(n - 2)\n}\nfn main() {\n    print(fib(12))\n}\n",
        "total = 0\ni = 0\nwhile true {\n    i += 1\n    if i % 2 == 0 {\n        continue\n    }\n    if i > 7 {\n        break\n    }\n    total += i\n}\nprint(i)\nprint(total)\n",
        "s = \"a\" + \"b\"\nprint(s == \"ab\")\nprint(s != \"ab\")\nprint(1 < 1.5)\n",
    ];
    for src in programs {
        let base = run_src_opt(src, 0).unwrap();
        for opt in [1u8, 2, 3] {
            let optimized = run_src_opt(src, opt).unwrap();
            assert_eq!(base, optimized, "opt {} changed behavior for {}", opt, src);
        }
    }
}

#[test]
fn optimizer_folds_constants() {
    let src = "print(2 + 3 * 4)\n";
    let ir = lu_compiler::compile_ir_with_opts("t.lu", src, 2).unwrap();
    let text = lu_compiler::ir::disassemble_ir(&ir);
    assert!(
        !text.contains("addint"),
        "constant add must be folded:\n{}",
        text
    );
    assert!(
        text.contains("const 14"),
        "folded result expected:\n{}",
        text
    );
}

#[test]
fn optimizer_keeps_runtime_panics() {
    // Division by zero must NOT be folded away; it stays a runtime error.
    let e = run_src_opt("print(1 / 0)\n", 2).unwrap_err();
    assert!(e.contains("E5002"), "{}", e);
}

#[test]
fn optimizer_removes_unreachable_blocks() {
    let src =
        "i = 0\nwhile true {\n    i += 1\n    if i > 3 {\n        break\n    }\n}\nprint(i)\n";
    let unopt = lu_compiler::compile_ir_with_opts("t.lu", src, 0).unwrap();
    let opt = lu_compiler::compile_ir_with_opts("t.lu", src, 2).unwrap();
    let blocks_unopt: usize = unopt.funcs.iter().map(|f| f.blocks.len()).sum();
    let blocks_opt: usize = opt.funcs.iter().map(|f| f.blocks.len()).sum();
    assert!(
        blocks_opt < blocks_unopt,
        "unreachable blocks must be removed"
    );
}

// ---------------------------------------------------------------------------
// Bytecode container & verifier
// ---------------------------------------------------------------------------

#[test]
fn serialize_deserialize_roundtrip() {
    let module = compile_source(
        "test.lu",
        "fn add(a: int, b: int) -> int {\n    return a + b\n}\n\nfn main() {\n    print(add(2, 3))\n}\n",
    )
    .unwrap();
    let bytes = serialize(&module);
    let restored = deserialize(&bytes).unwrap();
    let mut out = String::new();
    run(&restored, |l| out.push_str(l)).unwrap();
    assert_eq!(out, "5\n");
}

#[test]
fn deserialize_rejects_garbage() {
    assert!(deserialize(b"nope").is_err());
    let module = compile_source("test.lu", "fn main() {\n    print(1)\n}\n").unwrap();
    let bytes = serialize(&module);
    assert!(deserialize(&bytes[..bytes.len() / 2]).is_err());
}

#[test]
fn verifier_rejects_tampered_code() {
    let module = compile_source("test.lu", "fn main() {\n    print(42)\n}\n").unwrap();
    let bytes = serialize(&module);
    let mut restored = deserialize(&bytes).unwrap();
    // Replace the first opcode of the entry function with an unknown one.
    restored.funcs[restored.entry].code[0] = 0xFF;
    assert!(verify(&restored).is_err());
}

#[test]
fn verifier_rejects_unverified_register_read() {
    // Hand-built module: reads register r1 which is never written.
    use lu_vm::bytecode::{op, BcFunction, BcTy};
    let m = BytecodeModule {
        consts: vec![],
        funcs: vec![BcFunction {
            name: "main".into(),
            arity: 0,
            nregs: 2,
            ret: BcTy::Unit,
            code: vec![op::PRINT, 1, 0, op::RET_UNIT],
        }],
        entry: 0,
    };
    assert!(verify(&m).is_err());
}

#[test]
fn verifier_accepts_valid_handbuilt_module() {
    use lu_vm::bytecode::{op, BcConst, BcFunction, BcTy};
    let m = BytecodeModule {
        consts: vec![BcConst::Int(7)],
        funcs: vec![BcFunction {
            name: "main".into(),
            arity: 0,
            nregs: 1,
            ret: BcTy::Unit,
            code: vec![
                op::CONST_INT,
                0,
                0,
                0,
                0,
                0,
                0,
                op::PRINT,
                0,
                0,
                op::RET_UNIT,
            ],
        }],
        entry: 0,
    };
    verify(&m).unwrap();
    let mut out = String::new();
    run(&m, |l| out.push_str(l)).unwrap();
    assert_eq!(out, "7\n");
}

#[test]
fn entry_value_returned() {
    let module = compile_source("test.lu", "fn main() {\n    print(1)\n}\n").unwrap();
    let mut out = String::new();
    let v = run(&module, |l| out.push_str(l)).unwrap();
    assert_eq!(v, Value::Unit);
}

// ---------------------------------------------------------------------------
// Lists, string methods and built-in functions
// ---------------------------------------------------------------------------

#[test]
fn list_literals_and_indexing() {
    let out = run_src(
        "numbers = [1, 2, 3]\nprint(numbers[0])\nprint(numbers[2])\nprint(numbers.len())\n",
    )
    .unwrap();
    assert_eq!(out, "1\n3\n3\n");
}

#[test]
fn list_mutation_methods() {
    let out = run_src(
        "lst = [1, 2]\nlst.push(3)\nprint(lst.len())\nlst.set(0, 10)\nprint(lst.get(0))\nlast = lst.pop()\nprint(last)\nprint(lst.len())\n",
    )
    .unwrap();
    assert_eq!(out, "3\n10\n3\n2\n");
}

#[test]
fn list_print() {
    assert_eq!(run_src("print([1, 2, 3])\n").unwrap(), "[1, 2, 3]\n");
}

#[test]
fn list_widening_mixed_int_float() {
    // The checker widens [1, 2.5] to a float list; the int element is cast.
    assert_eq!(
        run_src("lst = [1, 2.5]\nprint(lst.get(0))\nprint(lst.get(1))\n").unwrap(),
        "1.0\n2.5\n"
    );
}

#[test]
fn lists_flow_through_functions() {
    let out = run_src(
        "fn sum(lst) -> int {\n    total = 0\n    for i in 0..lst.len() {\n        total += lst.get(i)\n    }\n    return total\n}\n\nfn evens(n: int) {\n    out = [0]\n    for i in 0..n {\n        if i % 2 == 0 {\n            out.push(i)\n        }\n    }\n    return out\n}\n\nprint(sum([1, 2, 3, 4, 5]))\nprint(evens(6).len())\nprint(evens(6).get(2))\n",
    )
    .unwrap();
    // evens(6) is [0, 0, 2, 4]: the seeded 0 plus one even per iteration.
    assert_eq!(out, "15\n4\n2\n");
}

#[test]
fn string_methods() {
    let out = run_src(
        "print(\"Hello\".upper())\nprint(\"Hello\".lower())\nprint(\"  hi  \".trim())\nprint(\"hello\".contains(\"ell\"))\nprint(\"hello\".replace(\"l\", \"L\"))\nprint(\"hello\".len())\nparts = \"a-b-c\".split(\"-\")\nprint(parts.len())\nprint(parts.get(1))\n",
    )
    .unwrap();
    assert_eq!(out, "HELLO\nhello\nhi\ntrue\nheLLo\n5\n3\nb\n");
}

#[test]
fn math_builtins() {
    let out = run_src(
        "print(sqrt(144.0))\nprint(pow(2.0, 10.0))\nprint(abs(-5))\nprint(min(3, 7))\nprint(max(3, 7))\nprint(floor(2.7))\nprint(ceil(2.1))\nprint(round(2.5))\nprint(sin(0.0))\nprint(cos(0.0))\nprint(log(1.0))\n",
    )
    .unwrap();
    // sqrt/pow/floor/ceil/round/sin/cos/log produce floats; abs/min/max on
    // ints stay ints (overload selection).
    assert_eq!(out, "12.0\n1024.0\n5\n3\n7\n2.0\n3.0\n3.0\n0.0\n1.0\n0.0\n");
}

#[test]
fn fs_builtins_roundtrip() {
    let path = std::env::temp_dir().join("lutetium_e2e_fs.txt");
    let path_lit = path.to_str().unwrap().replace('\\', "/");
    let src = format!(
        "write_text(\"{p}\", \"line1\\nline2\\n\")\nprint(file_exists(\"{p}\"))\ncontent = read_text(\"{p}\")\nprint(content.len())\nlines = content.split(\"\\n\")\nprint(lines.len())\nprint(lines.get(1))\n",
        p = path_lit
    );
    let out = run_src(&src).unwrap();
    assert_eq!(out, "true\n12\n3\nline2\n");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn lists_survive_gc_collections() {
    // Many list allocations trigger minor/major collections; rooted and
    // reachable lists must stay correct.
    let out = run_src(
        "lst = [100]\nfor i in 0..2000 {\n    lst.push(i)\n    tmp = [i, i, i]\n    if tmp.len() != 3 {\n        print(-1)\n    }\n}\nprint(lst.len())\nprint(lst.get(1999))\n",
    )
    .unwrap();
    // lst is [100, 0, 1, ..., 1999]: 2001 elements, element 1999 is 1998.
    assert_eq!(out, "2001\n1998\n");
}

#[test]
fn err_mixed_list_element_types() {
    assert_eq!(err_code("x = [1, \"a\"]\n"), "E3001");
}

#[test]
fn err_push_wrong_element_type() {
    assert_eq!(err_code("lst = [1]\nlst.push(\"x\")\n"), "E3001");
}

#[test]
fn err_float_list_index() {
    assert_eq!(err_code("lst = [1]\nprint(lst[0.5])\n"), "E3010");
}

#[test]
fn err_unknown_method() {
    assert_eq!(err_code("s = \"hi\"\ns.push(1)\n"), "E3015");
}

#[test]
fn err_builtin_arity() {
    assert_eq!(err_code("print(sqrt(1, 2))\n"), "E3007");
}

#[test]
fn err_unknown_builtin_function() {
    assert_eq!(err_code("print(squrt(2))\n"), "E3006");
}

#[test]
fn err_empty_list_element_inference() {
    assert_eq!(err_code("x = []\n"), "E3002");
}

#[test]
fn err_nested_list() {
    assert_eq!(err_code("x = [[1]]\n"), "E3001");
}

#[test]
fn runtime_pop_from_empty_list() {
    let e = run_src("lst = [1]\nlst.pop()\nlst.pop()\n").unwrap_err();
    assert!(e.contains("E5009"), "{}", e);
}

#[test]
fn optimizer_preserves_lists_and_builtins() {
    let programs = [
        "lst = [1, 2, 3]\nlst.push(4)\nprint(lst.len())\nprint(lst.get(1))\nlst.set(2, 9)\nprint(lst.get(2))\n",
        "s = \"a-b\".split(\"-\")\nprint(s.len())\nprint(s.get(0))\nprint(sqrt(4.0))\nprint(abs(-3))\n",
    ];
    for src in programs {
        let base = run_src_opt(src, 0).unwrap();
        for opt in [1u8, 2, 3] {
            let optimized = run_src_opt(src, opt).unwrap();
            assert_eq!(base, optimized, "opt {} changed behavior for {}", opt, src);
        }
    }
}

// ---------------------------------------------------------------------------
// Maps (record literals, field access, len/get/set)
// ---------------------------------------------------------------------------

#[test]
fn map_literals_and_field_access() {
    let out =
        run_src("u = {name: \"Ada\", score: 95}\nprint(u.name)\nprint(u.score)\nprint(u.len())\n")
            .unwrap();
    assert_eq!(out, "Ada\n95\n2\n");
}

#[test]
fn map_heterogeneous_values() {
    // Record maps keep a per-key value type; mixed types are fine.
    let out = run_src(
        "u = {name: \"Ada\", score: 95, ratio: 0.5, ok: true}\nprint(u.ok)\nprint(u.ratio)\nprint(u.score)\n",
    )
    .unwrap();
    assert_eq!(out, "true\n0.5\n95\n");
}

#[test]
fn map_get_set_methods() {
    let out = run_src(
        "u = {n: 1}\nprint(u.get(\"n\"))\nu.set(\"n\", 42)\nprint(u.get(\"n\"))\nprint(u.len())\n",
    )
    .unwrap();
    assert_eq!(out, "1\n42\n1\n");
}

#[test]
fn map_set_widens_int_to_float_field() {
    let out = run_src("u = {r: 0.5}\nu.set(\"r\", 2)\nprint(u.get(\"r\"))\n").unwrap();
    assert_eq!(out, "2.0\n");
}

#[test]
fn map_print() {
    assert_eq!(
        run_src("print({name: \"Ada\"})\n").unwrap(),
        "{name: Ada}\n"
    );
}

#[test]
fn empty_map_is_a_valid_record() {
    let out = run_src("e = {}\nprint(e.len())\n").unwrap();
    assert_eq!(out, "0\n");
}

#[test]
fn map_duplicate_keys_last_wins() {
    let out = run_src("u = {n: 1, n: 2}\nprint(u.n)\nprint(u.len())\n").unwrap();
    assert_eq!(out, "2\n1\n");
}

#[test]
fn map_field_after_set_reflects_mutation() {
    let out = run_src(
        "u = {name: \"Ada\", score: 95}\nu.set(\"score\", 100)\nprint(u.score)\nprint(u.get(\"score\"))\n",
    )
    .unwrap();
    assert_eq!(out, "100\n100\n");
}

#[test]
fn map_field_is_reference_semantics() {
    // Two variables may point at the same map; mutation is visible through
    // both, exactly like lists.
    let out = run_src("a = {n: 1}\nb = a\nb.set(\"n\", 7)\nprint(a.n)\n").unwrap();
    assert_eq!(out, "7\n");
}

#[test]
fn maps_flow_through_functions() {
    let out = run_src(
        "fn bump(u) -> int {\n    u.set(\"score\", u.score + 1)\n    return u.score\n}\n\nu = {score: 41}\nprint(bump(u))\nprint(u.score)\n",
    )
    .unwrap();
    assert_eq!(out, "42\n42\n");
}

#[test]
fn maps_survive_gc_collections() {
    // Many map allocations trigger minor/major collections; rooted and
    // reachable maps (and their field values) must stay correct.
    let out = run_src(
        "u = {n: 0, tag: \"keep\"}\nfor i in 0..2000 {\n    u.set(\"n\", i)\n    tmp = {n: i, tag: \"v\"}\n    if tmp.len() != 2 {\n        print(-1)\n    }\n}\nprint(u.n)\nprint(u.tag)\n",
    )
    .unwrap();
    assert_eq!(out, "1999\nkeep\n");
}

#[test]
fn optimizer_preserves_maps() {
    let programs = [
        "u = {name: \"Ada\", score: 95}\nprint(u.name)\nprint(u.score)\nu.set(\"score\", 100)\nprint(u.get(\"score\"))\n",
        "e = {}\nprint(e.len())\n",
    ];
    for src in programs {
        let base = run_src_opt(src, 0).unwrap();
        for opt in [1u8, 2, 3] {
            let optimized = run_src_opt(src, opt).unwrap();
            assert_eq!(base, optimized, "opt {} changed behavior for {}", opt, src);
        }
    }
}

#[test]
fn err_unknown_map_field() {
    assert_eq!(err_code("u = {name: \"Ada\"}\nprint(u.score)\n"), "E3010");
}

#[test]
fn err_field_on_non_map() {
    assert_eq!(err_code("x = 5\nprint(x.name)\n"), "E3010");
}

#[test]
fn err_field_on_list() {
    assert_eq!(err_code("lst = [1]\nprint(lst.head)\n"), "E3010");
}

#[test]
fn err_map_set_wrong_value_type() {
    assert_eq!(
        err_code("u = {score: 95}\nu.set(\"score\", \"high\")\n"),
        "E3001"
    );
}

#[test]
fn err_map_set_adds_field() {
    // Record maps have a fixed key set: new keys cannot be added.
    assert_eq!(err_code("u = {n: 1}\nu.set(\"m\", 2)\n"), "E3010");
}

#[test]
fn err_map_get_unknown_key() {
    assert_eq!(err_code("u = {n: 1}\nprint(u.get(\"m\"))\n"), "E3010");
}

#[test]
fn err_map_get_dynamic_key() {
    assert_eq!(
        err_code("u = {n: 1}\nk = \"n\"\nprint(u.get(k))\n"),
        "E3010"
    );
}

#[test]
fn err_map_get_non_string_key() {
    assert_eq!(err_code("u = {n: 1}\nprint(u.get(1))\n"), "E3010");
}

#[test]
fn err_map_set_non_string_key() {
    assert_eq!(err_code("u = {n: 1}\nu.set(2, 3)\n"), "E3010");
}

#[test]
fn err_map_in_list() {
    assert_eq!(err_code("x = [{n: 1}]\n"), "E3001");
}

#[test]
fn err_list_in_map() {
    assert_eq!(err_code("x = {v: [1]}\n"), "E3001");
}

#[test]
fn err_map_in_map() {
    assert_eq!(err_code("x = {a: {b: 1}}\n"), "E3001");
}

#[test]
fn err_map_equality() {
    assert_eq!(err_code("a = {n: 1}\nb = {n: 1}\nprint(a == b)\n"), "E3010");
}

#[test]
fn err_map_literal_too_large() {
    let mut src = String::from("u = {");
    for i in 0..128 {
        if i > 0 {
            src.push_str(", ");
        }
        src.push_str(&format!("k{}: {}", i, i));
    }
    src.push_str("}\n");
    assert_eq!(err_code(&src), "E3016");
}

#[test]
fn err_map_method_unknown() {
    assert_eq!(err_code("u = {n: 1}\nu.push(2)\n"), "E3015");
}

// ---------------------------------------------------------------------------
// `for x in lst` iteration and list equality (bytecode v4: EQ_LIST/NE_LIST)
// ---------------------------------------------------------------------------

#[test]
fn for_in_iterates_in_order() {
    let out = run_src("for x in [10, 20, 30] {\n    print(x)\n}\n").unwrap();
    assert_eq!(out, "10\n20\n30\n");
}

#[test]
fn for_in_empty_list_zero_iterations() {
    let out = run_src(
        "count = 0\nfor x in [0] {\n    count = count + 1\n}\nempty = [0]\nempty.pop()\nfor x in empty {\n    count = count + 1\n}\nprint(count)\n",
    )
    .unwrap();
    assert_eq!(out, "1\n");
}

#[test]
fn for_in_over_float_list() {
    let out = run_src("for x in [0.5, 1.5, 2.5] {\n    print(x)\n}\n").unwrap();
    assert_eq!(out, "0.5\n1.5\n2.5\n");
}

#[test]
fn for_in_loop_var_is_a_copy() {
    // Assigning to the loop variable affects only the body's local copy;
    // iteration itself is unchanged.
    let out = run_src("for x in [1, 2] {\n    x = x + 10\n    print(x)\n}\n").unwrap();
    assert_eq!(out, "11\n12\n");
}

#[test]
fn for_in_iterable_evaluated_once() {
    // The iterable expression runs exactly once, before the loop.
    let out = run_src(
        "calls = [0]\nfn mk(calls) {\n    calls.set(0, calls.get(0) + 1)\n    [1, 2]\n}\nfor x in mk(calls) {\n    print(x)\n}\nprint(calls[0])\n",
    )
    .unwrap();
    assert_eq!(out, "1\n2\n1\n");
}

#[test]
fn for_in_push_extends_and_pop_shortens() {
    // Documented semantics: the length is re-checked every iteration, so
    // pushes made in the body extend the walk and pops shorten it.
    let out = run_src(
        "grow = [1]\nfor x in grow {\n    if x < 5 {\n        grow.push(x + 1)\n    }\n}\nprint(grow.len())\nshrink = [1, 2, 3, 4, 5]\nfor x in shrink {\n    shrink.pop()\n}\nprint(shrink.len())\n",
    )
    .unwrap();
    assert_eq!(out, "5\n2\n");
}

#[test]
fn for_in_break_and_continue() {
    let out = run_src(
        "found = -1\nfor x in [3, 7, 9, 12] {\n    if x == 9 {\n        found = x\n        break\n    }\n}\nprint(found)\nskip = 0\nfor x in [1, 2, 3, 4, 5] {\n    if x == 3 {\n        continue\n    }\n    skip = skip + x\n}\nprint(skip)\n",
    )
    .unwrap();
    assert_eq!(out, "9\n12\n");
}

#[test]
fn for_in_nested() {
    let out = run_src(
        "nested = 0\nfor a in [1, 2] {\n    for b in [10, 20] {\n        nested = nested + a * b\n    }\n}\nprint(nested)\n",
    )
    .unwrap();
    assert_eq!(out, "90\n");
}

#[test]
fn for_in_survives_gc_stress() {
    // Allocations in the body with GC stress (a collection at every
    // allocation); the captured list and index must stay valid.
    let out = run_gc_stress(
        "total = 0\nfor x in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {\n    total = total + x\n    junk = [0, 0, 0, 0]\n    junk.push(x)\n    if junk.len() != 5 {\n        print(-1)\n    }\n}\nprint(total)\n",
    )
    .unwrap();
    assert_eq!(out, "55\n");
}

#[test]
fn for_in_inside_function() {
    let out = run_src(
        "fn sum(lst) -> int {\n    total = 0\n    for x in lst {\n        total = total + x\n    }\n    return total\n}\nprint(sum([1, 2, 3]))\nprint(sum([0]))\n",
    )
    .unwrap();
    assert_eq!(out, "6\n0\n");
}

#[test]
fn list_equality_basics() {
    let out = run_src(
        "lst = [1, 2]\nprint(lst == [1, 2])\nprint(lst != [1, 3])\nprint([1, 2] == [2, 1])\nprint(lst == lst)\nprint([1, 2] == [1, 2, 3])\n",
    )
    .unwrap();
    assert_eq!(out, "true\ntrue\nfalse\ntrue\nfalse\n");
}

#[test]
fn list_equality_int_float_widening() {
    let out = run_src("print([1] == [1.0])\nprint([1, 2.5] == [1.0, 2.5])\nprint([1] == [1.5])\n")
        .unwrap();
    assert_eq!(out, "true\ntrue\nfalse\n");
}

#[test]
fn list_equality_bool_str_elements() {
    let out = run_src(
        "print([true, false] == [true, false])\nprint([\"a\", \"b\"] == [\"a\", \"b\"])\nprint([\"a\"] == [\"b\"])\nprint([true] == [false])\n",
    )
    .unwrap();
    assert_eq!(out, "true\ntrue\nfalse\nfalse\n");
}

#[test]
fn list_equality_sees_mutations_through_alias() {
    // Reference semantics: mutating one alias changes what `==` sees.
    let out = run_src(
        "a = [1, 2]\nb = a\nprint(a == b)\nb.set(0, 9)\nprint(a == b)\nprint(a == [9, 2])\n",
    )
    .unwrap();
    assert_eq!(out, "true\ntrue\ntrue\n");
}

#[test]
fn optimizer_preserves_for_in_and_equality() {
    let programs = [
        "total = 0\nfor x in [1, 2, 3] {\n    total = total + x\n}\nprint(total)\n",
        "grow = [1]\nfor x in grow {\n    if x < 3 {\n        grow.push(x + 1)\n    }\n}\nprint(grow.len())\n",
        "lst = [1, 2]\nprint(lst == [1, 2])\nprint(lst != [2, 1])\n",
    ];
    for src in programs {
        let base = run_src_opt(src, 0).unwrap();
        for opt in [1u8, 2, 3] {
            let optimized = run_src_opt(src, opt).unwrap();
            assert_eq!(base, optimized, "opt {} changed behavior for {}", opt, src);
        }
    }
}

#[test]
fn err_for_in_over_int() {
    assert_eq!(err_code("n = 5\nfor x in n {\n    print(x)\n}\n"), "E3010");
}

#[test]
fn err_for_in_over_float() {
    assert_eq!(err_code("for x in 1.5 {\n    print(x)\n}\n"), "E3010");
}

#[test]
fn err_for_in_over_str() {
    assert_eq!(err_code("for x in \"abc\" {\n    print(x)\n}\n"), "E3010");
}

#[test]
fn err_for_in_over_map() {
    assert_eq!(
        err_code("m = {a: 1}\nfor x in m {\n    print(x)\n}\n"),
        "E3010"
    );
}

#[test]
fn err_list_eq_mixed_element_types() {
    assert_eq!(err_code("a = [1]\nb = [\"x\"]\nprint(a == b)\n"), "E3010");
}

#[test]
fn err_list_ordering() {
    assert_eq!(err_code("a = [1]\nb = [2]\nprint(a < b)\n"), "E3010");
}
