//! Typed intermediate representation.
//!
//! The IR is backend independent: it is produced by the type checker + lower
//! pass and consumed by the bytecode backend today, and by the native
//! (Cranelift) backend in a later milestone. Control flow uses basic blocks
//! with structured terminators. Registers are explicit: locals occupy
//! `r0..r{nlocals-1}`, temporaries follow.

use lu_vm::bytecode::BuiltinKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Str,
    Unit,
    /// A list value (element types are tracked by the checker, not here).
    List,
    /// A map value (string keys; the value type is tracked by the checker).
    Map,
    /// Opaque native GUI window handle.
    Window,
    /// Opaque native GUI widget handle.
    Widget,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Float => "float",
            Ty::Bool => "bool",
            Ty::Str => "str",
            Ty::Unit => "unit",
            Ty::List => "list",
            Ty::Map => "map",
            Ty::Window => "window",
            Ty::Widget => "widget",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
}

#[derive(Debug, Clone)]
pub enum IrInstr {
    Const {
        v: Const,
        dst: u16,
    },
    Move {
        src: u16,
        dst: u16,
    },
    Bin {
        op: BinOp,
        ty: Ty,
        a: u16,
        b: u16,
        dst: u16,
    },
    Un {
        op: UnOp,
        ty: Ty,
        a: u16,
        dst: u16,
    },
    IntToFloat {
        a: u16,
        dst: u16,
    },
    Call {
        func: usize,
        args: Vec<u16>,
        dst: u16,
    },
    /// Built-in operation (list, string, math, fs). Lowered to the BUILTIN
    /// opcode by the bytecode backend; the native backend must support the
    /// kind or reject the module up front.
    Builtin {
        kind: BuiltinKind,
        args: Vec<u16>,
        dst: u16,
    },
    /// Native-only button with a statically resolved Lutetium callback.
    GuiButton { args: Vec<u16>, handler: usize, dst: u16 },
    Print {
        src: u16,
        ty: Ty,
    },
}

#[derive(Debug, Clone)]
pub enum IrTerm {
    Jmp(usize),
    Br { cond: u16, then: usize, els: usize },
    Ret(u16),
    RetUnit,
}

#[derive(Debug, Clone, Default)]
pub struct IrBlock {
    pub instrs: Vec<IrInstr>,
    pub term: Option<IrTerm>,
}

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<Ty>,
    pub ret: Ty,
    pub nlocals: u16,
    pub nregs: u16,
    /// Type of every register (params, locals, temporaries); needed by the
    /// native backend to pick Cranelift types and runtime calls.
    pub reg_tys: Vec<Ty>,
    pub blocks: Vec<IrBlock>,
}

#[derive(Debug, Clone)]
pub struct IrModule {
    pub funcs: Vec<IrFunction>,
    /// Index into `funcs` of the entry point (`main` or `__lutetium_init`).
    pub entry: usize,
}

// ---------------------------------------------------------------------------
// IR disassembly (`lutetium inspect file.lu --ir`)
// ---------------------------------------------------------------------------

fn ty_name(t: &Ty) -> &'static str {
    match t {
        Ty::Int => "int",
        Ty::Float => "float",
        Ty::Bool => "bool",
        Ty::Str => "str",
        Ty::Unit => "unit",
        Ty::List => "list",
        Ty::Map => "map",
        Ty::Window => "window",
        Ty::Widget => "widget",
    }
}

fn const_text(c: &Const) -> String {
    match c {
        Const::Int(v) => v.to_string(),
        Const::Float(v) => v.to_string(),
        Const::Bool(v) => v.to_string(),
        Const::Str(s) => format!("{:?}", s),
        Const::Unit => "unit".to_string(),
    }
}

fn op_text(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Mod => "mod",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Gt => "gt",
        BinOp::Ge => "ge",
    }
}

fn un_text(op: &UnOp) -> &'static str {
    match op {
        UnOp::Neg => "neg",
        UnOp::Not => "not",
    }
}

fn instr_text(i: &IrInstr) -> String {
    match i {
        IrInstr::Const { v, dst } => format!("const {} -> r{}", const_text(v), dst),
        IrInstr::Move { src, dst } => format!("move r{}, r{}", src, dst),
        IrInstr::Bin { op, ty, a, b, dst } => {
            format!("{}{} r{}, r{}, r{}", op_text(op), ty_name(ty), a, b, dst)
        }
        IrInstr::Un { op, ty, a, dst } => {
            format!("{}{} r{}, r{}", un_text(op), ty_name(ty), a, dst)
        }
        IrInstr::IntToFloat { a, dst } => format!("int_to_float r{}, r{}", a, dst),
        IrInstr::Call { func, args, dst } => {
            let args_s = args
                .iter()
                .map(|a| format!("r{}", a))
                .collect::<Vec<_>>()
                .join(", ");
            format!("call #{}({}) -> r{}", func, args_s, dst)
        }
        IrInstr::Builtin { kind, args, dst } => {
            let args_s = args
                .iter()
                .map(|a| format!("r{}", a))
                .collect::<Vec<_>>()
                .join(", ");
            format!("builtin {} ({}) -> r{}", kind.name(), args_s, dst)
        }
        IrInstr::GuiButton { args, handler, dst } => format!("gui_button {:?}, handler #{} -> r{}", args, handler, dst),
        IrInstr::Print { src, ty } => format!("print{} r{}", ty_name(ty), src),
    }
}

fn term_text(t: &IrTerm) -> String {
    match t {
        IrTerm::Jmp(b) => format!("jmp b{}", b),
        IrTerm::Br { cond, then, els } => format!("br r{}, b{}, b{}", cond, then, els),
        IrTerm::Ret(r) => format!("ret r{}", r),
        IrTerm::RetUnit => "ret_unit".to_string(),
    }
}

/// Human-readable IR dump.
pub fn disassemble_ir(m: &IrModule) -> String {
    let mut out = String::new();
    for (i, f) in m.funcs.iter().enumerate() {
        let entry = if i == m.entry { " [entry]" } else { "" };
        out.push_str(&format!(
            "fn {} #{} ({} register(s), ret {}){}\n",
            f.name,
            i,
            f.nregs,
            ty_name(&f.ret),
            entry
        ));
        for (bi, b) in f.blocks.iter().enumerate() {
            out.push_str(&format!("  b{}:\n", bi));
            for instr in &b.instrs {
                out.push_str(&format!("    {}\n", instr_text(instr)));
            }
            out.push_str(&format!(
                "    {}\n",
                term_text(b.term.as_ref().unwrap_or(&IrTerm::RetUnit))
            ));
        }
        out.push('\n');
    }
    out
}
