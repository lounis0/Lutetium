//! Bytecode backend: typed IR -> `.lbc` module.

use crate::ir::{BinOp, Const, IrInstr, IrModule, IrTerm, Ty, UnOp};
use lu_vm::bytecode::{op, BcConst, BcFunction, BcTy, BuiltinKind, BytecodeModule};
use std::collections::HashMap;

pub fn codegen(ir: &IrModule) -> BytecodeModule {
    let mut consts: Vec<BcConst> = Vec::new();
    let mut num_pool: HashMap<(u8, u64), u32> = HashMap::new();
    let mut str_pool: HashMap<String, u32> = HashMap::new();
    let funcs = ir
        .funcs
        .iter()
        .map(|f| {
            let mut code = Vec::new();
            let mut offsets = vec![0usize; f.blocks.len()];
            let mut patches: Vec<(usize, usize)> = Vec::new(); // (code position, block id)

            for (bi, block) in f.blocks.iter().enumerate() {
                offsets[bi] = code.len();
                for instr in &block.instrs {
                    match instr {
                        IrInstr::Const { v, dst } => match v {
                            Const::Int(i) => {
                                let ci = intern_num(
                                    &mut consts,
                                    &mut num_pool,
                                    1,
                                    *i as u64,
                                    BcConst::Int(*i),
                                );
                                push(&mut code, op::CONST_INT);
                                put_u32(&mut code, ci);
                                put_u16(&mut code, *dst);
                            }
                            Const::Float(fl) => {
                                let ci = intern_num(
                                    &mut consts,
                                    &mut num_pool,
                                    2,
                                    fl.to_bits(),
                                    BcConst::Float(*fl),
                                );
                                push(&mut code, op::CONST_FLOAT);
                                put_u32(&mut code, ci);
                                put_u16(&mut code, *dst);
                            }
                            Const::Bool(b) => {
                                let ci = intern_num(
                                    &mut consts,
                                    &mut num_pool,
                                    3,
                                    *b as u64,
                                    BcConst::Bool(*b),
                                );
                                push(&mut code, op::CONST_BOOL);
                                put_u32(&mut code, ci);
                                put_u16(&mut code, *dst);
                            }
                            Const::Str(s) => {
                                let ci = intern_str(&mut consts, &mut str_pool, s);
                                push(&mut code, op::CONST_STR);
                                put_u32(&mut code, ci);
                                put_u16(&mut code, *dst);
                            }
                            Const::Unit => {
                                push(&mut code, op::CONST_UNIT);
                                put_u16(&mut code, *dst);
                            }
                        },
                        IrInstr::Move { src, dst } => {
                            push(&mut code, op::MOVE);
                            put_u16(&mut code, *src);
                            put_u16(&mut code, *dst);
                        }
                        IrInstr::Bin {
                            op: bop,
                            ty,
                            a,
                            b,
                            dst,
                        } => {
                            push(&mut code, bin_opcode(*bop, *ty));
                            put_u16(&mut code, *a);
                            put_u16(&mut code, *b);
                            put_u16(&mut code, *dst);
                        }
                        IrInstr::Un {
                            op: uop,
                            ty,
                            a,
                            dst,
                        } => {
                            push(&mut code, un_opcode(*uop, *ty));
                            put_u16(&mut code, *a);
                            put_u16(&mut code, *dst);
                        }
                        IrInstr::IntToFloat { a, dst } => {
                            push(&mut code, op::INT_TO_FLOAT);
                            put_u16(&mut code, *a);
                            put_u16(&mut code, *dst);
                        }
                        IrInstr::Call { func, args, dst } => {
                            push(&mut code, op::CALL);
                            put_u16(&mut code, *func as u16);
                            push(&mut code, args.len() as u8);
                            for a in args {
                                put_u16(&mut code, *a);
                            }
                            put_u16(&mut code, *dst);
                        }
                        IrInstr::Builtin { kind, args, dst } => {
                            push(&mut code, op::BUILTIN);
                            push(&mut code, kind.kind_byte());
                            put_u16(&mut code, *dst);
                            push(&mut code, args.len() as u8);
                            for a in args {
                                put_u16(&mut code, *a);
                            }
                        }
                        IrInstr::GuiButton { args, dst, .. } => {
                            // Keep the bytecode compiler total: the VM will report
                            // GUI as unavailable, while native builds lower the
                            // callback directly to a function pointer.
                            push(&mut code, op::BUILTIN);
                            push(&mut code, BuiltinKind::GuiButton.kind_byte());
                            put_u16(&mut code, *dst);
                            push(&mut code, args.len() as u8);
                            for a in args {
                                put_u16(&mut code, *a);
                            }
                        }
                        IrInstr::Print { src, ty: _ } => {
                            push(&mut code, op::PRINT);
                            put_u16(&mut code, *src);
                        }
                    }
                }
                match &block.term {
                    Some(IrTerm::Jmp(b)) => {
                        patches.push((code.len() + 1, *b));
                        push(&mut code, op::JUMP);
                        put_i32_placeholder(&mut code);
                    }
                    Some(IrTerm::Br { cond, then, els }) => {
                        patches.push((code.len() + 3, *els));
                        push(&mut code, op::JUMP_IF_FALSE);
                        put_u16(&mut code, *cond);
                        put_i32_placeholder(&mut code);
                        patches.push((code.len() + 1, *then));
                        push(&mut code, op::JUMP);
                        put_i32_placeholder(&mut code);
                    }
                    Some(IrTerm::Ret(r)) => {
                        push(&mut code, op::RET);
                        put_u16(&mut code, *r);
                    }
                    Some(IrTerm::RetUnit) => push(&mut code, op::RET_UNIT),
                    None => push(&mut code, op::RET_UNIT),
                }
            }

            for (pos, block) in patches {
                let target = offsets[block] as i32;
                code[pos..pos + 4].copy_from_slice(&target.to_le_bytes());
            }

            BcFunction {
                name: f.name.clone(),
                arity: f.params.len() as u8,
                nregs: f.nregs,
                ret: match f.ret {
                    Ty::Int => BcTy::Int,
                    Ty::Float => BcTy::Float,
                    Ty::Bool => BcTy::Bool,
                    Ty::Str => BcTy::Str,
                    Ty::Unit => BcTy::Unit,
                    Ty::List => BcTy::List,
                    Ty::Map => BcTy::Map,
                    Ty::Window => BcTy::Window,
                    Ty::Widget => BcTy::Widget,
                },
                code,
            }
        })
        .collect();

    BytecodeModule {
        consts,
        funcs,
        entry: ir.entry,
    }
}

fn push(code: &mut Vec<u8>, b: u8) {
    code.push(b);
}

fn put_u16(code: &mut Vec<u8>, v: u16) {
    code.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(code: &mut Vec<u8>, v: u32) {
    code.extend_from_slice(&v.to_le_bytes());
}

fn put_i32_placeholder(code: &mut Vec<u8>) {
    code.extend_from_slice(&0i32.to_le_bytes());
}

fn intern_num(
    consts: &mut Vec<BcConst>,
    pool: &mut HashMap<(u8, u64), u32>,
    tag: u8,
    bits: u64,
    value: BcConst,
) -> u32 {
    let key = (tag, bits);
    if let Some(&i) = pool.get(&key) {
        return i;
    }
    let i = consts.len() as u32;
    consts.push(value);
    pool.insert(key, i);
    i
}

fn intern_str(consts: &mut Vec<BcConst>, pool: &mut HashMap<String, u32>, s: &str) -> u32 {
    if let Some(&i) = pool.get(s) {
        return i;
    }
    let i = consts.len() as u32;
    consts.push(BcConst::Str(s.to_string()));
    pool.insert(s.to_string(), i);
    i
}

fn bin_opcode(op_: BinOp, ty: Ty) -> u8 {
    use op::*;
    match (op_, ty) {
        (BinOp::Add, Ty::Int) => ADD_INT,
        (BinOp::Sub, Ty::Int) => SUB_INT,
        (BinOp::Mul, Ty::Int) => MUL_INT,
        (BinOp::Div, Ty::Int) => DIV_INT,
        (BinOp::Mod, Ty::Int) => MOD_INT,
        (BinOp::Add, Ty::Float) => ADD_FLOAT,
        (BinOp::Sub, Ty::Float) => SUB_FLOAT,
        (BinOp::Mul, Ty::Float) => MUL_FLOAT,
        (BinOp::Div, Ty::Float) => DIV_FLOAT,
        (BinOp::Add, Ty::Str) => CONCAT_STR,
        (BinOp::Eq, Ty::Int) => EQ_INT,
        (BinOp::Ne, Ty::Int) => NE_INT,
        (BinOp::Lt, Ty::Int) => LT_INT,
        (BinOp::Le, Ty::Int) => LE_INT,
        (BinOp::Gt, Ty::Int) => GT_INT,
        (BinOp::Ge, Ty::Int) => GE_INT,
        (BinOp::Eq, Ty::Float) => EQ_FLOAT,
        (BinOp::Ne, Ty::Float) => NE_FLOAT,
        (BinOp::Lt, Ty::Float) => LT_FLOAT,
        (BinOp::Le, Ty::Float) => LE_FLOAT,
        (BinOp::Gt, Ty::Float) => GT_FLOAT,
        (BinOp::Ge, Ty::Float) => GE_FLOAT,
        (BinOp::Eq, Ty::Str) => EQ_STR,
        (BinOp::Ne, Ty::Str) => NE_STR,
        (BinOp::Eq, Ty::Bool) => EQ_BOOL,
        (BinOp::Ne, Ty::Bool) => NE_BOOL,
        (BinOp::Eq, Ty::List) => EQ_LIST,
        (BinOp::Ne, Ty::List) => NE_LIST,
        _ => unreachable!("unsupported ({op_:?}, {ty:?}) combination"),
    }
}

fn un_opcode(op_: UnOp, ty: Ty) -> u8 {
    use op::*;
    match op_ {
        UnOp::Not => NOT,
        UnOp::Neg => match ty {
            Ty::Int => NEG_INT,
            Ty::Float => NEG_FLOAT,
            _ => unreachable!("unary `-` on non-numeric type"),
        },
    }
}
