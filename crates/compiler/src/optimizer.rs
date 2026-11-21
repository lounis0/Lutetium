//! IR optimizer.
//!
//! Behavior-preserving passes over the typed IR. Optimization levels:
//!
//! * `0` — no optimization.
//! * `1` — constant folding + unreachable block elimination.
//! * `2` — level 1 + dead temporary elimination + block-local common
//!   subexpression elimination.
//! * `3` — currently identical to level 2; aggressive inlining and loop
//!   optimizations are reserved for a later milestone (documented, not a
//!   silent no-op).
//!
//! Every pass is tested for output equivalence against level 0.

use crate::ir::{BinOp, Const, IrFunction, IrInstr, IrModule, IrTerm, Ty};

pub fn optimize(module: &mut IrModule, level: u8) {
    if level == 0 {
        return;
    }
    for f in module.funcs.iter_mut() {
        if level >= 1 {
            fold_constants(f);
            remove_unreachable_blocks(f);
        }
        if level >= 2 {
            eliminate_dead_temps(f);
            block_local_cse(f);
        }
    }
}

/// Fold `Const` values through `Bin`/`Un`/`IntToFloat` in a single linear
/// pass per block, rewriting instruction streams in place.
fn fold_constants(f: &mut IrFunction) {
    for block in f.blocks.iter_mut() {
        // map register -> constant for temps defined in this block
        let mut consts: std::collections::HashMap<u16, Const> = std::collections::HashMap::new();
        let mut out: Vec<IrInstr> = Vec::with_capacity(block.instrs.len());
        for instr in block.instrs.drain(..) {
            let folded = match instr {
                IrInstr::Const { v, dst } => {
                    consts.insert(dst, v.clone());
                    IrInstr::Const { v, dst }
                }
                IrInstr::Move { src, dst } => {
                    if let Some(c) = consts.get(&src) {
                        let c = c.clone();
                        consts.insert(dst, c.clone());
                        IrInstr::Const { v: c, dst }
                    } else {
                        consts.remove(&dst);
                        IrInstr::Move { src, dst }
                    }
                }
                IrInstr::Bin { op, ty, a, b, dst } => {
                    let ca = consts.get(&a).cloned();
                    let cb = consts.get(&b).cloned();
                    match fold_bin(op, ty, ca, cb) {
                        Some(c) => {
                            consts.insert(dst, c.clone());
                            IrInstr::Const { v: c, dst }
                        }
                        None => {
                            consts.remove(&dst);
                            IrInstr::Bin { op, ty, a, b, dst }
                        }
                    }
                }
                IrInstr::Un { op, ty, a, dst } => {
                    let ca = consts.get(&a).cloned();
                    match fold_un(op, ty, ca) {
                        Some(c) => {
                            consts.insert(dst, c.clone());
                            IrInstr::Const { v: c, dst }
                        }
                        None => {
                            consts.remove(&dst);
                            IrInstr::Un { op, ty, a, dst }
                        }
                    }
                }
                IrInstr::IntToFloat { a, dst } => {
                    if let Some(Const::Int(i)) = consts.get(&a) {
                        let c = Const::Float(*i as f64);
                        consts.insert(dst, c.clone());
                        IrInstr::Const { v: c, dst }
                    } else {
                        consts.remove(&dst);
                        IrInstr::IntToFloat { a, dst }
                    }
                }
                IrInstr::Call { func, args, dst } => {
                    // calls are not folded (side effects); invalidate dst
                    consts.remove(&dst);
                    // but constants may flow through args unchanged
                    IrInstr::Call { func, args, dst }
                }
                IrInstr::Builtin { kind, args, dst } => {
                    // builtins are not folded here (they may have side
                    // effects, e.g. ListPush/WriteText); invalidate dst
                    consts.remove(&dst);
                    IrInstr::Builtin { kind, args, dst }
                }
                IrInstr::GuiButton { args, handler, dst } => {
                    consts.remove(&dst);
                    IrInstr::GuiButton { args, handler, dst }
                }
                IrInstr::Print { src, ty } => IrInstr::Print { src, ty },
            };
            out.push(folded);
        }
        block.instrs = out;
    }
}

fn fold_bin(op: BinOp, ty: Ty, a: Option<Const>, b: Option<Const>) -> Option<Const> {
    let (a, b) = (a?, b?);
    Some(match (op, ty) {
        (BinOp::Add, Ty::Int) => {
            let (Const::Int(x), Const::Int(y)) = (&a, &b) else {
                return None;
            };
            Const::Int(x.checked_add(*y)?)
        }
        (BinOp::Sub, Ty::Int) => {
            let (Const::Int(x), Const::Int(y)) = (&a, &b) else {
                return None;
            };
            Const::Int(x.checked_sub(*y)?)
        }
        (BinOp::Mul, Ty::Int) => {
            let (Const::Int(x), Const::Int(y)) = (&a, &b) else {
                return None;
            };
            Const::Int(x.checked_mul(*y)?)
        }
        (BinOp::Div, Ty::Int) => {
            let (Const::Int(x), Const::Int(y)) = (&a, &b) else {
                return None;
            };
            if *y == 0 {
                return None; // keep the runtime panic path
            }
            Const::Int(x.checked_div(*y)?)
        }
        (BinOp::Mod, Ty::Int) => {
            let (Const::Int(x), Const::Int(y)) = (&a, &b) else {
                return None;
            };
            if *y == 0 {
                return None;
            }
            Const::Int(x.checked_rem(*y)?)
        }
        (BinOp::Add, Ty::Float)
        | (BinOp::Sub, Ty::Float)
        | (BinOp::Mul, Ty::Float)
        | (BinOp::Div, Ty::Float) => {
            let (Const::Float(x), Const::Float(y)) = (&a, &b) else {
                return None;
            };
            Const::Float(match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                _ => x / y,
            })
        }
        (BinOp::Add, Ty::Str) => {
            let (Const::Str(x), Const::Str(y)) = (&a, &b) else {
                return None;
            };
            Const::Str(format!("{}{}", x, y))
        }
        (BinOp::Eq, _) | (BinOp::Ne, _) => {
            let eq = match (&a, &b) {
                (Const::Int(x), Const::Int(y)) => x == y,
                (Const::Float(x), Const::Float(y)) => x == y,
                (Const::Bool(x), Const::Bool(y)) => x == y,
                (Const::Str(x), Const::Str(y)) => x == y,
                _ => return None,
            };
            Const::Bool(if op == BinOp::Eq { eq } else { !eq })
        }
        (BinOp::Lt, _) | (BinOp::Le, _) | (BinOp::Gt, _) | (BinOp::Ge, _) => {
            let ord = match (&a, &b) {
                (Const::Int(x), Const::Int(y)) => x.partial_cmp(y)?,
                (Const::Float(x), Const::Float(y)) => x.partial_cmp(y)?,
                _ => return None,
            };
            use std::cmp::Ordering::*;
            Const::Bool(matches!(
                (op, ord),
                (BinOp::Lt, Less)
                    | (BinOp::Le, Less | Equal)
                    | (BinOp::Gt, Greater)
                    | (BinOp::Ge, Greater | Equal)
            ))
        }
        _ => return None,
    })
}

fn fold_un(op: crate::ir::UnOp, ty: Ty, a: Option<Const>) -> Option<Const> {
    let a = a?;
    use crate::ir::UnOp::*;
    Some(match (op, ty) {
        (Not, Ty::Bool) => {
            let Const::Bool(b) = a else { return None };
            Const::Bool(!b)
        }
        (Neg, Ty::Int) => {
            let Const::Int(i) = a else { return None };
            Const::Int(i.checked_neg()?)
        }
        (Neg, Ty::Float) => {
            let Const::Float(x) = a else { return None };
            Const::Float(-x)
        }
        _ => return None,
    })
}

/// Remove blocks unreachable from block 0.
fn remove_unreachable_blocks(f: &mut IrFunction) {
    let mut reachable = vec![false; f.blocks.len()];
    let mut work = vec![0usize];
    while let Some(b) = work.pop() {
        if reachable[b] {
            continue;
        }
        reachable[b] = true;
        match &f.blocks[b].term {
            Some(IrTerm::Jmp(t)) => work.push(*t),
            Some(IrTerm::Br { then, els, cond: _ }) => {
                work.push(*then);
                work.push(*els);
            }
            _ => {}
        }
    }
    if reachable.iter().all(|r| *r) {
        return;
    }
    // map old block id -> new id
    let mut map = vec![usize::MAX; f.blocks.len()];
    let mut next = 0usize;
    for (i, r) in reachable.iter().enumerate() {
        if *r {
            map[i] = next;
            next += 1;
        }
    }
    let mut new_blocks = Vec::with_capacity(next);
    for (i, r) in reachable.iter().enumerate() {
        if !*r {
            continue;
        }
        let mut b = f.blocks[i].clone();
        b.term = b.term.take().map(|t| match t {
            IrTerm::Jmp(t) => IrTerm::Jmp(map[t]),
            IrTerm::Br { cond, then, els } => IrTerm::Br {
                cond,
                then: map[then],
                els: map[els],
            },
            other => other,
        });
        new_blocks.push(b);
    }
    f.blocks = new_blocks;
}

/// Remove instructions writing temporaries that are never read.
/// Calls and prints are never removed (side effects).
fn eliminate_dead_temps(f: &mut IrFunction) {
    let first_temp = f.nlocals as usize;
    loop {
        let mut used = vec![false; f.nregs as usize];
        for block in &f.blocks {
            for instr in &block.instrs {
                for r in reads(instr) {
                    used[r as usize] = true;
                }
            }
            match &block.term {
                Some(IrTerm::Jmp(_)) => {}
                Some(IrTerm::Br { cond, .. }) => used[*cond as usize] = true,
                Some(IrTerm::Ret(r)) => used[*r as usize] = true,
                Some(IrTerm::RetUnit) => {}
                None => {}
            }
        }
        let mut changed = false;
        for block in &mut f.blocks {
            let mut keep = Vec::with_capacity(block.instrs.len());
            for instr in block.instrs.drain(..) {
                let removable = match &instr {
                    IrInstr::Const { dst, .. }
                    | IrInstr::Move { dst, .. }
                    | IrInstr::Bin { dst, .. }
                    | IrInstr::Un { dst, .. }
                    | IrInstr::IntToFloat { dst, .. } => {
                        (*dst as usize) >= first_temp && !used[*dst as usize]
                    }
                    _ => false,
                };
                if removable {
                    changed = true;
                } else {
                    keep.push(instr);
                }
            }
            block.instrs = keep;
        }
        if !changed {
            break;
        }
    }
}

fn reads(instr: &IrInstr) -> Vec<u16> {
    match instr {
        IrInstr::Const { .. } => vec![],
        IrInstr::Move { src, .. } => vec![*src],
        IrInstr::Bin { a, b, .. } => vec![*a, *b],
        IrInstr::Un { a, .. } => vec![*a],
        IrInstr::IntToFloat { a, .. } => vec![*a],
        IrInstr::Call { args, .. } => args.clone(),
        IrInstr::Builtin { args, .. } => args.clone(),
        IrInstr::GuiButton { args, .. } => args.clone(),
        IrInstr::Print { src, .. } => vec![*src],
    }
}

/// Block-local CSE: within one block, an arithmetic/comparison instruction
/// with operands identical to an earlier instruction of the same kind reuses
/// the earlier result. Only pure ops participate (no calls, no prints).
type CseKey = (u8, u8, u8, u16, u16);

fn block_local_cse(f: &mut IrFunction) {
    for block in &mut f.blocks {
        let mut table: Vec<(CseKey, u16)> = Vec::new();
        let mut out = Vec::with_capacity(block.instrs.len());
        for instr in block.instrs.drain(..) {
            let key: Option<(u8, u8, u8, u16, u16)> = match &instr {
                IrInstr::Bin { op, ty, a, b, .. } => Some((1u8, *op as u8, *ty as u8, *a, *b)),
                IrInstr::Un { op, ty, a, .. } => Some((2u8, *op as u8, *ty as u8, *a, u16::MAX)),
                IrInstr::IntToFloat { a, .. } => Some((3u8, 0, 0, *a, u16::MAX)),
                _ => None,
            };
            if let Some(key) = key {
                if let Some((_, existing)) = table.iter().find(|(k, _)| *k == key) {
                    let existing = *existing;
                    let dst = match &instr {
                        IrInstr::Bin { dst, .. }
                        | IrInstr::Un { dst, .. }
                        | IrInstr::IntToFloat { dst, .. } => *dst,
                        _ => unreachable!(),
                    };
                    if dst != existing {
                        out.push(IrInstr::Move { src: existing, dst });
                        continue;
                    }
                } else {
                    let dst = match &instr {
                        IrInstr::Bin { dst, .. }
                        | IrInstr::Un { dst, .. }
                        | IrInstr::IntToFloat { dst, .. } => *dst,
                        _ => unreachable!(),
                    };
                    table.push((key, dst));
                }
            }
            out.push(instr);
        }
        block.instrs = out;
    }
}
