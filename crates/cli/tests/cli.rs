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
    assert_eq!(lutetium_cli::VERSION, "0.4.4");
    assert_eq!(real_main(args(&["--version"])), 0);
}

#[test]
fn check_ok_and_json() {
    let f = repo("examples/fibonacci.lu");
    assert_eq!(real_main(vec!["check".into(), f.clone()]), 0);
    assert_eq!(
        real_main(vec![
            "check".into(),
            "--diagnostics".into(),
            "json".into(),
            f
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
fn new_command_scaffolds_a_checkable_console_project() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("lutetium_cli_new_{nonce}");
    let root = std::path::Path::new(&name);
    assert!(!root.exists(), "test project path must be unique");
    assert_eq!(real_main(vec!["new".into(), name.clone()]), 0);
    assert!(root.join("lutetium.toml").is_file());
    assert!(root.join("src/main.lu").is_file());
    assert_eq!(real_main(vec!["check".into(), name.clone()]), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn gui_template_project_builds_a_windows_gui_executable() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("lutetium_cli_gui_project_{nonce}");
    let root = std::path::PathBuf::from(&name);
    assert!(!root.exists(), "test project path must be unique");
    assert_eq!(
        real_main(vec!["new".into(), name.clone(), "--template=gui".into()]),
        0
    );
    let project = root.to_string_lossy().into_owned();
    assert_eq!(real_main(vec!["check".into(), project.clone()]), 0);
    assert_eq!(real_main(vec!["build".into(), project]), 0);
    let exe = root.join(format!("dist/{name}.exe"));
    assert!(exe.is_file(), "GUI project executable should be produced");
    let pe = std::fs::read(&exe).unwrap();
    assert!(pe.len() >= 0x40 && &pe[..2] == b"MZ");
    let pe_offset = u32::from_le_bytes(pe[0x3c..0x40].try_into().unwrap()) as usize;
    let subsystem_offset = pe_offset + 4 + 20 + 68;
    assert_eq!(
        u16::from_le_bytes(
            pe[subsystem_offset..subsystem_offset + 2]
                .try_into()
                .unwrap()
        ),
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_directory_supports_check_run_and_bytecode_build() {
    let _guard = BUILD_LOCK.lock().unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lutetium_cli_project_{nonce}"));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("lutetium.toml"),
        "name = \"ProjectSmoke\"\nentry = \"src/main.lu\"\nkind = \"console\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.lu"),
        "fn main() {\n    print(6 * 7)\n}\n",
    )
    .unwrap();
    let path = root.to_string_lossy().into_owned();

    assert_eq!(real_main(vec!["check".into(), path.clone()]), 0);
    assert_eq!(real_main(vec!["run".into(), path.clone()]), 0);
    // A console project defaults to the native target and produces a runnable
    // project-local artifact; explicit bytecode output remains available.
    assert_eq!(real_main(vec!["build".into(), path.clone()]), 0);
    let exe = root.join("dist/ProjectSmoke.exe");
    let output = std::process::Command::new(&exe)
        .output()
        .expect("project native executable should launch");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
    assert_eq!(
        real_main(vec![
            "build".into(),
            path,
            "--target".into(),
            "bytecode".into(),
        ]),
        0
    );
    assert!(root.join("dist/ProjectSmoke.lbc").is_file());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_manifest_errors_have_a_command_error_exit_code() {
    let root = std::env::temp_dir().join(format!(
        "lutetium_cli_bad_project_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("lutetium.toml"),
        "name = \"Bad\"\nentry = \"../outside.lu\"\nkind = \"console\"\n",
    )
    .unwrap();
    assert_eq!(
        real_main(vec!["check".into(), root.to_string_lossy().into_owned()]),
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_entry_symlink_cannot_escape_project_root() {
    let root = std::env::temp_dir().join(format!(
        "lutetium_cli_project_symlink_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let outside =
        std::env::temp_dir().join(format!("lutetium_cli_outside_{}.lu", std::process::id()));
    std::fs::write(&outside, "fn main() {}\n").unwrap();
    std::fs::write(
        root.join("lutetium.toml"),
        "name = \"Symlink\"\nentry = \"src/main.lu\"\nkind = \"console\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("src/main.lu")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&outside, root.join("src/main.lu")).unwrap();
    assert_eq!(
        real_main(vec!["check".into(), root.to_string_lossy().into_owned()]),
        2
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_file(outside).unwrap();
}

#[test]
fn cli_flags_reject_unknown_duplicate_and_missing_values() {
    let src = repo("examples/hello.lu");
    for args in [
        vec!["check".into(), src.clone(), "--unknown".into()],
        vec!["check".into(), src.clone(), "--diagnostics".into()],
        vec!["check".into(), src.clone(), "--diagnostics=xml".into()],
        vec![
            "check".into(),
            src.clone(),
            "--diagnostics=json".into(),
            "--diagnostics".into(),
            "json".into(),
        ],
        vec!["run".into(), src.clone(), "--gc-stats=true".into()],
        vec![
            "run".into(),
            src.clone(),
            "--gc-stats".into(),
            "--gc-stats".into(),
        ],
        vec![
            "build".into(),
            src.clone(),
            "--target".into(),
            "invalid".into(),
        ],
        vec!["build".into(), src.clone(), "--target".into()],
        vec!["build".into(), src.clone(), "--opt".into(), "4".into()],
        vec!["build".into(), src.clone(), "--release=true".into()],
        vec!["build".into(), src.clone(), "--unknown".into()],
        vec!["check".into(), src.clone(), repo("examples/fibonacci.lu")],
    ] {
        assert_eq!(
            real_main(args.clone()),
            2,
            "arguments should be rejected: {args:?}"
        );
    }
    assert_eq!(
        real_main(vec!["check".into(), "--diagnostics=json".into(), src]),
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
        real_main(vec!["build".into(), "--opt".into(), "1".into(), src]),
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
    for code in ["E3001", "E5009", "E6002", "E6003"] {
        assert_eq!(real_main(args(&["explain", code])), 0);
    }
    for retired in ["E2005", "E6001"] {
        assert_eq!(real_main(args(&["explain", retired])), 2);
    }
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
    assert_eq!(
        real_main(args(&["new", "Bad/Name", "--template", "console"])),
        2
    );
    assert_eq!(
        real_main(args(&["new", "BadTemplate", "--template", "json"])),
        2
    );
    assert_eq!(
        real_main(args(&["new", "--template", "console", "BadOrder"])),
        0
    );
    std::fs::remove_dir_all("BadOrder").unwrap();
    assert_eq!(
        real_main(args(&[
            "new",
            "Duplicate",
            "--template",
            "console",
            "--template=gui"
        ])),
        2
    );
}
