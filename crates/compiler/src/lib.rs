//! Lutetium compiler front end: name resolution, type checking, typed IR and
//! bytecode code generation.

pub mod codegen;
pub mod ir;
pub mod lower;
pub mod optimizer;
pub mod pipeline;
pub mod resolve;
pub mod typecheck;

pub use pipeline::{compile_ir, compile_ir_with_opts, compile_source};
