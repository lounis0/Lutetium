//! Whole-compiler pipeline: source text -> verified bytecode module.

use crate::codegen::codegen;
use crate::ir::IrModule;
use crate::lower::lower;
use crate::resolve::resolve;
use crate::typecheck::check;
use lu_diagnostics::Diagnostic;
use lu_syntax::{lex, parse};
use lu_vm::bytecode::BytecodeModule;

/// Compile Lutetium source to the typed IR (shared by both backends).
pub fn compile_ir(name: &str, src: &str) -> Result<IrModule, Vec<Diagnostic>> {
    compile_ir_with_opts(name, src, 0)
}

/// Compile with an optimization level (see `crate::optimizer`).
pub fn compile_ir_with_opts(_name: &str, src: &str, opt: u8) -> Result<IrModule, Vec<Diagnostic>> {
    let (tokens, lex_diags) = lex(src);
    if !lex_diags.is_empty() {
        return Err(lex_diags);
    }
    let module = parse(tokens)?;
    let sigs = resolve(&module)?;
    let tmodule = check(&module, &sigs)?;
    let mut ir = lower(&tmodule);
    crate::optimizer::optimize(&mut ir, opt);
    Ok(ir)
}

/// Compile Lutetium source into bytecode. Returns every diagnostic on failure.
pub fn compile_source(_name: &str, src: &str) -> Result<BytecodeModule, Vec<Diagnostic>> {
    Ok(codegen(&compile_ir(_name, src)?))
}
