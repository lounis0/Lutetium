//! Integration tests for the CLI command implementations.

use lutetium_cli::real_main;
use std::sync::Mutex;

/// Both `build` tests write into the shared relative `build/` directory;
/// serialize them so parallel execution cannot delete a directory the other
/// test is using.
static BUILD_LOCK: Mutex<()> = Mutex::new(());

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn repo(rel: &str) -> String {
    format!("{}\\..\\..\\{}", env!("CARGO_MANIFEST_DIR"), rel)
}

#[test]
fn version_prints() {
    assert_eq!(real_main(args(&["--version"])), 0);
}

#[test]
fn check_ok_and_json() {
    let f = repo("examples/fibonacci.lu");
    assert_eq!(real_main(vec!["check".into(), f.clone()]), 0);
    assert_eq!(
        real_main(vec![
            "check".into(),
            f,
            "--diagnostics".into(),
            "json".into()
        ]),
        0
    );
}

#[test]
fn check_reports_errors_with_exit_code_1() {
    let bad = std::env::temp_dir().join("lutetium_bad.lu");
    std::fs::write(&bad, "fn main() {\n    let x: int = true\n}\n").unwrap();
    let path = bad.to_str().unwrap().to_string();
    assert_eq!(real_main(vec!["check".into(), path]), 1);
    std::fs::remove_file(&bad).ok();
}

#[test]
fn run_examples() {
    assert_eq!(real_main(vec!["run".into(), repo("examples/hello.lu")]), 0);
    assert_eq!(
        real_main(vec!["run".into(), repo("examples/fibonacci.lu")]),
        0
    );
    assert_eq!(
        real_main(vec!["run".into(), repo("examples/strings.lu")]),
        0
    );
    // concise-syntax examples
    assert_eq!(
        real_main(vec!["run".into(), repo("examples/concise/hello.lu")]),
        0
    );
    assert_eq!(
        real_main(vec!["run".into(), repo("examples/concise/loop.lu")]),
        0
    );
    assert_eq!(
        real_main(vec!["run".into(), repo("examples/concise/function.lu")]),
        0
    );
    assert_eq!(
        real_main(vec!["run".into(), repo("examples/concise/conditional.lu")]),
        0
    );
    assert_eq!(
        real_main(vec![
            "run".into(),
            repo("examples/concise/explicit_typing.lu")
        ]),
        0
    );
}

#[test]
fn run_runtime_error_exits_1() {
    let bad = std::env::temp_dir().join("lutetium_div0.lu");
    std::fs::write(&bad, "fn main() {\n    print(1 / 0)\n}\n").unwrap();
    let path = bad.to_str().unwrap().to_string();
    assert_eq!(real_main(vec!["run".into(), path]), 1);
    std::fs::remove_file(&bad).ok();
}

#[test]
fn run_gc_stats_reports_real_numbers() {
    let _guard = BUILD_LOCK.lock().unwrap();
    let src = std::env::temp_dir().join("lutetium_gc.lu");
    std::fs::write(&src, "lst = [1, 2, 3]\nlst.push(4)\nprint(lst.len())\n").unwrap();
    let path = src.to_str().unwrap().to_string();
    assert_eq!(
        real_main(vec!["run".into(), path.clone(), "--gc-stats".into()]),
        0
    );
    assert_eq!(
        real_main(vec![
            "run".into(),
            path,
            "--gc-stress".into(),
            "--gc-stats".into()
        ]),
        0
    );
    std::fs::remove_file(&src).ok();
}

#[test]
fn build_writes_lbc_then_inspect_reads_it() {
    let _guard = BUILD_LOCK.lock().unwrap();
    let src = repo("examples/fibonacci.lu");
    assert_eq!(real_main(vec!["build".into(), src]), 0);
    // `build` writes `build/<name>.lbc` relative to the current directory.
    assert_eq!(real_main(args(&["inspect", "build/fibonacci.lbc"])), 0);
    std::fs::remove_file("build/fibonacci.lbc").ok();
    std::fs::remove_dir("build").ok();
}

#[test]
fn build_native_produces_runnable_exe() {
    let _guard = BUILD_LOCK.lock().unwrap();
    let src = repo("examples/fibonacci.lu");
    assert_eq!(
        real_main(vec![
            "build".into(),
            src,
            "--target".into(),
            "native".into()
        ]),
        0
    );
    // `build` writes `build/<name>.exe` relative to the current directory.
    let exe = std::path::Path::new("build/fibonacci.exe");
    assert!(exe.exists(), "native executable must exist");
    let out = std::process::Command::new(exe.canonicalize().unwrap())
        .output()
        .expect("generated native exe must launch");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "55\n");
    std::fs::remove_file(exe).ok();
    std::fs::remove_file("build/fibonacci.obj").ok();
    std::fs::remove_file("build/fibonacci.runtime.a").ok();
    // Legacy name from before the per-exe runtime archive rename.
    std::fs::remove_file("build/lutetium_runtime.a").ok();
}

#[test]
fn build_gui_produces_launchable_windows_gui_executable() {
    let _guard = BUILD_LOCK.lock().unwrap();
    assert_eq!(
        real_main(vec![
            "build".into(),
            repo("examples/ion_button.lu"),
            "--target".into(),
            "gui".into()
        ]),
        0
    );
    let exe = std::path::Path::new("build/ion_button.exe");
    assert!(exe.exists(), "GUI executable must exist");
    let _ = std::fs::remove_file(exe);
    let _ = std::fs::remove_file("build/ion_button.obj");
    let _ = std::fs::remove_file("build/ion_button.runtime.a");
}

#[test]
fn build_bytecode_respects_opt_flag_and_inspect_ir() {
    let src = repo("examples/fibonacci.lu");
    assert_eq!(
        real_main(vec!["build".into(), src, "--opt".into(), "1".into()]),
        0
    );
    assert!(std::path::Path::new("build/fibonacci.lbc").exists());
    std::fs::remove_file("build/fibonacci.lbc").ok();
    // typed IR inspection of a source file
    assert_eq!(
        real_main(vec![
            "inspect".into(),
            repo("examples/hello.lu"),
            "--ir".into()
        ]),
        0
    );
    assert_eq!(
        real_main(vec![
            "inspect".into(),
            repo("examples/hello.lu"),
            "--ir".into(),
            "--optimized".into()
        ]),
        0
    );
}

#[test]
fn explain_known_and_unknown_codes() {
    assert_eq!(real_main(args(&["explain", "E3001"])), 0);
    assert_eq!(real_main(args(&["explain", "NOPE123"])), 2);
}

#[test]
fn unknown_command_exits_2() {
    assert_eq!(real_main(args(&["bogus"])), 2);
}

#[test]
fn usage_exits_0() {
    assert_eq!(real_main(args(&["help"])), 0);
    assert_eq!(real_main(args(&[])), 0);
}
