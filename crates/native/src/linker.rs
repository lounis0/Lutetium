//! Windows executable linking for the native backend.
//!
//! Generated COFF objects are linked with the MinGW-w64 toolchain that ships
//! inside the rustup GNU toolchain (self-contained: `crt2.o`, import
//! libraries, and a GCC driver wrapper). The linker is used only during
//! compilation — the produced executable has no dependency on Lutetium or on
//! this source tree.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct LinkError(pub String);

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[E6003] native linking failed: {}", self.0)
    }
}

fn sysroot() -> Result<PathBuf, LinkError> {
    let out = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .map_err(|e| LinkError(format!("cannot locate the Rust toolchain (rustc): {}", e)))?;
    if !out.status.success() {
        return Err(LinkError("rustc --print sysroot failed".to_string()));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return Err(LinkError(
            "rustc --print sysroot produced no output".to_string(),
        ));
    }
    Ok(PathBuf::from(s))
}

/// Paths to the self-contained MinGW pieces shipped with the rustup GNU
/// toolchain.
pub struct Toolchain {
    /// `x86_64-w64-mingw32-gcc.exe` driver wrapper (forwards to the linker).
    pub gcc: PathBuf,
    /// Directory with `crt2.o`, `libkernel32.a`, `libmsvcrt.a`, ...
    pub libs: PathBuf,
}

pub fn find_toolchain() -> Result<Toolchain, LinkError> {
    let root = sysroot()?;
    let base = root
        .join("lib")
        .join("rustlib")
        .join("x86_64-pc-windows-gnu");
    let gcc = base
        .join("bin")
        .join("self-contained")
        .join("x86_64-w64-mingw32-gcc.exe");
    let libs = base.join("lib").join("self-contained");
    if !gcc.exists() {
        return Err(LinkError(format!(
            "MinGW GCC driver not found at {} (is the x86_64-pc-windows-gnu toolchain installed?)",
            gcc.display()
        )));
    }
    if !libs.exists() {
        return Err(LinkError(format!(
            "self-contained MinGW libraries not found at {}",
            libs.display()
        )));
    }
    Ok(Toolchain { gcc, libs })
}

/// Link `object_path` (COFF) plus the Lutetium runtime staticlib into
/// `out_exe`. `runtime_lib` is the embedded runtime artifact path.
pub fn link_exe(object_path: &Path, runtime_lib: &Path, out_exe: &Path) -> Result<(), LinkError> {
    link_exe_with_subsystem(object_path, runtime_lib, out_exe, false)
}

/// Link with the Windows GUI subsystem when requested.
pub fn link_gui_exe(
    object_path: &Path,
    runtime_lib: &Path,
    out_exe: &Path,
) -> Result<(), LinkError> {
    link_exe_with_subsystem(object_path, runtime_lib, out_exe, true)
}

fn link_exe_with_subsystem(
    object_path: &Path,
    runtime_lib: &Path,
    out_exe: &Path,
    gui: bool,
) -> Result<(), LinkError> {
    let tc = find_toolchain()?;
    let mut cmd = Command::new(&tc.gcc);
    // The driver must find `ld.exe` (and friends) shipped next to it.
    let mut bin_dir = tc.gcc.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    if bin_dir.as_os_str().is_empty() {
        bin_dir = std::env::current_dir().unwrap_or_default();
    }
    let old_path = env::var_os("PATH").unwrap_or_default();
    let new_path = std::env::join_paths(
        std::iter::once(bin_dir.clone()).chain(std::env::split_paths(&old_path)),
    )
    .map_err(|e| LinkError(e.to_string()))?;
    cmd.env("PATH", &new_path);
    cmd.arg("-o").arg(out_exe);
    // rust-mingw does not ship crtbegin.o/crtend.o; rustc itself links with
    // -nostartfiles and provides crt2.o from the self-contained directory.
    cmd.arg("-nostartfiles");
    cmd.arg(tc.libs.join("crt2.o"));
    cmd.arg(object_path);
    cmd.arg(runtime_lib);
    cmd.arg(format!("-L{}", tc.libs.display()));
    // Search paths for startup objects (crt2.o, crtbegin.o) and subprograms.
    cmd.arg(format!("-B{}", tc.libs.display()));
    if let Some(bin) = tc.gcc.parent() {
        cmd.arg(format!("-B{}", bin.display()));
    }
    // The bundled driver is not a full GCC; disable the LTO plugin lookup.
    cmd.arg("-fno-use-linker-plugin");
    // rustc's runtime bits reference these beyond the mingw defaults.
    cmd.arg("-lws2_32");
    // Rust 1.96 and newer resolve std's Windows futex wait/wake primitives
    // through this import library in rustup's self-contained MinGW sysroot.
    cmd.arg("-lsynchronization");
    cmd.arg("-lbcrypt");
    cmd.arg("-ladvapi32");
    cmd.arg("-luserenv");
    cmd.arg("-lntdll");
    if gui {
        cmd.arg("-Wl,--subsystem,windows");
        cmd.arg("-luser32");
        cmd.arg("-lgdi32");
    } else {
        cmd.arg("-Wl,--subsystem,console");
    }
    let out = cmd
        .output()
        .map_err(|e| LinkError(format!("failed to run the linker driver: {}", e)))?;
    if !out.status.success() {
        return Err(LinkError(format!(
            "linker exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    if !out_exe.exists() {
        return Err(LinkError(format!(
            "linker reported success but {} was not produced",
            out_exe.display()
        )));
    }
    Ok(())
}
