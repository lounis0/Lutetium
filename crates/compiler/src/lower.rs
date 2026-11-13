//! Lowering: typed AST -> typed IR.
//!
//! One IR function per checked function. Locals keep their checker-assigned
//! register slots; temporaries are allocated above `nlocals` and reused
//! per-statement. `&&`/`||` are lowered through branch blocks that both write
//! the same destination register, which the bytecode verifier accepts.

use crate::ir::{BinOp, Const, IrBlock, IrFunction, IrInstr, IrModule, IrTerm, Ty};
use crate::typecheck::{
    expr_is_value, texpr_ty, LogicOp, TExpr, TFun, TModule, TStmt, PRINT_BUILTIN,
};
use lu_syntax::ast::AssignOp;
use lu_vm::bytecode::{BcTy, BuiltinKind};

/// Convert a bytecode-level type to the IR type representation.
fn ir_ty(t: BcTy) -> Ty {
    match t {
        BcTy::Int => Ty::Int,
        BcTy::Float => Ty::Float,
        BcTy::Bool => Ty::Bool,
        BcTy::Str => Ty::Str,
        BcTy::Unit => Ty::Unit,
        BcTy::List => Ty::List,
        BcTy::Map => Ty::Map,
        BcTy::Window => Ty::Window,
        BcTy::Widget => Ty::Widget,
    }
}

pub fn lower(tm: &TModule) -> IrModule {
    let funcs = tm.funcs.iter().map(lower_fn).collect();
    IrModule {
        funcs,
        entry: tm.entry,
    }
}

fn lower_fn(tf: &TFun) -> IrFunction {
    let mut lo = FnLower {
        name: tf.name.clone(),
        params: tf.params.iter().map(|t| ir_ty(*t)).collect(),
        ret: ir_ty(tf.ret),
        nlocals: tf.nlocals,
        blocks: Vec::new(),
        cur: 0,
        next_temp: tf.nlocals,
        max_reg: tf.nlocals.saturating_sub(1),
        reg_tys: {
            let mut t = vec![Ty::Unit; tf.nlocals.max(1) as usize];
            for (i, p) in tf.params.iter().enumerate() {
                t[i] = ir_ty(*p);
            }
            t
        },
        breaks: Vec::new(),
        conts: Vec::new(),
    };
    lo.blocks.push(IrBlock::default());
    lo.lower_stmts(&tf.body);
    if !lo.terminated() {
        lo.term(IrTerm::RetUnit);
    }
    IrFunction {
        name: lo.name,
        params: lo.params,
        ret: lo.ret,
        nlocals: lo.nlocals,
        nregs: lo.max_reg + 1,
        reg_tys: {
            lo.reg_tys.resize(lo.max_reg as usize + 1, Ty::Unit);
            lo.reg_tys
        },
        blocks: lo.blocks,
    }
}

struct FnLower {
    name: String,
    #[allow(dead_code)]
    params: Vec<Ty>,
    #[allow(dead_code)]
    ret: Ty,
    #[allow(dead_code)]
    nlocals: u16,
    blocks: Vec<IrBlock>,
    cur: usize,
    next_temp: u16,
    max_reg: u16,
    /// Type of every register written so far (params, locals, temps).
    reg_tys: Vec<Ty>,
    breaks: Vec<usize>,
    conts: Vec<usize>,
}

impl FnLower {
    fn new_block(&mut self) -> usize {
        self.blocks.push(IrBlock::default());
        self.blocks.len() - 1
    }

    fn emit(&mut self, instr: IrInstr) {
        self.blocks[self.cur].instrs.push(instr);
    }

    fn term(&mut self, t: IrTerm) {
        let b = &mut self.blocks[self.cur];
        if b.term.is_none() {
            b.term = Some(t);
        }
    }

    fn terminated(&self) -> bool {
        self.blocks[self.cur].term.is_some()
    }

    fn temp(&mut self) -> u16 {
        let t = self.next_temp;
        self.next_temp += 1;
        if t > self.max_reg {
            self.max_reg = t;
        }
        t
    }

    fn note(&mut self, r: u16) {
        if r > self.max_reg {
            self.max_reg = r;
        }
    }

    /// Record the type of a register and grow bookkeeping as needed.
    fn def_ty(&mut self, r: u16, t: Ty) {
        self.note(r);
        if self.reg_tys.len() <= r as usize {
            self.reg_tys.resize(r as usize + 1, Ty::Unit);
        }
        self.reg_tys[r as usize] = t;
    }

    fn lower_stmts(&mut self, stmts: &[TStmt]) {
        for s in stmts {
            self.lower_stmt(s);
        }
    }

    /// Lower statements, capturing the value of a trailing value expression
    /// into `dst` (used for if-expression branch values).
    fn lower_branch(&mut self, stmts: &[TStmt], dst: u16) {
        if let Some((TStmt::Expr(e), rest)) = stmts.split_last() {
            if expr_is_value(e) {
                self.lower_stmts(rest);
                let v = self.lower_expr(e);
                self.emit(IrInstr::Move { src: v, dst });
                self.note(dst);
                return;
            }
        }
        self.lower_stmts(stmts);
    }

    fn lower_stmt(&mut self, s: &TStmt) {
        match s {
            TStmt::Let { init, slot, cast } => {
                let v = self.lower_expr(init);
                let v = self.maybe_cast(v, *cast);
                self.emit(IrInstr::Move { src: v, dst: *slot });
                let slot_ty = if *cast {
                    Ty::Float
                } else {
                    ir_ty(texpr_ty(init).to_bc())
                };
                self.def_ty(*slot, slot_ty);
            }
            TStmt::Assign {
                op,
                slot,
                cast,
                value,
                ty,
            } => {
                let v = self.lower_expr(value);
                let v = self.maybe_cast(v, *cast);
                match op {
                    AssignOp::Set => {
                        self.emit(IrInstr::Move { src: v, dst: *slot });
                    }
                    aop => {
                        let binop = match aop {
                            AssignOp::Add => BinOp::Add,
                            AssignOp::Sub => BinOp::Sub,
                            AssignOp::Mul => BinOp::Mul,
                            AssignOp::Div => BinOp::Div,
                            AssignOp::Set => unreachable!(),
                        };
                        let t = self.temp();
                        self.emit(IrInstr::Bin {
                            op: binop,
                            ty: ir_ty(*ty),
                            a: *slot,
                            b: v,
                            dst: t,
                        });
                        self.def_ty(t, ir_ty(*ty));
                        self.emit(IrInstr::Move { src: t, dst: *slot });
                    }
                }
                self.def_ty(*slot, ir_ty(*ty));
            }
            TStmt::Expr(e) => {
                self.lower_expr(e);
            }
            TStmt::If { cond, then, els } => {
                let c = self.lower_expr(cond);
                let then_b = self.new_block();
                let merge = self.new_block();
                let else_b = els.as_ref().map(|_| self.new_block());
                self.term(IrTerm::Br {
                    cond: c,
                    then: then_b,
                    els: else_b.unwrap_or(merge),
                });
                self.switch_to(then_b);
                self.lower_stmts(then);
                if !self.terminated() {
                    self.term(IrTerm::Jmp(merge));
                }
                if let (Some(eb), Some(estmts)) = (else_b, els) {
                    self.switch_to(eb);
                    self.lower_stmts(estmts);
                    if !self.terminated() {
                        self.term(IrTerm::Jmp(merge));
                    }
                }
                self.switch_to(merge);
            }
            TStmt::While { cond, body } => {
                // The condition lives in its own block so the back edge does
                // not re-execute statements preceding the loop.
                let cond_b = self.new_block();
                self.term(IrTerm::Jmp(cond_b));
                self.switch_to(cond_b);
                let c = self.lower_expr(cond);
                let body_b = self.new_block();
                let after = self.new_block();
                self.term(IrTerm::Br {
                    cond: c,
                    then: body_b,
                    els: after,
                });
                self.breaks.push(after);
                self.conts.push(cond_b);
                self.switch_to(body_b);
                self.lower_stmts(body);
                if !self.terminated() {
                    self.term(IrTerm::Jmp(cond_b));
                }
                self.breaks.pop();
                self.conts.pop();
                self.switch_to(after);
            }
            TStmt::For {
                slot,
                start,
                end,
                inclusive,
                body,
            } => {
                // init: slot = start; jump to the condition
                let sv = self.lower_expr(start);
                self.emit(IrInstr::Move {
                    src: sv,
                    dst: *slot,
                });
                self.def_ty(*slot, Ty::Int);
                let cond_b = self.new_block();
                self.term(IrTerm::Jmp(cond_b));
                self.switch_to(cond_b);
                // cond: slot < end (or <= end for inclusive ranges)
                let ev = self.lower_expr(end);
                let c = self.temp();
                self.def_ty(c, Ty::Bool);
                self.emit(IrInstr::Bin {
                    op: if *inclusive { BinOp::Le } else { BinOp::Lt },
                    ty: Ty::Int,
                    a: *slot,
                    b: ev,
                    dst: c,
                });
                let body_b = self.new_block();
                let after = self.new_block();
                let cont_b = self.new_block();
                let inc_b = self.new_block();
                self.term(IrTerm::Br {
                    cond: c,
                    then: body_b,
                    els: after,
                });
                self.breaks.push(after);
                self.conts.push(cont_b);
                self.switch_to(body_b);
                self.lower_stmts(body);
                if !self.terminated() {
                    self.term(IrTerm::Jmp(cont_b));
                }
                self.breaks.pop();
                self.conts.pop();
                // continue: increment, then re-test. Inclusive ranges stop by
                // equality so the increment never overflows past the bound.
                self.switch_to(cont_b);
                if *inclusive {
                    let eq = self.temp();
                    self.def_ty(eq, Ty::Bool);
                    self.emit(IrInstr::Bin {
                        op: BinOp::Eq,
                        ty: Ty::Int,
                        a: *slot,
                        b: ev,
                        dst: eq,
                    });
                    self.term(IrTerm::Br {
                        cond: eq,
                        then: after,
                        els: inc_b,
                    });
                } else {
                    self.term(IrTerm::Jmp(inc_b));
                }
                self.switch_to(inc_b);
                let one = self.temp();
                self.def_ty(one, Ty::Int);
                self.emit(IrInstr::Const {
                    v: Const::Int(1),
                    dst: one,
                });
                let next = self.temp();
                self.def_ty(next, Ty::Int);
                self.emit(IrInstr::Bin {
                    op: BinOp::Add,
                    ty: Ty::Int,
                    a: *slot,
                    b: one,
                    dst: next,
                });
                self.emit(IrInstr::Move {
                    src: next,
                    dst: *slot,
                });
                self.term(IrTerm::Jmp(cond_b));
                self.switch_to(after);
            }
            TStmt::ForIn {
                slot,
                iterable,
                elem,
                body,
            } => {
                // The iterable expression is evaluated exactly once; its list
                // value is captured in a temp root (which also keeps the list
                // alive for the GC for the whole loop).
                let l = self.lower_expr(iterable);
                // Hidden index register, defined once before the loop.
                let idx = self.temp();
                self.def_ty(idx, Ty::Int);
                self.emit(IrInstr::Const {
                    v: Const::Int(0),
                    dst: idx,
                });
                let cond_b = self.new_block();
                self.term(IrTerm::Jmp(cond_b));
                self.switch_to(cond_b);
                // The length is recomputed every iteration: pushing inside
                // the body extends the iteration, popping shortens it, and
                // `idx < len` keeps `list_get` in bounds.
                let n = self.temp();
                self.def_ty(n, Ty::Int);
                self.emit(IrInstr::Builtin {
                    kind: BuiltinKind::ListLen,
                    args: vec![l],
                    dst: n,
                });
                let c = self.temp();
                self.def_ty(c, Ty::Bool);
                self.emit(IrInstr::Bin {
                    op: BinOp::Lt,
                    ty: Ty::Int,
                    a: idx,
                    b: n,
                    dst: c,
                });
                let body_b = self.new_block();
                let after = self.new_block();
                let cont_b = self.new_block();
                self.term(IrTerm::Br {
                    cond: c,
                    then: body_b,
                    els: after,
                });
                self.breaks.push(after);
                self.conts.push(cont_b);
                self.switch_to(body_b);
                // slot = list_get(l, idx) — always in bounds (idx < len here).
                self.def_ty(*slot, ir_ty(*elem));
                self.emit(IrInstr::Builtin {
                    kind: BuiltinKind::ListGet,
                    args: vec![l, idx],
                    dst: *slot,
                });
                self.lower_stmts(body);
                if !self.terminated() {
                    self.term(IrTerm::Jmp(cont_b));
                }
                self.breaks.pop();
                self.conts.pop();
                // continue: increment the index, then re-test the length.
                self.switch_to(cont_b);
                let one = self.temp();
                self.def_ty(one, Ty::Int);
                self.emit(IrInstr::Const {
                    v: Const::Int(1),
                    dst: one,
                });
                let next = self.temp();
                self.def_ty(next, Ty::Int);
                self.emit(IrInstr::Bin {
                    op: BinOp::Add,
                    ty: Ty::Int,
                    a: idx,
                    b: one,
                    dst: next,
                });
                self.emit(IrInstr::Move {
                    src: next,
                    dst: idx,
                });
                self.term(IrTerm::Jmp(cond_b));
                self.switch_to(after);
            }
            TStmt::Return { value } => match value {
                Some(e) => {
                    let v = self.lower_expr(e);
                    self.term(IrTerm::Ret(v));
                }
                None => self.term(IrTerm::RetUnit),
            },
            TStmt::Break => {
                let target = *self.breaks.last().expect("break target");
                self.term(IrTerm::Jmp(target));
                // subsequent statements (if any) go into an unreachable block
                let fresh = self.new_block();
                self.switch_to(fresh);
            }
            TStmt::Continue => {
                let target = *self.conts.last().expect("continue target");
                self.term(IrTerm::Jmp(target));
                let fresh = self.new_block();
                self.switch_to(fresh);
            }
        }
    }

    fn switch_to(&mut self, b: usize) {
        self.cur = b;
    }

    fn maybe_cast(&mut self, v: u16, cast: bool) -> u16 {
        if cast {
            let t = self.temp();
            self.def_ty(t, Ty::Float);
            self.emit(IrInstr::IntToFloat { a: v, dst: t });
            t
        } else {
            v
        }
    }

    fn lower_expr(&mut self, e: &TExpr) -> u16 {
        match e {
            TExpr::Int(v) => {
                let d = self.temp();
                self.def_ty(d, Ty::Int);
                self.emit(IrInstr::Const {
                    v: Const::Int(*v),
                    dst: d,
                });
                d
            }
            TExpr::Float(v) => {
                let d = self.temp();
                self.def_ty(d, Ty::Float);
                self.emit(IrInstr::Const {
                    v: Const::Float(*v),
                    dst: d,
                });
                d
            }
            TExpr::Bool(v) => {
                let d = self.temp();
                self.def_ty(d, Ty::Bool);
                self.emit(IrInstr::Const {
                    v: Const::Bool(*v),
                    dst: d,
                });
                d
            }
            TExpr::Str(v) => {
                let d = self.temp();
                self.def_ty(d, Ty::Str);
                self.emit(IrInstr::Const {
                    v: Const::Str(v.clone()),
                    dst: d,
                });
                d
            }
            TExpr::Local { slot, .. } => {
                self.note(*slot);
                *slot
            }
            TExpr::Logic { op, lhs, rhs } => {
                let r = self.temp();
                self.def_ty(r, Ty::Bool);
                let la = self.lower_expr(lhs);
                let rhs_b = self.new_block();
                let short_b = self.new_block();
                let merge = self.new_block();
                match op {
                    LogicOp::And => self.term(IrTerm::Br {
                        cond: la,
                        then: rhs_b,
                        els: short_b,
                    }),
                    LogicOp::Or => self.term(IrTerm::Br {
                        cond: la,
                        then: short_b,
                        els: rhs_b,
                    }),
                }
                self.switch_to(rhs_b);
                let lb = self.lower_expr(rhs);
                self.emit(IrInstr::Move { src: lb, dst: r });
                self.term(IrTerm::Jmp(merge));
                self.switch_to(short_b);
                self.emit(IrInstr::Const {
                    v: Const::Bool(matches!(op, LogicOp::Or)),
                    dst: r,
                });
                self.term(IrTerm::Jmp(merge));
                self.switch_to(merge);
                r
            }
            TExpr::Binary {
                op,
                operand_ty,
                lcast,
                rcast,
                lhs,
                rhs,
                ..
            } => {
                let mut a = self.lower_expr(lhs);
                a = self.maybe_cast(a, *lcast);
                let mut b = self.lower_expr(rhs);
                b = self.maybe_cast(b, *rcast);
                let d = self.temp();
                let is_cmp = matches!(
                    op,
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
                );
                self.def_ty(d, if is_cmp { Ty::Bool } else { ir_ty(*operand_ty) });
                self.emit(IrInstr::Bin {
                    op: *op,
                    ty: ir_ty(*operand_ty),
                    a,
                    b,
                    dst: d,
                });
                d
            }
            TExpr::Unary { op, ty, expr } => {
                let a = self.lower_expr(expr);
                let d = self.temp();
                self.def_ty(d, ir_ty(*ty));
                self.emit(IrInstr::Un {
                    op: *op,
                    ty: ir_ty(*ty),
                    a,
                    dst: d,
                });
                d
            }
            TExpr::If {
                cond,
                then,
                els,
                ty,
            } => {
                let r = self.temp();
                self.def_ty(r, ir_ty(ty.to_bc()));
                let c = self.lower_expr(cond);
                let then_b = self.new_block();
                let else_b = self.new_block();
                let merge = self.new_block();
                self.term(IrTerm::Br {
                    cond: c,
                    then: then_b,
                    els: else_b,
                });
                self.switch_to(then_b);
                self.lower_branch(then, r);
                if !self.terminated() {
                    self.term(IrTerm::Jmp(merge));
                }
                self.switch_to(else_b);
                match els {
                    Some(eb) => {
                        self.lower_branch(eb, r);
                        if !self.terminated() {
                            self.term(IrTerm::Jmp(merge));
                        }
                    }
                    None => {
                        // checker rejects value-position ifs without else;
                        // keep the IR well-formed regardless
                        self.emit(IrInstr::Const {
                            v: Const::Unit,
                            dst: r,
                        });
                        self.term(IrTerm::Jmp(merge));
                    }
                }
                self.switch_to(merge);
                r
            }
            TExpr::Call {
                func, casts, args, ..
            } => {
                let mut regs = Vec::with_capacity(args.len());
                for (i, a) in args.iter().enumerate() {
                    let v = self.lower_expr(a);
                    let cast = casts.get(i).copied().unwrap_or(false);
                    regs.push(self.maybe_cast(v, cast));
                }
                if *func == PRINT_BUILTIN {
                    let arg_ty = ir_ty(texpr_ty(&args[0]).to_bc());
                    self.emit(IrInstr::Print {
                        src: regs[0],
                        ty: arg_ty,
                    });
                    let d = self.temp();
                    self.emit(IrInstr::Const {
                        v: Const::Unit,
                        dst: d,
                    });
                    self.def_ty(d, Ty::Unit);
                    d
                } else {
                    let d = self.temp();
                    self.emit(IrInstr::Call {
                        func: *func,
                        args: regs,
                        dst: d,
                    });
                    self.def_ty(d, ir_ty(texpr_ty(e).to_bc()));
                    d
                }
            }
            TExpr::List { elems, ty } => {
                let mut regs = Vec::with_capacity(elems.len());
                for e in elems {
                    let v = self.lower_expr(e);
                    // A mixed [int, float] list was widened to float by the
                    // checker; materialize the widening for int elements.
                    let v = if *ty == BcTy::Float
                        && matches!(texpr_ty(e), crate::typecheck::CTy::K(BcTy::Int))
                    {
                        let t = self.temp();
                        self.def_ty(t, Ty::Float);
                        self.emit(IrInstr::IntToFloat { a: v, dst: t });
                        t
                    } else {
                        v
                    };
                    regs.push(v);
                }
                let d = self.temp();
                self.def_ty(d, Ty::List);
                self.emit(IrInstr::Builtin {
                    kind: BuiltinKind::ListNew,
                    args: regs,
                    dst: d,
                });
                d
            }
            TExpr::Map { entries } => {
                // Lower each entry to (key, value) register pairs; keys are
                // string constants. The map builtin kind interleaves them.
                let mut regs = Vec::with_capacity(entries.len() * 2);
                for (key, v) in entries {
                    let k = self.temp();
                    self.def_ty(k, Ty::Str);
                    self.emit(IrInstr::Const {
                        v: Const::Str(key.clone()),
                        dst: k,
                    });
                    let vr = self.lower_expr(v);
                    regs.push(k);
                    regs.push(vr);
                }
                let d = self.temp();
                self.def_ty(d, Ty::Map);
                self.emit(IrInstr::Builtin {
                    kind: BuiltinKind::MapNew,
                    args: regs,
                    dst: d,
                });
                d
            }
            TExpr::Field { recv, key, ty } => {
                let b = self.lower_expr(recv);
                let k = self.temp();
                self.def_ty(k, Ty::Str);
                self.emit(IrInstr::Const {
                    v: Const::Str(key.clone()),
                    dst: k,
                });
                let d = self.temp();
                self.def_ty(d, ir_ty(ty.to_bc()));
                self.emit(IrInstr::Builtin {
                    kind: BuiltinKind::MapGet,
                    args: vec![b, k],
                    dst: d,
                });
                d
            }
            TExpr::Index { base, idx, ty } => {
                let b = self.lower_expr(base);
                let i = self.lower_expr(idx);
                let d = self.temp();
                self.def_ty(d, ir_ty(ty.to_bc()));
                self.emit(IrInstr::Builtin {
                    kind: BuiltinKind::ListGet,
                    args: vec![b, i],
                    dst: d,
                });
                d
            }
            TExpr::Method {
                recv,
                kind,
                casts,
                args,
                ty,
            } => {
                let recv_reg = self.lower_expr(recv);
                let mut regs = Vec::with_capacity(args.len() + 1);
                regs.push(recv_reg);
                for (i, a) in args.iter().enumerate() {
                    let v = self.lower_expr(a);
                    regs.push(self.maybe_cast(v, casts.get(i).copied().unwrap_or(false)));
                }
                let d = self.temp();
                self.def_ty(d, ir_ty(ty.to_bc()));
                self.emit(IrInstr::Builtin {
                    kind: *kind,
                    args: regs,
                    dst: d,
                });
                d
            }
            TExpr::GuiCallback { kind, args, callback } => {
                let regs: Vec<u16> = args.iter().map(|arg| self.lower_expr(arg)).collect();
                let cb = self.temp();
                self.def_ty(cb, Ty::Int);
                self.emit(IrInstr::Const { v: Const::Int(*callback as i64), dst: cb });
                let mut args = regs;
                args.push(cb);
                let dst = self.temp();
                self.def_ty(dst, Ty::Unit);
                self.emit(IrInstr::Builtin { kind: *kind, args, dst });
                dst
            }
            TExpr::GuiButton { args, handler } => {
                let mut regs = Vec::with_capacity(args.len() + 1);
                for arg in args {
                    regs.push(self.lower_expr(arg));
                }
                let handler_ref = self.temp();
                self.def_ty(handler_ref, Ty::Int);
                self.emit(IrInstr::Const { v: Const::Int(*handler as i64), dst: handler_ref });
                regs.push(handler_ref);
                let dst = self.temp();
                self.def_ty(dst, Ty::Widget);
                regs.pop();
                self.emit(IrInstr::GuiButton { args: regs, handler: *handler, dst });
                dst
            }
            TExpr::Builtin {
                kind,
                casts,
                args,
                ty,
            } => {
                let mut regs = Vec::with_capacity(args.len());
                for (i, a) in args.iter().enumerate() {
                    let v = self.lower_expr(a);
                    regs.push(self.maybe_cast(v, casts.get(i).copied().unwrap_or(false)));
                }
                let d = self.temp();
                self.def_ty(d, ir_ty(ty.to_bc()));
                self.emit(IrInstr::Builtin {
                    kind: *kind,
                    args: regs,
                    dst: d,
                });
                d
            }
        }
    }
}
