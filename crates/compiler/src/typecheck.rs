//! Type checker for the Lutetium 0.1 subset.
//!
//! Responsibilities:
//! * bind variables to typed, block-scoped slots (params are `r0..r{arity-1}`),
//! * infer `let` types from initializers, check explicit annotations,
//! * infer unannotated return types via a fixpoint across the function table
//!   (annotated return types are used as-is),
//! * insert the single legal implicit conversion (`int` -> `float`, never the
//!   reverse) and mark it so the lowering pass can materialize it,
//! * reject everything else with stable diagnostics.

use crate::ir::{BinOp, UnOp};
use crate::resolve::FnInfo;
use lu_diagnostics::{Diagnostic, Span};
use lu_syntax::ast::{
    AssignOp, BinOp as ABinOp, Block, Expr, FnDecl, Module, Stmt, TypeKind, UnOp as AUnOp,
};
use lu_vm::bytecode::{BcTy, BuiltinKind};
use std::collections::HashMap;

/// Sentinel function index for the `print` builtin.
pub const PRINT_BUILTIN: usize = usize::MAX;

/// Observation of an inferred parameter type across a checking pass.
#[derive(Debug, Clone, PartialEq)]
enum Obs {
    Unobserved,
    Observed(CTy),
    Conflicted,
}

/// Fixpoint state of one function: its return type and parameter types.
#[derive(Debug, Clone, PartialEq)]
struct FnState {
    ret: CTy,
    params: Vec<CTy>,
}

/// Value produced by a block: the type of a trailing non-`unit` expression.
fn block_value(stmts: &[TStmt]) -> Option<CTy> {
    match stmts.last()? {
        TStmt::Expr(e) => {
            let t = texpr_ty(e);
            if t == CTy::K(BcTy::Unit) {
                None
            } else {
                Some(t)
            }
        }
        _ => None,
    }
}

/// Whether an expression produces a usable value (for implicit tail returns
/// and if-expression branch values).
pub fn expr_is_value(e: &TExpr) -> bool {
    match e {
        TExpr::Call { ty, .. } | TExpr::Builtin { ty, .. } | TExpr::Method { ty, .. } => {
            !matches!(ty, CTy::K(BcTy::Unit) | CTy::Unknown)
        }
        _ => true,
    }
}

/// Checker-time type: a concrete bytecode type, a list of scalars (`L`), or
/// `Unknown` (transient, used while return-type inference has not converged,
/// or after an error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CTy {
    K(BcTy),
    L(BcTy),
    /// A record-style map: a fixed set of string keys with per-key value
    /// types, kept sorted by key name. Produced by map literals; `.field`
    /// access, `get` and `set` are checked against the fields at compile
    /// time.
    R(Vec<(String, BcTy)>),
    Unknown,
}

impl CTy {
    pub fn name(&self) -> &'static str {
        match self {
            CTy::K(t) => t.name(),
            CTy::L(_) => "list",
            CTy::R(_) => "map",
            CTy::Unknown => "unknown",
        }
    }

    pub fn to_bc(&self) -> BcTy {
        match self {
            CTy::K(t) => *t,
            // Element types are tracked by the checker; the bytecode-level
            // type of a list value is the `List` tag.
            CTy::L(_) => BcTy::List,
            // Field types are tracked by the checker; the bytecode-level
            // type of a map value is the `Map` tag.
            CTy::R(_) => BcTy::Map,
            // Only reachable in failing compilations; the placeholder value is
            // never code-generated.
            CTy::Unknown => BcTy::Unit,
        }
    }
}

fn cty_of(t: BcTy) -> CTy {
    CTy::K(t)
}

/// Type of a typed expression.
pub fn texpr_ty(t: &TExpr) -> CTy {
    match t {
        TExpr::Int(..) => cty_of(BcTy::Int),
        TExpr::Float(..) => cty_of(BcTy::Float),
        TExpr::Bool(..) => cty_of(BcTy::Bool),
        TExpr::Str(..) => cty_of(BcTy::Str),
        TExpr::Unary { ty, .. } => cty_of(*ty),
        TExpr::Local { ty, .. } => ty.clone(),
        TExpr::Binary { ty, .. }
        | TExpr::Call { ty, .. }
        | TExpr::Builtin { ty, .. }
        | TExpr::Method { ty, .. }
        | TExpr::Index { ty, .. }
        | TExpr::If { ty, .. } => ty.clone(),
        TExpr::GuiButton { .. } => cty_of(BcTy::Widget),
        TExpr::GuiCallback { .. } => cty_of(BcTy::Unit),
        TExpr::List { ty, .. } => CTy::L(*ty),
        TExpr::Map { entries, .. } => CTy::R(sorted_fields(entries)),
        TExpr::Field { ty, .. } => ty.clone(),
        TExpr::Logic { .. } => cty_of(BcTy::Bool),
    }
}

/// Per-key value types of a map literal, sorted and deduplicated (a later
/// duplicate key wins, matching the runtime `map_new` semantics).
fn sorted_fields(entries: &[(String, TExpr)]) -> Vec<(String, BcTy)> {
    let mut fields: Vec<(String, BcTy)> = entries
        .iter()
        .map(|(k, v)| (k.clone(), texpr_ty(v).to_bc()))
        .collect();
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    fields.dedup_by(|a, b| {
        if a.0 == b.0 {
            b.1 = a.1; // later value type wins
            true
        } else {
            false
        }
    });
    fields
}

/// `Some(cast)` when a value of type `from` may be used where `to` is
/// expected; `cast` marks the implicit `int` -> `float` widening.
fn castable(from: CTy, to: CTy) -> Option<bool> {
    match (from, to) {
        (CTy::Unknown, _) | (_, CTy::Unknown) => Some(false),
        (CTy::L(a), CTy::L(b)) => {
            if a == b {
                Some(false)
            } else if a == BcTy::Int && b == BcTy::Float {
                Some(true)
            } else {
                None
            }
        }
        (CTy::L(_), _) | (_, CTy::L(_)) => None,
        // Record maps are compatible only when their fields match exactly
        // (same keys, same value types).
        (CTy::R(a), CTy::R(b)) => {
            if a == b {
                Some(false)
            } else {
                None
            }
        }
        (CTy::R(_), _) | (_, CTy::R(_)) => None,
        (CTy::K(a), CTy::K(b)) => {
            if a == b {
                Some(false)
            } else if a == BcTy::Int && b == BcTy::Float {
                Some(true)
            } else {
                None
            }
        }
    }
}

/// Unify two inferred types. `Unknown` absorbs into the other side; int+float
/// widens to float.
fn unify(a: Option<CTy>, b: CTy) -> Result<CTy, ()> {
    match (a, b) {
        (None, other) => Ok(other),
        (Some(prev), CTy::Unknown) => Ok(prev),
        (Some(CTy::Unknown), other) => Ok(other),
        (Some(CTy::K(x)), CTy::K(y)) => {
            if x == y {
                Ok(cty_of(x))
            } else if (x == BcTy::Int && y == BcTy::Float) || (x == BcTy::Float && y == BcTy::Int) {
                Ok(cty_of(BcTy::Float))
            } else {
                Err(())
            }
        }
        (Some(CTy::L(a)), CTy::L(b)) => {
            if a == b {
                Ok(CTy::L(a))
            } else if a == BcTy::Int && b == BcTy::Float {
                Ok(CTy::L(BcTy::Float))
            } else if a == BcTy::Float && b == BcTy::Int {
                Ok(CTy::L(BcTy::Float))
            } else {
                Err(())
            }
        }
        // Record maps unify only when identical (same keys, same types).
        (Some(CTy::R(a)), CTy::R(b)) => {
            if a == b {
                Ok(CTy::R(a))
            } else {
                Err(())
            }
        }
        // lists/maps and scalars never unify; lists and maps never unify
        (Some(CTy::L(_)), _)
        | (Some(_), CTy::L(_))
        | (Some(CTy::R(_)), _)
        | (Some(_), CTy::R(_)) => Err(()),
    }
}

fn is_numeric(t: &CTy) -> bool {
    matches!(t, CTy::K(BcTy::Int) | CTy::K(BcTy::Float) | CTy::Unknown)
}

// ---------------------------------------------------------------------------
// Typed AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum TExpr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Local {
        slot: u16,
        ty: CTy,
    },
    Binary {
        op: BinOp,
        /// result type of the expression (bool for comparisons)
        ty: CTy,
        /// operand type (int/float/str) used to select the opcode
        operand_ty: BcTy,
        lcast: bool,
        rcast: bool,
        lhs: Box<TExpr>,
        rhs: Box<TExpr>,
    },
    Logic {
        op: LogicOp,
        lhs: Box<TExpr>,
        rhs: Box<TExpr>,
    },
    Unary {
        op: UnOp,
        ty: BcTy,
        expr: Box<TExpr>,
    },
    Call {
        func: usize,
        casts: Vec<bool>,
        args: Vec<TExpr>,
        ty: CTy,
    },
    If {
        cond: Box<TExpr>,
        then: Vec<TStmt>,
        els: Option<Vec<TStmt>>,
        ty: CTy,
    },
    /// List literal; `ty` is the element type (scalars only in 0.4).
    List {
        elems: Vec<TExpr>,
        ty: BcTy,
    },
    /// Map literal `{name: "x", score: 42}`; keys are string keys with
    /// per-key value types (record semantics).
    Map {
        entries: Vec<(String, TExpr)>,
    },
    /// Field access `recv.name` on a map; `ty` is the value type.
    Field {
        recv: Box<TExpr>,
        key: String,
        ty: CTy,
    },
    /// Indexing a list; `ty` is the element type.
    Index {
        base: Box<TExpr>,
        idx: Box<TExpr>,
        ty: CTy,
    },
    /// Built-in method on a list or string.
    Method {
        recv: Box<TExpr>,
        kind: BuiltinKind,
        casts: Vec<bool>,
        args: Vec<TExpr>,
        ty: CTy,
    },
    /// Built-in free function (math, fs, ...).
    Builtin {
        kind: BuiltinKind,
        casts: Vec<bool>,
        args: Vec<TExpr>,
        ty: CTy,
    },
    /// Button creation with a statically resolved Lutetium callback.
    GuiButton {
        args: Vec<TExpr>,
        handler: usize,
    },
    GuiCallback {
        kind: BuiltinKind,
        args: Vec<TExpr>,
        callback: usize,
    },
}

#[derive(Debug, Clone)]
pub enum TStmt {
    Let {
        init: TExpr,
        slot: u16,
        cast: bool,
    },
    Assign {
        op: AssignOp,
        slot: u16,
        cast: bool,
        value: TExpr,
        ty: BcTy,
    },
    Expr(TExpr),
    If {
        cond: TExpr,
        then: Vec<TStmt>,
        els: Option<Vec<TStmt>>,
    },
    While {
        cond: TExpr,
        body: Vec<TStmt>,
    },
    For {
        slot: u16,
        start: TExpr,
        end: TExpr,
        inclusive: bool,
        body: Vec<TStmt>,
    },
    /// `for x in lst` — `iterable` is checked to be a list; `elem` is the
    /// element type the loop variable receives.
    ForIn {
        slot: u16,
        iterable: TExpr,
        elem: BcTy,
        body: Vec<TStmt>,
    },
    Return {
        value: Option<TExpr>,
    },
    Break,
    Continue,
}

#[derive(Debug, Clone)]
pub struct TFun {
    pub name: String,
    pub params: Vec<BcTy>,
    pub ret: BcTy,
    pub nlocals: u16,
    pub body: Vec<TStmt>,
}

#[derive(Debug, Clone)]
pub struct TModule {
    pub funcs: Vec<TFun>,
    pub entry: usize,
}

// ---------------------------------------------------------------------------
// Checker
// ---------------------------------------------------------------------------

struct LocalInfo {
    #[allow(dead_code)]
    name: String,
    ty: CTy,
    mutable: bool,
    #[allow(dead_code)]
    span: Span,
}

impl Clone for LocalInfo {
    fn clone(&self) -> Self {
        LocalInfo {
            name: self.name.clone(),
            ty: self.ty.clone(),
            mutable: self.mutable,
            span: self.span,
        }
    }
}

struct FnCx<'a> {
    index: usize,
    sigs: &'a [FnInfo],
    state: &'a [FnState],
    observed: &'a mut Vec<Vec<Obs>>,
    diags: &'a mut Vec<Diagnostic>,
    locals: Vec<LocalInfo>,
    scopes: Vec<HashMap<String, u16>>,
    declared_ret: CTy,
    inferred: Option<CTy>,
    has_valued_return: bool,
    loop_depth: usize,
    /// Top-level (implicit entry) code: no returns allowed, no signature.
    top_level: bool,
}

pub fn check(module: &Module, sigs: &[FnInfo]) -> Result<TModule, Vec<Diagnostic>> {
    // The resolver may have dropped some declarations (duplicates, `print`);
    // map declaration index -> signature index.
    let mut sig_of_decl: Vec<Option<usize>> = Vec::with_capacity(module.funcs.len());
    for decl in &module.funcs {
        sig_of_decl.push(sigs.iter().position(|s| s.name == decl.name));
    }
    let n = sigs.len();
    // state[i]: current best-known return + parameter types of function i.
    let mut state: Vec<FnState> = sigs
        .iter()
        .map(|s| FnState {
            ret: s.ret.map(cty_of).unwrap_or(CTy::Unknown),
            params: s
                .params
                .iter()
                .map(|p| p.map(cty_of).unwrap_or(CTy::Unknown))
                .collect(),
        })
        .collect();

    let mut last: Option<(Vec<TFun>, Option<TFun>, Vec<Diagnostic>)> = None;
    let passes = (n + 2).max(3);
    for _ in 0..passes {
        let mut next = state.clone();
        let mut diags = Vec::new();
        let mut observed: Vec<Vec<Obs>> = sigs
            .iter()
            .map(|s| vec![Obs::Unobserved; s.params.len()])
            .collect();
        let mut tfuns = Vec::with_capacity(module.funcs.len());
        for (di, decl) in module.funcs.iter().enumerate() {
            let Some(si) = sig_of_decl[di] else { continue };
            let mut cx = FnCx {
                index: si,
                sigs,
                state: &state,
                observed: &mut observed,
                diags: &mut diags,
                locals: Vec::new(),
                scopes: vec![HashMap::new()],
                declared_ret: sigs[si].ret.map(cty_of).unwrap_or(CTy::Unknown),
                inferred: None,
                has_valued_return: false,
                loop_depth: 0,
                top_level: false,
            };
            let tf = cx.check_fn(decl);
            next[si].ret = cx.final_ret();
            tfuns.push(tf);
        }

        // Top-level executable statements (implicit entry point).
        let mut init_fun: Option<TFun> = None;
        if !module.top.is_empty() {
            let main_idx = sigs.iter().position(|s| s.name == "main");
            let mut cx = FnCx {
                index: sigs.len(),
                sigs,
                state: &state,
                observed: &mut observed,
                diags: &mut diags,
                locals: Vec::new(),
                scopes: vec![HashMap::new()],
                declared_ret: cty_of(BcTy::Unit),
                inferred: None,
                has_valued_return: false,
                loop_depth: 0,
                top_level: true,
            };
            let mut body = cx.check_top(&module.top);
            if let Some(mi) = main_idx {
                body.push(TStmt::Expr(TExpr::Call {
                    func: mi,
                    casts: vec![],
                    args: vec![],
                    ty: cty_of(BcTy::Unit),
                }));
            }
            init_fun = Some(TFun {
                name: "__lutetium_init".to_string(),
                params: vec![],
                ret: BcTy::Unit,
                nlocals: cx.locals.len() as u16,
                body,
            });
        }

        // Fold observed parameter types into the next state.
        for (si, s) in sigs.iter().enumerate() {
            for (i, p) in s.params.iter().enumerate() {
                if p.is_none() {
                    next[si].params[i] = match &observed[si][i] {
                        Obs::Observed(t) => t.clone(),
                        _ => CTy::Unknown,
                    };
                }
            }
        }

        let converged = next == state;
        last = Some((tfuns, init_fun, diags));
        state = next;
        if converged {
            break;
        }
    }

    let (tfuns, init_fun, mut diags) = last.unwrap();

    // Functions whose return or parameter types could not be pinned down.
    for (i, s) in sigs.iter().enumerate() {
        if s.ret.is_none() && state[i].ret == CTy::Unknown {
            diags.push(
                Diagnostic::error(
                    "E3002",
                    s.span,
                    format!("cannot infer the return type of `{}`", s.name),
                )
                .with_hint(
                    "recursive functions with inferred return types need an explicit annotation, e.g. `-> int`",
                ),
            );
        }
        for (pi, name) in s.params.iter().enumerate() {
            if name.is_none() && state[i].params[pi] == CTy::Unknown {
                diags.push(
                    Diagnostic::error(
                        "E3002",
                        s.span,
                        format!(
                            "cannot infer the type of parameter {} of `{}`",
                            pi + 1,
                            s.name
                        ),
                    )
                    .with_hint(
                        "annotate the parameter (e.g. `x: int`) or make sure every call site passes a value of the same type",
                    ),
                );
            }
        }
    }

    if diags.is_empty() {
        let main_idx = sigs.iter().position(|s| s.name == "main");
        let mut funcs = tfuns;
        let entry = if let Some(init) = init_fun {
            funcs.push(init);
            funcs.len() - 1
        } else {
            main_idx.expect("resolver guarantees main or top-level code")
        };
        Ok(TModule { funcs, entry })
    } else {
        Err(diags)
    }
}

impl<'a> FnCx<'a> {
    fn err(&mut self, code: &'static str, span: Span, msg: String) {
        self.diags.push(Diagnostic::error(code, span, msg));
    }

    fn hint_err(&mut self, code: &'static str, span: Span, msg: String, hint: String) {
        self.diags
            .push(Diagnostic::error(code, span, msg).with_hint(hint));
    }

    fn final_ret(&self) -> CTy {
        if self.declared_ret != CTy::Unknown {
            return self.declared_ret.clone();
        }
        match &self.inferred {
            Some(t) => t.clone(),
            None => cty_of(BcTy::Unit),
        }
    }

    fn check_fn(&mut self, decl: &FnDecl) -> TFun {
        // parameters: slots 0..arity, in their own scope. Unannotated
        // parameters use the fixpoint state (inferred from call sites).
        for (i, p) in decl.params.iter().enumerate() {
            // validate the annotation itself (unknown type names, etc.)
            if let Some(ann) = &p.ty {
                if matches!(&ann.ty, TypeKind::Named(_)) {
                    crate::resolve::ann_to_opt(ann, self.diags);
                }
            }
            let slot = self.locals.len() as u16;
            let ty = match self.sigs[self.index].params.get(i) {
                Some(Some(t)) => cty_of(*t),
                Some(None) => self.state[self.index].params[i].clone(),
                None => cty_of(BcTy::Unit),
            };
            self.locals.push(LocalInfo {
                name: p.name.clone(),
                ty,
                mutable: true,
                span: p.name_span,
            });
            self.scopes[0].insert(p.name.clone(), slot);
        }

        let mut body = self.check_block(&decl.body);

        // Expression-bodied functions: a trailing non-`unit` expression is an
        // implicit return.
        if let Some(t) = block_value(&body) {
            if self.declared_ret == CTy::K(BcTy::Unit) {
                self.hint_err(
                    "E3001",
                    decl.body.span,
                    format!(
                        "function `{}` returns `unit` but its last expression produces a value of type `{}`",
                        decl.name,
                        t.name()
                    ),
                    "add a return type annotation (e.g. `-> int`) to use the expression-bodied form".into(),
                );
            } else {
                self.has_valued_return = true;
                if self.declared_ret != CTy::Unknown {
                    if castable(t.clone(), self.declared_ret.clone()).is_none() {
                        self.err(
                            "E3001",
                            decl.body.span,
                            format!(
                                "return type mismatch: `{}` is declared to return `{}`, but its last expression produces `{}`",
                                decl.name,
                                self.declared_ret.name(),
                                t.name()
                            ),
                        );
                    }
                } else {
                    match unify(self.inferred.clone(), t.clone()) {
                        Ok(u) => self.inferred = Some(u),
                        Err(()) => {
                            let prev = self.inferred.clone().unwrap_or(CTy::Unknown);
                            self.hint_err(
                                "E3002",
                                decl.body.span,
                                format!(
                                    "conflicting return types in `{}`: `{}` and `{}`",
                                    decl.name,
                                    prev.name(),
                                    t.name()
                                ),
                                "annotate the return type explicitly, e.g. `-> int`".into(),
                            );
                        }
                    }
                }
                if let Some(TStmt::Expr(e)) = body.last_mut() {
                    let e = std::mem::replace(e, TExpr::Bool(false));
                    *body.last_mut().unwrap() = TStmt::Return { value: Some(e) };
                }
            }
        }

        let ret = self.final_ret();
        if ret != CTy::K(BcTy::Unit) {
            let ends_with_return = matches!(body.last(), Some(TStmt::Return { .. }));
            if !ends_with_return {
                self.hint_err(
                    "E3003",
                    decl.body.span,
                    format!(
                        "function `{}` returns `{}` but its last statement is not a `return`",
                        decl.name,
                        ret.name()
                    ),
                    "add a `return` as the last statement of the function".into(),
                );
            }
        }

        TFun {
            name: decl.name.clone(),
            params: (0..self.sigs[self.index].params.len())
                .map(|i| {
                    self.sigs[self.index].params[i]
                        .map(|t| t)
                        .unwrap_or_else(|| self.state[self.index].params[i].clone().to_bc())
                })
                .collect(),
            ret: ret.clone().to_bc(),
            nlocals: self.locals.len() as u16,
            body,
        }
    }

    /// Top-level (implicit entry) statements.
    fn check_top(&mut self, stmts: &[Stmt]) -> Vec<TStmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            if let Some(t) = self.check_stmt(s) {
                out.push(t);
            }
        }
        out
    }

    fn check_block(&mut self, block: &Block) -> Vec<TStmt> {
        self.scopes.push(HashMap::new());
        let mut out = Vec::with_capacity(block.stmts.len());
        for s in &block.stmts {
            if let Some(t) = self.check_stmt(s) {
                out.push(t);
            }
        }
        self.scopes.pop();
        out
    }

    fn lookup(&mut self, name: &str, span: Span) -> Option<u16> {
        for scope in self.scopes.iter().rev() {
            if let Some(&slot) = scope.get(name) {
                return Some(slot);
            }
        }
        self.hint_err(
            "E3005",
            span,
            format!("unknown variable `{}`", name),
            "define it with `let` before this line".into(),
        );
        None
    }

    fn lookup_quiet(&self, name: &str) -> Option<u16> {
        for scope in self.scopes.iter().rev() {
            if let Some(&slot) = scope.get(name) {
                return Some(slot);
            }
        }
        None
    }

    fn check_stmt(&mut self, s: &Stmt) -> Option<TStmt> {
        match s {
            Stmt::Let {
                mutable,
                name,
                name_span,
                ty,
                init,
                span,
            } => {
                let init_t = self.check_expr(init);
                let it = texpr_ty(&init_t);
                let slot = self.locals.len() as u16;
                let (cast, final_ty) = match ty {
                    Some(ann) => {
                        let dt = cty_of(
                            crate::resolve::ann_to_opt(ann, self.diags).unwrap_or(BcTy::Unit),
                        );
                        match castable(it.clone(), dt.clone()) {
                            Some(cast) => (cast, dt),
                            None => {
                                self.err(
                                    "E3001",
                                    *span,
                                    format!(
                                        "cannot initialize `{}` (declared `{}`) with a value of type `{}`",
                                        name,
                                        ann.name(),
                                        it.name()
                                    ),
                                );
                                (false, dt)
                            }
                        }
                    }
                    None => match it {
                        CTy::Unknown => {
                            self.hint_err(
                                "E3002",
                                *span,
                                format!("cannot infer the type of `{}`", name),
                                "add a type annotation, e.g. `let x: int = ...`".into(),
                            );
                            (false, CTy::Unknown)
                        }
                        t => (false, t),
                    },
                };
                let already = self.scopes.last().unwrap().contains_key(name);
                if already {
                    self.hint_err(
                        "E3011",
                        *name_span,
                        format!("variable `{}` is already defined in this scope", name),
                        "rename the variable or remove the earlier binding".into(),
                    );
                }
                self.scopes.last_mut().unwrap().insert(name.clone(), slot);
                self.locals.push(LocalInfo {
                    name: name.clone(),
                    ty: final_ty,
                    mutable: *mutable,
                    span: *name_span,
                });
                Some(TStmt::Let {
                    init: init_t,
                    slot,
                    cast,
                })
            }
            Stmt::Expr(e) => Some(TStmt::Expr(self.check_expr(e))),
            Stmt::Assign {
                target,
                target_span,
                op,
                value,
                span,
            } => {
                let value_t = self.check_expr(value);
                let vt = texpr_ty(&value_t);

                // Implicit declaration: the first plain assignment to an
                // unknown identifier declares a mutable local with the
                // inferred type. Compound assignment to an unknown name
                // stays an error (checked below via lookup).
                if matches!(op, AssignOp::Set) && self.lookup_quiet(target).is_none() {
                    if vt == CTy::Unknown {
                        self.hint_err(
                            "E3002",
                            *target_span,
                            format!("cannot infer the type of `{}`", target),
                            "add a type annotation, e.g. `let x: int = ...`".into(),
                        );
                    }
                    let slot = self.locals.len() as u16;
                    self.scopes.last_mut().unwrap().insert(target.clone(), slot);
                    self.locals.push(LocalInfo {
                        name: target.clone(),
                        ty: vt,
                        mutable: true,
                        span: *target_span,
                    });
                    return Some(TStmt::Let {
                        init: value_t,
                        slot,
                        cast: false,
                    });
                }

                let slot = match self.lookup(target, *target_span) {
                    Some(s) => s,
                    None => return None,
                };
                let info = self.locals[slot as usize].clone();
                let info_ty = info.ty;
                if !info.mutable {
                    self.hint_err(
                        "E3008",
                        *target_span,
                        format!("cannot assign to immutable variable `{}`", target),
                        "declare it with `let mut`".into(),
                    );
                }
                let (cast, val_ty) = match op {
                    AssignOp::Set => match castable(vt.clone(), info_ty.clone()) {
                        Some(c) => (c, info_ty),
                        None => {
                            self.err(
                                "E3001",
                                *span,
                                format!(
                                    "cannot assign a value of type `{}` to `{}` of type `{}`",
                                    vt.name(),
                                    target,
                                    info_ty.name()
                                ),
                            );
                            (false, info_ty)
                        }
                    },
                    aop => {
                        let binop = match aop {
                            AssignOp::Add => BinOp::Add,
                            AssignOp::Sub => BinOp::Sub,
                            AssignOp::Mul => BinOp::Mul,
                            AssignOp::Div => BinOp::Div,
                            AssignOp::Set => unreachable!(),
                        };
                        match self.bin_result(binop, info_ty.clone(), vt, *span) {
                            Some((_, rcast, rt)) => {
                                if castable(rt.clone(), info_ty.clone()).is_none() {
                                    self.err(
                                        "E3001",
                                        *span,
                                        format!(
                                            "cannot assign a value of type `{}` to `{}` of type `{}`",
                                            rt.name(),
                                            target,
                                            info_ty.name()
                                        ),
                                    );
                                }
                                (rcast, rt)
                            }
                            None => (false, info_ty),
                        }
                    }
                };
                Some(TStmt::Assign {
                    op: *op,
                    slot,
                    cast,
                    value: value_t,
                    ty: val_ty.to_bc(),
                })
            }
            Stmt::If {
                cond, then, els, ..
            } => {
                let cond_t = self.check_expr(cond);
                self.require_bool(&texpr_ty(&cond_t), cond.span());
                let t = self.check_block(then);
                let e = els.as_ref().map(|b| self.check_block(b));
                Some(TStmt::If {
                    cond: cond_t,
                    then: t,
                    els: e,
                })
            }
            Stmt::While { cond, body, .. } => {
                let cond_t = self.check_expr(cond);
                self.require_bool(&texpr_ty(&cond_t), cond.span());
                self.loop_depth += 1;
                let b = self.check_block(body);
                self.loop_depth -= 1;
                Some(TStmt::While {
                    cond: cond_t,
                    body: b,
                })
            }
            Stmt::For {
                var,
                var_span,
                start,
                end,
                inclusive,
                body,
                ..
            } => {
                let start_t = self.check_expr(start);
                let st = texpr_ty(&start_t);
                let end_t = self.check_expr(end);
                let et = texpr_ty(&end_t);
                for (bound_name, bt) in [("start", st), ("end", et)] {
                    if let CTy::K(k) = bt {
                        if k != BcTy::Int {
                            self.hint_err(
                                "E3010",
                                *var_span,
                                format!("range {} bound must be `int`, found `{}`", bound_name, k.name()),
                                "range loops iterate over ints; float iteration is not supported yet".into(),
                            );
                        }
                    }
                }
                self.loop_depth += 1;
                self.scopes.push(HashMap::new());
                let slot = self.locals.len() as u16;
                self.scopes.last_mut().unwrap().insert(var.clone(), slot);
                self.locals.push(LocalInfo {
                    name: var.clone(),
                    ty: cty_of(BcTy::Int),
                    mutable: true,
                    span: *var_span,
                });
                let b = self.check_block(body);
                self.scopes.pop();
                self.loop_depth -= 1;
                Some(TStmt::For {
                    slot,
                    start: start_t,
                    end: end_t,
                    inclusive: *inclusive,
                    body: b,
                })
            }
            Stmt::ForIn {
                var,
                var_span,
                iterable,
                body,
                ..
            } => {
                let iter_t = self.check_expr(iterable);
                let it = texpr_ty(&iter_t);
                // The element type the loop variable receives.
                let elem = match it {
                    CTy::L(elem) => elem,
                    // Failing compilations only; the placeholder is never
                    // executed (same convention as CTy::Unknown -> Unit).
                    CTy::Unknown => BcTy::Unit,
                    other => {
                        self.hint_err(
                            "E3010",
                            *var_span,
                            format!("cannot iterate over a value of type `{}`", other.name()),
                            "only lists are iterable; for number loops use a range like `0..n`"
                                .into(),
                        );
                        BcTy::Unit
                    }
                };
                self.loop_depth += 1;
                self.scopes.push(HashMap::new());
                let slot = self.locals.len() as u16;
                self.scopes.last_mut().unwrap().insert(var.clone(), slot);
                self.locals.push(LocalInfo {
                    name: var.clone(),
                    ty: cty_of(elem),
                    mutable: true,
                    span: *var_span,
                });
                let b = self.check_block(body);
                self.scopes.pop();
                self.loop_depth -= 1;
                Some(TStmt::ForIn {
                    slot,
                    iterable: iter_t,
                    elem,
                    body: b,
                })
            }
            Stmt::Return { value, span } => {
                if self.top_level {
                    self.hint_err(
                        "E3014",
                        *span,
                        "`return` outside of a function".into(),
                        "top-level statements run before `main`; move the return into a function"
                            .into(),
                    );
                    return None;
                }
                match value {
                    Some(e) => {
                        let t = self.check_expr(e);
                        let vt = texpr_ty(&t);
                        self.has_valued_return = true;
                        if self.declared_ret != CTy::Unknown {
                            if castable(vt.clone(), self.declared_ret.clone()).is_none() {
                                self.err(
                                    "E3001",
                                    *span,
                                    format!(
                                        "return type mismatch: expected `{}`, found `{}`",
                                        self.declared_ret.name(),
                                        vt.name()
                                    ),
                                );
                            }
                        } else {
                            match unify(self.inferred.clone(), vt.clone()) {
                                Ok(u) => self.inferred = Some(u),
                                Err(()) => {
                                    let prev = self.inferred.clone().unwrap_or(CTy::Unknown);
                                    self.hint_err(
                                        "E3002",
                                        *span,
                                        format!(
                                            "conflicting return types in `{}`: `{}` and `{}`",
                                            self.sigs[self.index].name,
                                            prev.name(),
                                            vt.name()
                                        ),
                                        "annotate the return type explicitly, e.g. `-> int`".into(),
                                    );
                                }
                            }
                        }
                        Some(TStmt::Return { value: Some(t) })
                    }
                    None => {
                        if let CTy::K(t) = self.declared_ret {
                            if t != BcTy::Unit {
                                self.err(
                                    "E3001",
                                    *span,
                                    format!(
                                        "missing return value: `{}` returns `{}`",
                                        self.sigs[self.index].name,
                                        t.name()
                                    ),
                                );
                            }
                        } else {
                            match unify(self.inferred.clone(), cty_of(BcTy::Unit)) {
                                Ok(u) => self.inferred = Some(u),
                                Err(()) => {
                                    let prev = self.inferred.clone().unwrap_or(CTy::Unknown);
                                    self.hint_err(
                                        "E3002",
                                        *span,
                                        format!(
                                            "conflicting return types in `{}`: `{}` and `unit`",
                                            self.sigs[self.index].name,
                                            prev.name()
                                        ),
                                        "annotate the return type explicitly".into(),
                                    );
                                }
                            }
                        }
                        Some(TStmt::Return { value: None })
                    }
                }
            }
            Stmt::Break { span } => {
                if self.loop_depth == 0 {
                    self.hint_err(
                        "E3012",
                        *span,
                        "`break` outside of a loop".into(),
                        "`break` can only be used inside `while`".into(),
                    );
                }
                Some(TStmt::Break)
            }
            Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    self.hint_err(
                        "E3013",
                        *span,
                        "`continue` outside of a loop".into(),
                        "`continue` can only be used inside `while`".into(),
                    );
                }
                Some(TStmt::Continue)
            }
        }
    }

    fn require_bool(&mut self, t: &CTy, span: Span) {
        match t {
            CTy::K(BcTy::Bool) | CTy::Unknown => {}
            other => {
                self.err(
                    "E3009",
                    span,
                    format!("condition must be a `bool`, found `{}`", other.name()),
                );
            }
        }
    }

    /// Result type of an arithmetic binary operation, plus per-operand casts.
    /// Returns None after reporting an error.
    fn bin_result(&mut self, op: BinOp, a: CTy, b: CTy, span: Span) -> Option<(bool, bool, CTy)> {
        let sym = match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            _ => unreachable!(),
        };
        match (a.clone(), b.clone()) {
            (CTy::K(BcTy::Int), CTy::K(BcTy::Int)) => Some((false, false, cty_of(BcTy::Int))),
            (CTy::Unknown, other) | (other, CTy::Unknown) if is_numeric(&other) => {
                Some((false, false, CTy::Unknown))
            }
            (x, y) if is_numeric(&x) && is_numeric(&y) => {
                let lcast = matches!(x, CTy::K(BcTy::Int));
                let rcast = matches!(y, CTy::K(BcTy::Int));
                Some((lcast, rcast, cty_of(BcTy::Float)))
            }
            (CTy::K(BcTy::Str), CTy::K(BcTy::Str)) if op == BinOp::Add => {
                Some((false, false, cty_of(BcTy::Str)))
            }
            _ => {
                let hint = if op == BinOp::Add {
                    "use `+` for numbers or to concatenate two strings".to_string()
                } else {
                    format!("`{}` is defined for two ints or two floats", sym)
                };
                self.hint_err(
                    "E3010",
                    span,
                    format!(
                        "cannot apply `{}` to `{}` and `{}`",
                        sym,
                        a.name(),
                        b.name()
                    ),
                    hint,
                );
                None
            }
        }
    }

    fn check_expr(&mut self, e: &Expr) -> TExpr {
        match e {
            Expr::Int(v, _) => TExpr::Int(*v),
            Expr::Float(v, _) => TExpr::Float(*v),
            Expr::Bool(v, _) => TExpr::Bool(*v),
            Expr::Str(v, _) => TExpr::Str(v.clone()),
            Expr::Ident(name, span) => {
                let slot = match self.lookup(name, *span) {
                    Some(s) => s,
                    None => return TExpr::Bool(false),
                };
                let ty = self.locals[slot as usize].ty.clone();
                TExpr::Local { slot, ty }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs_t = self.check_expr(lhs);
                let rhs_t = self.check_expr(rhs);
                let lt = texpr_ty(&lhs_t);
                let rt = texpr_ty(&rhs_t);
                match op {
                    ABinOp::And | ABinOp::Or => {
                        for t in [lt.clone(), rt.clone()] {
                            if let CTy::K(k) = t {
                                if k != BcTy::Bool {
                                    self.err(
                                        "E3010",
                                        *span,
                                        format!(
                                            "`{}` expects bool operands, found `{}`",
                                            if *op == ABinOp::And { "&&" } else { "||" },
                                            k.name()
                                        ),
                                    );
                                }
                            }
                        }
                        TExpr::Logic {
                            op: if *op == ABinOp::And {
                                LogicOp::And
                            } else {
                                LogicOp::Or
                            },
                            lhs: Box::new(lhs_t),
                            rhs: Box::new(rhs_t),
                        }
                    }
                    ABinOp::Eq | ABinOp::Ne => {
                        let cmp_sym = if *op == ABinOp::Eq { "==" } else { "!=" };
                        let ok = match (lt.clone(), rt.clone()) {
                            (CTy::K(a), CTy::K(b)) if a == b => true,
                            (CTy::K(BcTy::Int), CTy::K(BcTy::Float))
                            | (CTy::K(BcTy::Float), CTy::K(BcTy::Int)) => true,
                            // Lists compare element-wise when their element
                            // types are compatible (int/float widening).
                            (CTy::L(a), CTy::L(b))
                                if a == b
                                    || (a == BcTy::Int && b == BcTy::Float)
                                    || (a == BcTy::Float && b == BcTy::Int) =>
                            {
                                true
                            }
                            (CTy::Unknown, _) | (_, CTy::Unknown) => true,
                            _ => false,
                        };
                        if !ok {
                            self.hint_err(
                                "E3010",
                                *span,
                                format!(
                                    "cannot compare `{}` and `{}` with `{}`",
                                    lt.name(),
                                    rt.name(),
                                    cmp_sym
                                ),
                                "compare values of the same type".into(),
                            );
                        }
                        let lcast =
                            matches!(lt, CTy::K(BcTy::Int)) && matches!(rt, CTy::K(BcTy::Float));
                        let rcast =
                            matches!(rt, CTy::K(BcTy::Int)) && matches!(lt, CTy::K(BcTy::Float));
                        let operand_ty = match (lt.clone(), rt.clone()) {
                            (CTy::L(_), CTy::L(_)) => BcTy::List,
                            (CTy::K(a), CTy::K(b)) if a == b => a,
                            (CTy::K(BcTy::Int), CTy::K(BcTy::Float))
                            | (CTy::K(BcTy::Float), CTy::K(BcTy::Int)) => BcTy::Float,
                            _ => BcTy::Int,
                        };
                        TExpr::Binary {
                            op: if *op == ABinOp::Eq {
                                BinOp::Eq
                            } else {
                                BinOp::Ne
                            },
                            ty: cty_of(BcTy::Bool),
                            operand_ty,
                            lcast,
                            rcast,
                            lhs: Box::new(lhs_t),
                            rhs: Box::new(rhs_t),
                        }
                    }
                    ABinOp::Lt | ABinOp::Le | ABinOp::Gt | ABinOp::Ge => {
                        if !is_numeric(&lt) || !is_numeric(&rt) {
                            self.hint_err(
                                "E3010",
                                *span,
                                format!(
                                    "ordering comparison requires numeric operands, found `{}` and `{}`",
                                    lt.name(),
                                    rt.name()
                                ),
                                "`<`, `<=`, `>` and `>=` are defined for ints and floats".into(),
                            );
                        }
                        let operand_ty = match (lt.clone(), rt.clone()) {
                            (CTy::K(a), CTy::K(b)) => {
                                if a == BcTy::Float || b == BcTy::Float {
                                    BcTy::Float
                                } else {
                                    BcTy::Int
                                }
                            }
                            _ => BcTy::Int,
                        };
                        let lcast =
                            matches!(lt, CTy::K(BcTy::Int)) && matches!(rt, CTy::K(BcTy::Float));
                        let rcast =
                            matches!(rt, CTy::K(BcTy::Int)) && matches!(lt, CTy::K(BcTy::Float));
                        TExpr::Binary {
                            op: match op {
                                ABinOp::Lt => BinOp::Lt,
                                ABinOp::Le => BinOp::Le,
                                ABinOp::Gt => BinOp::Gt,
                                _ => BinOp::Ge,
                            },
                            ty: cty_of(BcTy::Bool),
                            operand_ty,
                            lcast,
                            rcast,
                            lhs: Box::new(lhs_t),
                            rhs: Box::new(rhs_t),
                        }
                    }
                    ABinOp::Add | ABinOp::Sub | ABinOp::Mul | ABinOp::Div | ABinOp::Mod => {
                        let irop = match op {
                            ABinOp::Add => BinOp::Add,
                            ABinOp::Sub => BinOp::Sub,
                            ABinOp::Mul => BinOp::Mul,
                            ABinOp::Div => BinOp::Div,
                            _ => BinOp::Mod,
                        };
                        match self.bin_result(irop, lt, rt, *span) {
                            Some((lcast, rcast, ty)) => {
                                let operand_ty = ty.to_bc();
                                TExpr::Binary {
                                    op: irop,
                                    ty,
                                    operand_ty,
                                    lcast,
                                    rcast,
                                    lhs: Box::new(lhs_t),
                                    rhs: Box::new(rhs_t),
                                }
                            }
                            None => TExpr::Binary {
                                op: irop,
                                ty: CTy::Unknown,
                                operand_ty: BcTy::Int,
                                lcast: false,
                                rcast: false,
                                lhs: Box::new(lhs_t),
                                rhs: Box::new(rhs_t),
                            },
                        }
                    }
                }
            }
            Expr::Unary { op, expr, span } => {
                let inner = self.check_expr(expr);
                let t = texpr_ty(&inner);
                match op {
                    AUnOp::Neg => {
                        let ty = match t {
                            CTy::K(BcTy::Int) => BcTy::Int,
                            CTy::K(BcTy::Float) => BcTy::Float,
                            CTy::Unknown => {
                                self.err(
                                    "E3010",
                                    *span,
                                    format!("unary `-` expects a number, found `{}`", t.name()),
                                );
                                BcTy::Int
                            }
                            other => {
                                self.err(
                                    "E3010",
                                    *span,
                                    format!("unary `-` expects a number, found `{}`", other.name()),
                                );
                                BcTy::Int
                            }
                        };
                        TExpr::Unary {
                            op: UnOp::Neg,
                            ty,
                            expr: Box::new(inner),
                        }
                    }
                    AUnOp::Not => {
                        if let CTy::K(k) = t {
                            if k != BcTy::Bool {
                                self.err(
                                    "E3010",
                                    *span,
                                    format!("`!` expects a bool, found `{}`", k.name()),
                                );
                            }
                        }
                        TExpr::Unary {
                            op: UnOp::Not,
                            ty: BcTy::Bool,
                            expr: Box::new(inner),
                        }
                    }
                }
            }
            Expr::List(elems, span) => {
                let mut tchecked = Vec::with_capacity(elems.len());
                let mut elem_ty: Option<BcTy> = None;
                for e in elems {
                    let t = self.check_expr(e);
                    let et = texpr_ty(&t);
                    match et {
                        CTy::K(k) => {
                            if k == BcTy::List {
                                self.hint_err(
                                    "E3001",
                                    e.span(),
                                    "nested lists are not supported yet".into(),
                                    "list elements must be int, float, bool or str".into(),
                                );
                            }
                            elem_ty = match elem_ty {
                                None => Some(k),
                                Some(prev) => match (prev, k) {
                                    (BcTy::Int, BcTy::Float) | (BcTy::Float, BcTy::Int) => {
                                        Some(BcTy::Float)
                                    }
                                    (p, k2) if p == k2 => Some(p),
                                    (p, k2) => {
                                        self.err(
                                            "E3001",
                                            *span,
                                            format!(
                                                "list elements must have one type: found `{}` and `{}`",
                                                CTy::K(p).name(),
                                                CTy::K(k2).name()
                                            ),
                                        );
                                        Some(prev)
                                    }
                                },
                            };
                        }
                        CTy::L(_) | CTy::R(_) => {
                            self.hint_err(
                                "E3001",
                                e.span(),
                                "nested lists/maps are not supported yet".into(),
                                "list elements must be int, float, bool or str".into(),
                            );
                        }
                        CTy::Unknown => {}
                    }
                    tchecked.push(t);
                }
                let ty = match elem_ty {
                    Some(t) => t,
                    None => {
                        self.hint_err(
                            "E3002",
                            *span,
                            "cannot infer the element type of an empty list".into(),
                            "add at least one element (e.g. `[0]`) so the element type can be inferred".into(),
                        );
                        BcTy::Int
                    }
                };
                TExpr::List {
                    elems: tchecked,
                    ty,
                }
            }
            Expr::Map(entries, span) => {
                if entries.len() > 127 {
                    // The BUILTIN instruction stores argc in one byte, and a
                    // map literal lowers to interleaved (key, value) pairs.
                    self.err(
                        "E3016",
                        *span,
                        format!(
                            "map literal has {} entries; the maximum is 127",
                            entries.len()
                        ),
                    );
                }
                let mut tchecked = Vec::with_capacity(entries.len());
                for (key, _key_span, v) in entries {
                    let t = self.check_expr(v);
                    match texpr_ty(&t) {
                        CTy::K(k) => {
                            if k == BcTy::List || k == BcTy::Map {
                                self.hint_err(
                                    "E3001",
                                    v.span(),
                                    "nested lists/maps are not supported yet".into(),
                                    "map values must be int, float, bool or str".into(),
                                );
                            }
                        }
                        CTy::L(_) | CTy::R(_) => {
                            self.hint_err(
                                "E3001",
                                v.span(),
                                "nested lists/maps are not supported yet".into(),
                                "map values must be int, float, bool or str".into(),
                            );
                        }
                        CTy::Unknown => {}
                    }
                    tchecked.push((key.clone(), t));
                }
                TExpr::Map { entries: tchecked }
            }
            Expr::Field {
                recv,
                name,
                name_span,
                span: _,
            } => {
                let recv_t = self.check_expr(recv);
                let ty = match texpr_ty(&recv_t) {
                    CTy::R(fields) => match fields.iter().find(|(k, _)| k == name) {
                        Some((_, vt)) => cty_of(*vt),
                        None => {
                            let names = fields
                                .iter()
                                .map(|(k, _)| k.as_str())
                                .collect::<Vec<_>>()
                                .join(", ");
                            self.hint_err(
                                "E3010",
                                *name_span,
                                format!("map has no field `{}`", name),
                                if names.is_empty() {
                                    "this map has no fields".to_string()
                                } else {
                                    format!("this map's fields are: {}", names)
                                },
                            );
                            CTy::Unknown
                        }
                    },
                    other => {
                        self.hint_err(
                            "E3010",
                            *name_span,
                            format!(
                                "cannot access field `{}` on a value of type `{}`",
                                name,
                                other.name()
                            ),
                            "only maps support field access in 0.4".into(),
                        );
                        CTy::Unknown
                    }
                };
                TExpr::Field {
                    recv: Box::new(recv_t),
                    key: name.clone(),
                    ty,
                }
            }
            Expr::Index { base, idx, span } => {
                let base_t = self.check_expr(base);
                let idx_t = self.check_expr(idx);
                let it = texpr_ty(&idx_t);
                if let CTy::K(k) = it {
                    if k != BcTy::Int {
                        self.err(
                            "E3010",
                            idx.span(),
                            format!("list index must be `int`, found `{}`", k.name()),
                        );
                    }
                }
                let ty = match texpr_ty(&base_t) {
                    CTy::L(elem) => cty_of(elem),
                    CTy::K(BcTy::List) => {
                        self.hint_err(
                            "E3002",
                            *span,
                            "cannot infer the element type of this list".into(),
                            "make the list come from a literal or an inferred function return"
                                .into(),
                        );
                        CTy::Unknown
                    }
                    other => {
                        self.hint_err(
                            "E3010",
                            *span,
                            format!("cannot index a value of type `{}`", other.name()),
                            "only lists support indexing in 0.4".into(),
                        );
                        CTy::Unknown
                    }
                };
                TExpr::Index {
                    base: Box::new(base_t),
                    idx: Box::new(idx_t),
                    ty,
                }
            }
            Expr::Method {
                recv,
                name,
                name_span,
                args,
                span,
            } => {
                let recv_t = self.check_expr(recv);
                let rt = texpr_ty(&recv_t);
                // Element type of the receiver list (equals the receiver
                // type for scalars; used for push/set parameter checks).
                let recv_elem = match rt.clone() {
                    CTy::L(e) => cty_of(e),
                    other => other,
                };
                let mut targs = Vec::with_capacity(args.len());
                let mut atys = Vec::with_capacity(args.len());
                for a in args {
                    let t = self.check_expr(a);
                    atys.push(texpr_ty(&t));
                    targs.push(t);
                }
                // Record-map methods: field types are per key, so `get` and
                // `set` are checked against the literal key and the record's
                // fields rather than a uniform value type.
                if let CTy::R(fields) = rt.clone() {
                    let known = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, t)| *t);
                    let field_list = || {
                        let names = fields
                            .iter()
                            .map(|(k, _)| k.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        if names.is_empty() {
                            "this map has no fields".to_string()
                        } else {
                            format!("this map's fields are: {}", names)
                        }
                    };
                    match name.as_str() {
                        "len" => {
                            if targs.len() != 0 {
                                self.hint_err(
                                    "E3007",
                                    *span,
                                    format!(
                                        "method `len` expects 0 arguments, found {}",
                                        targs.len()
                                    ),
                                    "adjust the call to match the method signature".into(),
                                );
                            }
                            return TExpr::Method {
                                recv: Box::new(recv_t),
                                kind: BuiltinKind::ListLen,
                                casts: vec![],
                                args: targs,
                                ty: cty_of(BcTy::Int),
                            };
                        }
                        "get" => {
                            if targs.len() != 1 {
                                self.hint_err(
                                    "E3007",
                                    *span,
                                    format!(
                                        "method `get` expects 1 argument, found {}",
                                        targs.len()
                                    ),
                                    "adjust the call to match the method signature".into(),
                                );
                                return TExpr::Bool(false);
                            }
                            let key = match &targs[0] {
                                TExpr::Str(s) => s.clone(),
                                t => {
                                    self.hint_err(
                                        "E3010",
                                        args[0].span(),
                                        format!(
                                            "map key must be a literal string, found a dynamic `{}`",
                                            texpr_ty(t).name()
                                        ),
                                        "use field access (`m.key`) or a literal key: `m.get(\"key\")`".into(),
                                    );
                                    return TExpr::Bool(false);
                                }
                            };
                            let Some(vt) = known(&key) else {
                                self.hint_err(
                                    "E3010",
                                    args[0].span(),
                                    format!("map has no key `{}`", key),
                                    field_list(),
                                );
                                return TExpr::Bool(false);
                            };
                            return TExpr::Method {
                                recv: Box::new(recv_t),
                                kind: BuiltinKind::MapGet,
                                casts: vec![],
                                args: targs,
                                ty: cty_of(vt),
                            };
                        }
                        "set" => {
                            if targs.len() != 2 {
                                self.hint_err(
                                    "E3007",
                                    *span,
                                    format!(
                                        "method `set` expects 2 arguments, found {}",
                                        targs.len()
                                    ),
                                    "adjust the call to match the method signature".into(),
                                );
                                return TExpr::Bool(false);
                            }
                            let key = match &targs[0] {
                                TExpr::Str(s) => s.clone(),
                                t => {
                                    self.hint_err(
                                        "E3010",
                                        args[0].span(),
                                        format!(
                                            "map key must be a literal string, found a dynamic `{}`",
                                            texpr_ty(t).name()
                                        ),
                                        "record maps have a fixed set of keys; use a literal key: `m.set(\"key\", value)`".into(),
                                    );
                                    return TExpr::Bool(false);
                                }
                            };
                            let Some(vt) = known(&key) else {
                                self.hint_err(
                                    "E3010",
                                    args[0].span(),
                                    format!("cannot add field `{}` to a record map", key),
                                    format!(
                                        "{}; new keys cannot be added after creation",
                                        field_list()
                                    ),
                                );
                                return TExpr::Bool(false);
                            };
                            // The value must match the field's type (with the
                            // usual int -> float widening).
                            let at = atys[1].clone();
                            let cast = match castable(at.clone(), cty_of(vt)) {
                                Some(c) => c,
                                None => {
                                    self.err(
                                        "E3001",
                                        args[1].span(),
                                        format!(
                                            "argument 2 of `set`: expected `{}`, found `{}`",
                                            CTy::K(vt).name(),
                                            at.name()
                                        ),
                                    );
                                    false
                                }
                            };
                            return TExpr::Method {
                                recv: Box::new(recv_t),
                                kind: BuiltinKind::MapSet,
                                casts: vec![false, cast],
                                args: targs,
                                ty: cty_of(BcTy::Unit),
                            };
                        }
                        _ => {}
                    }
                    // Any other method falls through to the generic table,
                    // which reports the unknown-method error for maps.
                }
                let builtin = match (rt, name.as_str()) {
                    (CTy::L(_), "len") => Some((BuiltinKind::ListLen, 0, cty_of(BcTy::Int))),
                    (CTy::L(_elem), "push") => Some((BuiltinKind::ListPush, 1, cty_of(BcTy::Unit))),
                    (CTy::L(elem), "pop") => Some((BuiltinKind::ListPop, 0, cty_of(elem))),
                    (CTy::L(elem), "get") => Some((BuiltinKind::ListGet, 1, cty_of(elem))),
                    (CTy::L(_elem), "set") => Some((BuiltinKind::ListSet, 2, cty_of(BcTy::Unit))),
                    (CTy::K(BcTy::Str), "len") => Some((BuiltinKind::StrLen, 0, cty_of(BcTy::Int))),
                    (CTy::K(BcTy::Str), "upper") => {
                        Some((BuiltinKind::StrUpper, 0, cty_of(BcTy::Str)))
                    }
                    (CTy::K(BcTy::Str), "lower") => {
                        Some((BuiltinKind::StrLower, 0, cty_of(BcTy::Str)))
                    }
                    (CTy::K(BcTy::Str), "trim") => {
                        Some((BuiltinKind::StrTrim, 0, cty_of(BcTy::Str)))
                    }
                    (CTy::K(BcTy::Str), "contains") => {
                        Some((BuiltinKind::StrContains, 1, cty_of(BcTy::Bool)))
                    }
                    (CTy::K(BcTy::Str), "replace") => {
                        Some((BuiltinKind::StrReplace, 2, cty_of(BcTy::Str)))
                    }
                    (CTy::K(BcTy::Str), "split") => {
                        Some((BuiltinKind::StrSplit, 1, CTy::L(BcTy::Str)))
                    }
                    (other, m) => {
                        self.hint_err(
                            "E3015",
                            *name_span,
                            format!("unknown method `{}` for type `{}`", m, other.name()),
                            "see STANDARD_LIBRARY.md for the available built-in methods".into(),
                        );
                        None
                    }
                };
                let Some((kind, want_argc, ty)) = builtin else {
                    return TExpr::Bool(false);
                };
                if targs.len() != want_argc {
                    self.hint_err(
                        "E3007",
                        *span,
                        format!(
                            "method `{}` expects {} argument(s), found {}",
                            name,
                            want_argc,
                            targs.len()
                        ),
                        "adjust the call to match the method signature".into(),
                    );
                }
                // parameter expectations per kind
                let param_tys: Vec<CTy> = match kind {
                    BuiltinKind::ListPush => vec![recv_elem],
                    BuiltinKind::ListGet => vec![cty_of(BcTy::Int)],
                    BuiltinKind::ListSet => vec![cty_of(BcTy::Int), recv_elem],
                    BuiltinKind::StrContains => vec![cty_of(BcTy::Str)],
                    BuiltinKind::StrReplace => vec![cty_of(BcTy::Str), cty_of(BcTy::Str)],
                    BuiltinKind::StrSplit => vec![cty_of(BcTy::Str)],
                    _ => vec![],
                };
                let mut casts = Vec::with_capacity(targs.len());
                for (i, t) in targs.iter().enumerate() {
                    let at = texpr_ty(t);
                    let want = param_tys.get(i).cloned().unwrap_or(CTy::Unknown);
                    let cast = match castable(at.clone(), want.clone()) {
                        Some(c) => c,
                        None => {
                            self.err(
                                "E3001",
                                args[i].span(),
                                format!(
                                    "argument {} of `{}`: expected `{}`, found `{}`",
                                    i + 1,
                                    name,
                                    want.name(),
                                    at.name()
                                ),
                            );
                            false
                        }
                    };
                    casts.push(cast);
                }
                TExpr::Method {
                    recv: Box::new(recv_t),
                    kind,
                    casts,
                    args: targs,
                    ty,
                }
            }
            Expr::Call {
                callee,
                callee_span,
                args,
                span,
            } => {
                if let Some(kind) = BuiltinKind::function_name_of(callee).filter(|kind| {
                    matches!(
                        kind,
                        BuiltinKind::GuiOnClick
                            | BuiltinKind::GuiOnKey
                            | BuiltinKind::GuiKeyHandler
                    )
                }) {
                    let expected_argc = if kind == BuiltinKind::GuiKeyHandler {
                        3
                    } else {
                        2
                    };
                    if args.len() != expected_argc {
                        self.hint_err(
                            "E3007",
                            *span,
                            format!(
                                "`{}` expects {} arguments, found {}",
                                callee,
                                expected_argc,
                                args.len()
                            ),
                            "pass the window, an optional key code, and a named handler".into(),
                        );
                        for arg in args {
                            let _ = self.check_expr(arg);
                        }
                        return TExpr::Bool(false);
                    }
                    let window = self.check_expr(&args[0]);
                    if castable(texpr_ty(&window), cty_of(BcTy::Window)).is_none() {
                        self.err(
                            "E3001",
                            args[0].span(),
                            "GUI callback registration requires a window".into(),
                        );
                    }
                    let mut cb_index = 1;
                    let mut checked = vec![window];
                    if kind == BuiltinKind::GuiKeyHandler {
                        let key = self.check_expr(&args[1]);
                        if castable(texpr_ty(&key), cty_of(BcTy::Int)).is_none() {
                            self.err("E3001", args[1].span(), "key code must be int".into());
                        }
                        checked.push(key);
                        cb_index = 2;
                    }
                    let callback_name = match &args[cb_index] {
                        Expr::Ident(name, _) => name,
                        other => {
                            self.err(
                                "E3001",
                                other.span(),
                                "GUI callback must name a Lutetium function".into(),
                            );
                            return TExpr::Bool(false);
                        }
                    };
                    let Some(callback) =
                        self.sigs.iter().position(|sig| sig.name == *callback_name)
                    else {
                        self.err(
                            "E3006",
                            args[cb_index].span(),
                            format!("unknown GUI callback `{}`", callback_name),
                        );
                        return TExpr::Bool(false);
                    };
                    let callback_sig = &self.sigs[callback];
                    let callback_ret = callback_sig
                        .ret
                        .map(cty_of)
                        .unwrap_or_else(|| self.state[callback].ret.clone());
                    if callback_sig.params != vec![Some(BcTy::Window)]
                        || (callback_ret != CTy::Unknown && callback_ret != cty_of(BcTy::Unit))
                    {
                        self.err(
                            "E3001",
                            args[cb_index].span(),
                            format!(
                                "GUI callback `{}` must have signature `fn {}(window: window) -> unit`",
                                callback_name, callback_name
                            ),
                        );
                    }
                    return TExpr::GuiCallback {
                        kind,
                        args: checked,
                        callback,
                    };
                }
                if callee == "gui_button" {
                    if args.len() != 8 {
                        self.hint_err(
                            "E3007",
                            *span,
                            format!("`gui_button` expects 8 arguments, found {}", args.len()),
                            "pass window, button id, caption, x, y, width, height, and handler"
                                .into(),
                        );
                        for arg in args {
                            let _ = self.check_expr(arg);
                        }
                        return TExpr::Bool(false);
                    }
                    let mut checked = Vec::with_capacity(7);
                    let expected = [
                        cty_of(BcTy::Window),
                        cty_of(BcTy::Int),
                        cty_of(BcTy::Str),
                        cty_of(BcTy::Int),
                        cty_of(BcTy::Int),
                        cty_of(BcTy::Int),
                        cty_of(BcTy::Int),
                    ];
                    for (i, (arg, want)) in args[..7].iter().zip(expected).enumerate() {
                        let value = self.check_expr(arg);
                        let got = texpr_ty(&value);
                        if castable(got.clone(), want.clone()).is_none() {
                            self.err(
                                "E3001",
                                arg.span(),
                                format!(
                                    "argument {} of `gui_button`: expected `{}`, found `{}`",
                                    i + 1,
                                    want.name(),
                                    got.name()
                                ),
                            );
                        }
                        checked.push(value);
                    }
                    let handler_name = match &args[7] {
                        Expr::Ident(name, _) => name,
                        other => {
                            self.err(
                                "E3001",
                                other.span(),
                                "gui_button handler must be a named Lutetium function".into(),
                            );
                            return TExpr::Bool(false);
                        }
                    };
                    let Some(handler) = self.sigs.iter().position(|sig| sig.name == *handler_name)
                    else {
                        self.err(
                            "E3006",
                            args[7].span(),
                            format!("unknown GUI handler `{}`", handler_name),
                        );
                        return TExpr::Bool(false);
                    };
                    let sig = &self.sigs[handler];
                    let handler_ret = sig
                        .ret
                        .map(cty_of)
                        .unwrap_or_else(|| self.state[handler].ret.clone());
                    if sig.params != vec![Some(BcTy::Window)]
                        || (handler_ret != CTy::Unknown && handler_ret != cty_of(BcTy::Unit))
                    {
                        self.err("E3001", args[7].span(), format!("GUI handler `{}` must have signature `fn {}(window: window) -> unit`", handler_name, handler_name));
                    }
                    return TExpr::GuiButton {
                        args: checked,
                        handler,
                    };
                }
                if callee == "print" {
                    if args.len() != 1 {
                        self.hint_err(
                            "E3007",
                            *span,
                            format!("`print` expects exactly 1 argument, found {}", args.len()),
                            "print one value at a time; concatenate strings with `+`".into(),
                        );
                    }
                    let mut targs = Vec::new();
                    for a in args {
                        targs.push(self.check_expr(a));
                    }
                    return TExpr::Call {
                        func: PRINT_BUILTIN,
                        casts: vec![],
                        args: targs,
                        ty: cty_of(BcTy::Unit),
                    };
                }
                let fi = match self.sigs.iter().position(|s| s.name == callee.as_str()) {
                    Some(i) => i,
                    None => {
                        // built-in free functions (math, fs, ...)
                        if let Some(kind) = BuiltinKind::function_name_of(callee) {
                            let mut targs = Vec::new();
                            let mut casts = Vec::new();
                            for a in args {
                                targs.push(self.check_expr(a));
                            }
                            // `abs`/`min`/`max` are overloaded: all-int
                            // arguments select the int variants so integer
                            // math stays integer (mixed/float -> float).
                            let all_int = !targs.is_empty()
                                && targs
                                    .iter()
                                    .all(|t| matches!(texpr_ty(t), CTy::K(BcTy::Int)));
                            let kind = if all_int {
                                match kind {
                                    BuiltinKind::AbsFloat => BuiltinKind::AbsInt,
                                    BuiltinKind::MinFloat => BuiltinKind::MinInt,
                                    BuiltinKind::MaxFloat => BuiltinKind::MaxInt,
                                    k => k,
                                }
                            } else {
                                kind
                            };
                            let (want_argc, _) = kind.sig();
                            if targs.len() != want_argc {
                                self.hint_err(
                                    "E3007",
                                    *span,
                                    format!(
                                        "`{}` expects {} argument(s), found {}",
                                        callee,
                                        want_argc,
                                        targs.len()
                                    ),
                                    "adjust the call to match the builtin signature".into(),
                                );
                            }
                            let param_tys: Vec<CTy> = match kind {
                                BuiltinKind::Sqrt
                                | BuiltinKind::Floor
                                | BuiltinKind::Ceil
                                | BuiltinKind::Round
                                | BuiltinKind::Sin
                                | BuiltinKind::Cos
                                | BuiltinKind::Tan
                                | BuiltinKind::Log => vec![cty_of(BcTy::Float)],
                                BuiltinKind::Pow
                                | BuiltinKind::MinFloat
                                | BuiltinKind::MaxFloat => {
                                    vec![cty_of(BcTy::Float), cty_of(BcTy::Float)]
                                }
                                BuiltinKind::MinInt | BuiltinKind::MaxInt => {
                                    vec![cty_of(BcTy::Int), cty_of(BcTy::Int)]
                                }
                                BuiltinKind::ReadText => vec![cty_of(BcTy::Str)],
                                BuiltinKind::WriteText => {
                                    vec![cty_of(BcTy::Str), cty_of(BcTy::Str)]
                                }
                                BuiltinKind::FileExists => vec![cty_of(BcTy::Str)],
                                BuiltinKind::GuiWindow => {
                                    vec![cty_of(BcTy::Str), cty_of(BcTy::Int), cty_of(BcTy::Int)]
                                }
                                BuiltinKind::GuiLabel | BuiltinKind::GuiInput => vec![
                                    cty_of(BcTy::Window),
                                    cty_of(BcTy::Str),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Int),
                                ],
                                BuiltinKind::GuiButton => vec![
                                    cty_of(BcTy::Window),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Str),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Int),
                                    cty_of(BcTy::Int),
                                ],
                                BuiltinKind::GuiSetText => {
                                    vec![cty_of(BcTy::Widget), cty_of(BcTy::Str)]
                                }
                                BuiltinKind::GuiRun | BuiltinKind::GuiClose => {
                                    vec![cty_of(BcTy::Window)]
                                }
                                _ => vec![],
                            };
                            for (i, t) in targs.iter().enumerate() {
                                let at = texpr_ty(t);
                                let want = param_tys.get(i).cloned().unwrap_or(CTy::Unknown);
                                let cast = match castable(at.clone(), want.clone()) {
                                    Some(c) => c,
                                    None => {
                                        self.err(
                                            "E3001",
                                            args[i].span(),
                                            format!(
                                                "argument {} of `{}`: expected `{}`, found `{}`",
                                                i + 1,
                                                callee,
                                                want.name(),
                                                at.name()
                                            ),
                                        );
                                        false
                                    }
                                };
                                casts.push(cast);
                            }
                            let ty = match kind {
                                BuiltinKind::Sqrt
                                | BuiltinKind::Pow
                                | BuiltinKind::Floor
                                | BuiltinKind::Ceil
                                | BuiltinKind::Round
                                | BuiltinKind::Sin
                                | BuiltinKind::Cos
                                | BuiltinKind::Tan
                                | BuiltinKind::Log
                                | BuiltinKind::MinFloat
                                | BuiltinKind::MaxFloat => cty_of(BcTy::Float),
                                BuiltinKind::MinInt | BuiltinKind::MaxInt => cty_of(BcTy::Int),
                                BuiltinKind::ReadText => cty_of(BcTy::Str),
                                BuiltinKind::WriteText => cty_of(BcTy::Unit),
                                BuiltinKind::FileExists => cty_of(BcTy::Bool),
                                BuiltinKind::GuiWindow => cty_of(BcTy::Window),
                                BuiltinKind::GuiLabel
                                | BuiltinKind::GuiInput
                                | BuiltinKind::GuiButton => cty_of(BcTy::Widget),
                                BuiltinKind::GuiSetText
                                | BuiltinKind::GuiRun
                                | BuiltinKind::GuiClose => cty_of(BcTy::Unit),
                                _ => CTy::Unknown,
                            };
                            return TExpr::Builtin {
                                kind,
                                casts,
                                args: targs,
                                ty,
                            };
                        }
                        self.hint_err(
                            "E3006",
                            *callee_span,
                            format!("unknown function `{}`", callee),
                            "check the spelling, or define it with `fn`".into(),
                        );
                        let mut targs = Vec::new();
                        for a in args {
                            targs.push(self.check_expr(a));
                        }
                        return TExpr::Call {
                            func: 0,
                            casts: vec![],
                            args: targs,
                            ty: CTy::Unknown,
                        };
                    }
                };
                let sig = &self.sigs[fi];
                if args.len() != sig.params.len() {
                    self.hint_err(
                        "E3007",
                        *span,
                        format!(
                            "function `{}` expects {} argument(s), found {}",
                            callee,
                            sig.params.len(),
                            args.len()
                        ),
                        "adjust the call site to match the definition".into(),
                    );
                }
                let mut casts = Vec::new();
                let mut targs = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let t = self.check_expr(a);
                    let at = texpr_ty(&t);
                    // Parameter type: annotated, or the current inferred state.
                    let want = match sig.params.get(i) {
                        Some(Some(ann)) => cty_of(*ann),
                        Some(None) => self.state[fi].params[i].clone(),
                        None => CTy::Unknown,
                    };
                    // Observe the argument type for unannotated parameters.
                    if sig.params.get(i) == Some(&None) && !matches!(at, CTy::Unknown) {
                        let mut conflict = false;
                        let mut prev = at.clone();
                        match &self.observed[fi][i] {
                            Obs::Unobserved => self.observed[fi][i] = Obs::Observed(at.clone()),
                            Obs::Observed(p) => {
                                prev = p.clone();
                                conflict = unify(Some(p.clone()), at.clone()).is_err();
                                if conflict {
                                    self.observed[fi][i] = Obs::Conflicted;
                                }
                            }
                            Obs::Conflicted => {
                                conflict = false; // already reported once
                            }
                        }
                        if conflict {
                            self.hint_err(
                                "E3001",
                                a.span(),
                                format!(
                                    "cannot infer the type of parameter {} of `{}`: conflicting call-site types `{}` and `{}`",
                                    i + 1,
                                    callee,
                                    prev.name(),
                                    at.name()
                                ),
                                "annotate the parameter, e.g. `x: int`".into(),
                            );
                        }
                    }
                    let cast = match castable(at.clone(), want.clone()) {
                        Some(c) => c,
                        None => {
                            self.err(
                                "E3001",
                                a.span(),
                                format!(
                                    "argument {} of `{}`: expected `{}`, found `{}`",
                                    i + 1,
                                    callee,
                                    want.name(),
                                    at.name()
                                ),
                            );
                            false
                        }
                    };
                    casts.push(cast);
                    targs.push(t);
                }
                let ty = match sig.ret {
                    Some(t) => cty_of(t),
                    None => self.state[fi].ret.clone(),
                };
                TExpr::Call {
                    func: fi,
                    casts,
                    args: targs,
                    ty,
                }
            }
            Expr::If {
                cond,
                then,
                els,
                span,
            } => {
                let cond_t = self.check_expr(cond);
                self.require_bool(&texpr_ty(&cond_t), cond.span());
                let then_b = self.check_block(then);
                let els_b = els.as_ref().map(|b| self.check_block(b));
                if els.is_none() {
                    self.hint_err(
                        "E2006",
                        *span,
                        "`if` used as a value needs an `else` branch".into(),
                        "add `else { ... }` or use the statement form".into(),
                    );
                }
                let then_v = block_value(&then_b);
                let els_v = els_b.as_ref().and_then(|b| block_value(b));
                let ty = match (then_v, els_v) {
                    (Some(a), Some(b)) => match unify(Some(a.clone()), b.clone()) {
                        Ok(u) => u,
                        Err(()) => {
                            self.err(
                                "E3001",
                                *span,
                                format!(
                                    "if-expression branches have different types: `{}` and `{}`",
                                    a.name(),
                                    b.name()
                                ),
                            );
                            CTy::Unknown
                        }
                    },
                    (Some(a), None) => {
                        if els.is_some() {
                            self.hint_err(
                                "E3001",
                                *span,
                                format!(
                                    "the `then` branch produces a value of type `{}` but the `else` branch does not",
                                    a.name()
                                ),
                                "make both branches end with a value expression".into(),
                            );
                        }
                        a
                    }
                    (None, Some(b)) => {
                        self.hint_err(
                            "E3001",
                            *span,
                            format!(
                                "the `else` branch produces a value of type `{}` but the `then` branch does not",
                                b.name()
                            ),
                            "make both branches end with a value expression".into(),
                        );
                        b
                    }
                    (None, None) => cty_of(BcTy::Unit),
                };
                TExpr::If {
                    cond: Box::new(cond_t),
                    then: then_b,
                    els: els_b,
                    ty,
                }
            }
        }
    }
}
