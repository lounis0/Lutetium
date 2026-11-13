//! Builds the Lutetium native runtime staticlib with plain rustc and exposes
//! its path to the native backend via `LUTETIUM_RUNTIME_PATH`.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../runtime/src/lib.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifest_dir
        .parent()
        .expect("crates/native has a parent")
        .join("runtime")
        .join("src")
        .join("lib.rs");

    let output_name = if cfg!(target_env = "gnu") {
        "liblutetium_runtime.a"
    } else {
        "lutetium_runtime.lib"
    };
    let artifact = out_dir.join(output_name);

    let status = Command::new("rustc")
        .arg("--edition=2021")
        .arg("--crate-type=staticlib")
        .arg("--crate-name=lutetium_runtime")
        .arg("-Copt-level=3")
        .arg("-Cdebug-assertions=off")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&src)
        .status()
        .expect("failed to launch rustc to build the native runtime");
    if !status.success() {
        panic!("building the Lutetium native runtime failed");
    }
    assert!(
        artifact.exists(),
        "runtime artifact {} missing",
        artifact.display()
    );

    println!(
        "cargo:rustc-env=LUTETIUM_RUNTIME_PATH={}",
        artifact.display()
    );
}
