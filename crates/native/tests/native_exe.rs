//! Native backend integration tests: compile real `.lu` programs to Windows
//! executables, run them as independent processes, and verify output/exit
//! codes. These test actual generated executables, not bytecode.

use lu_compiler::compile_ir;
use lu_native::build_exe;
use std::process::Command;

fn tmp_dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join("lutetium_native_tests");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Compile `src` to a native exe and run it, returning (exit_code, stdout, stderr).
fn run_native(src: &str, name: &str) -> (i32, String, String) {
    let dir = tmp_dir();
    let ir = compile_ir("test.lu", src).expect("source must compile");
    let exe = dir.join(format!("{}.exe", name));
    build_exe(&ir, &exe).expect("native build must succeed");
    assert!(exe.exists(), "generated executable missing");
    let out = Command::new(&exe)
        .output()
        .expect("generated exe must launch");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn native_hello_world() {
    let (code, out, err) = run_native("print(\"Hello from Lutetium\")\n", "hello");
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "Hello from Lutetium\n");
}

#[test]
fn native_arithmetic() {
    let (code, out, err) = run_native(
        "print(2 + 3 * 4)\nprint((2 + 3) * 4)\nprint(10 / 3)\nprint(10 % 3)\nprint(0 - 5)\nprint(1.5 + 2)\n",
        "arith",
    );
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "14\n20\n3\n1\n-5\n3.5\n");
}

#[test]
fn native_variables_and_strings() {
    let (code, out, err) = run_native(
        "name = \"Lutetium\"\ncount = 10\ncount += 5\nprice = 19.99\nactive = true\nprint(name)\nprint(count)\nprint(price)\nprint(active)\nprint(\"a\" + \"b\" + \"c\")\nprint(\"x\" == \"x\")\n",
        "vars",
    );
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "Lutetium\n15\n19.99\ntrue\nabc\ntrue\n");
}

#[test]
fn native_conditionals_and_loops() {
    let (code, out, err) = run_native(
        "age = 20\ncategory = if age >= 18 {\n    \"Adult\"\n} else {\n    \"Minor\"\n}\nprint(category)\nsum = 0\nfor i in 1..=100 {\n    sum += i\n}\nprint(sum)\nfor i in 1..5 {\n    if i == 2 {\n        continue\n    }\n    if i == 4 {\n        break\n    }\n    print(i)\n}\n",
        "condloop",
    );
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "Adult\n5050\n1\n3\n");
}

#[test]
fn native_functions_and_recursion() {
    let (code, out, err) = run_native(
        "fn square(x) {\n    x * x\n}\n\nfn fibonacci(n: int) -> int {\n    if n <= 1 {\n        return n\n    }\n    return fibonacci(n - 1) + fibonacci(n - 2)\n}\n\nprint(square(12))\nprint(fibonacci(10))\n",
        "funcrec",
    );
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "144\n55\n");
}

#[test]
fn native_division_by_zero_exits_1() {
    let (code, out, err) = run_native("print(1 / 0)\n", "divzero");
    assert_eq!(code, 1, "stdout: {} stderr: {}", out, err);
    assert!(err.contains("division by zero"), "stderr: {}", err);
}

#[test]
fn native_integer_overflow_exits_1() {
    let (code, out, err) = run_native("big = 9223372036854775807\nprint(big + 1)\n", "overflow");
    assert_eq!(code, 1, "stdout: {} stderr: {}", out, err);
    assert!(err.contains("overflow"), "stderr: {}", err);
}

#[test]
fn native_short_circuit_logic() {
    let (code, out, err) = run_native(
        "print(true && false)\nprint(true || false)\nprint(!false)\nprint(false && (1 / 0 == 0))\n",
        "logic",
    );
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "false\ntrue\ntrue\nfalse\n");
}

#[test]
fn native_while_loop() {
    let (code, out, err) = run_native(
        "total = 0\ni = 1\nwhile i <= 10 {\n    total += i\n    i += 1\n}\nprint(total)\n",
        "whileloop",
    );
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "55\n");
}

#[test]
fn native_matches_vm_semantics() {
    // A varied program executed through both backends must produce identical
    // observable behavior.
    let src = "name = \"Lutetium\"\nsum = 0\nfor i in 1..=10 {\n    sum += i\n}\nfn fib(n: int) -> int {\n    if n <= 1 {\n        return n\n    }\n    return fib(n - 1) + fib(n - 2)\n}\nprint(name)\nprint(sum)\nprint(fib(12))\nprint(if sum > 50 { 1.5 } else { 2.5 })\n";
    let expected = "Lutetium\n55\n144\n1.5\n";
    // bytecode VM
    let module = lu_compiler::compile_source("t.lu", src).unwrap();
    lu_vm::verify(&module).unwrap();
    let mut vm_out = String::new();
    lu_vm::run(&module, |l| vm_out.push_str(l)).unwrap();
    assert_eq!(vm_out, expected);
    // native
    let (code, out, err) = run_native(src, "equivalence");
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, expected);
}

#[test]
fn native_negative_numbers() {
    let (code, out, err) = run_native("a = -4\nprint(a / 2)\nprint(-1.5 * 2.0)\n", "neg");
    assert_eq!(code, 0, "stderr: {}", err);
    assert_eq!(out, "-2\n-3.0\n");
}
