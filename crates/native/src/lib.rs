//! Native (Cranelift) backend for Lutetium.
//!
//! Pipeline: typed IR (shared with the bytecode backend) -> Cranelift
//! functions -> COFF object (`backend.rs`) -> linked Windows executable
//! (`linker.rs`) with the embedded Lutetium runtime (`crates/runtime`,
//! built by `build.rs`). MSVC is never required: the linker pieces come from
//! rustup's self-contained MinGW-w64 toolchain.

pub mod backend;
pub mod linker;

pub use backend::NativeError;
pub use linker::LinkError;

use crate::backend::Backend;
use cranelift_codegen::isa;
use lu_compiler::ir::IrModule;

pub const TARGET_TRIPLE: &str = "x86_64-pc-windows-gnu";

/// Path of the runtime staticlib artifact, embedded at compiler build time.
pub const RUNTIME_LIB_BYTES: &[u8] = include_bytes!(env!("LUTETIUM_RUNTIME_PATH"));

/// Construct the Cranelift ISA for the Windows GNU target.
pub fn target_isa() -> Result<std::sync::Arc<dyn isa::TargetIsa>, NativeError> {
    let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
    let triple = target_lexicon::triple!("x86_64-pc-windows-gnu");
    isa::lookup(triple)
        .map_err(|e| NativeError(format!("target lookup failed: {}", e)))
        .and_then(|b| {
            b.finish(flags)
                .map_err(|e| NativeError(format!("ISA construction failed: {}", e)))
        })
}

/// Compile a typed IR module to a COFF object file image.
pub fn emit_object(ir: &IrModule) -> Result<Vec<u8>, NativeError> {
    let isa = target_isa()?;
    Backend::new(isa, ir)?.translate()?.finish()
}

/// Compile a typed IR module all the way to an executable:
/// IR -> COFF object (written next to the exe as `<stem>.obj`) -> linked exe.
/// The runtime staticlib is materialized next to the exe as
/// `<stem>.runtime.a` — derived entirely from the embedded bytes, so the
/// per-exe name keeps concurrent builds in one directory from clobbering
/// each other's runtime archive.
pub fn build_exe(ir: &IrModule, out_exe: &std::path::Path) -> Result<(), String> {
    build_exe_subsystem(ir, out_exe, false)
}

/// Build a standalone Windows GUI-subsystem executable.
pub fn build_gui_exe(ir: &IrModule, out_exe: &std::path::Path) -> Result<(), String> {
    build_exe_subsystem(ir, out_exe, true)
}

fn build_exe_subsystem(ir: &IrModule, out_exe: &std::path::Path, gui: bool) -> Result<(), String> {
    let obj_bytes = emit_object(ir).map_err(|e| e.to_string())?;
    let obj_path = out_exe.with_extension("obj");
    std::fs::write(&obj_path, &obj_bytes)
        .map_err(|e| format!("cannot write {}: {}", obj_path.display(), e))?;
    let runtime_path = out_exe.with_extension("runtime.a");
    std::fs::write(&runtime_path, RUNTIME_LIB_BYTES)
        .map_err(|e| format!("cannot write {}: {}", runtime_path.display(), e))?;
    if gui {
        linker::link_gui_exe(&obj_path, &runtime_path, out_exe).map_err(|e| e.to_string())
    } else {
        linker::link_exe(&obj_path, &runtime_path, out_exe).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cranelift_supports_windows_gnu() {
        let isa = target_isa().expect("cranelift must support x86_64-pc-windows-gnu");
        assert_eq!(isa.name(), "x64");
        assert_eq!(TARGET_TRIPLE, "x86_64-pc-windows-gnu");
    }

    #[test]
    fn runtime_artifact_is_embedded() {
        assert!(RUNTIME_LIB_BYTES.len() > 16, "runtime staticlib too small");
        assert_eq!(&RUNTIME_LIB_BYTES[0..8], b"!<arch>\n");
    }
}
