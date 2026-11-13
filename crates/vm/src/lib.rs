//! Lutetium bytecode runtime: module format, verifier and stack-free
//! register-machine interpreter.

pub mod bytecode;
pub mod heap;
pub mod verify;
pub mod vm;

pub use bytecode::{
    deserialize, disassemble, serialize, BcConst, BcError, BcFunction, BcTy, BuiltinKind,
    BytecodeModule, FORMAT_VERSION,
};
pub use heap::{GcStats, Heap, Obj};
pub use verify::{verify, VerifyError};
pub use vm::{format_value, run, run_with_options, Value, VmError, VmOptions};
