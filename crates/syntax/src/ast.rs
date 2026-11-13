//! Abstract syntax tree for the Lutetium 0.1 language subset.

use lu_diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Int,
    Float,
    Bool,
    Str,
    Unit,
    /// Opaque native GUI handles (valid only in native GUI programs).
    Window,
    Widget,
    /// Any other identifier; rejected by the resolver with E3004.
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnn {
    pub ty: TypeKind,
    pub span: Span,
}

impl TypeAnn {
    pub fn name(&self) -> &'static str {
        match &self.ty {
            TypeKind::Int => "int",
            TypeKind::Float => "float",
            TypeKind::Bool => "bool",
            TypeKind::Str => "str",
            TypeKind::Unit => "unit",
            TypeKind::Window => "window",
            TypeKind::Widget => "widget",
            TypeKind::Named(_) => "<unknown>",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub funcs: Vec<FnDecl>,
    /// Executable statements at the top level of the file (implicit entry).
    pub top: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub ret: Option<TypeAnn>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub name_span: Span,
    /// `None` = type inferred from call sites.
    pub ty: Option<TypeAnn>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
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
    And,
    Or,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64, Span),
    Float(f64, Span),
    Bool(bool, Span),
    Str(String, Span),
    Ident(String, Span),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        span: Span,
    },
    Call {
        callee: String,
        callee_span: Span,
        args: Vec<Expr>,
        span: Span,
    },
    /// Expression-position `if` (branches yield their last expression's value)
    If {
        cond: Box<Expr>,
        then: Block,
        els: Option<Block>,
        span: Span,
    },
    /// List literal `[a, b, c]`
    List(Vec<Expr>, Span),
    /// Map literal `{name: "x", score: 42}`; keys are identifiers read as
    /// string keys, `Span` is each key's span.
    Map(Vec<(String, Span, Expr)>, Span),
    /// Field access `recv.name` (maps only, in 0.4)
    Field {
        recv: Box<Expr>,
        name: String,
        name_span: Span,
        span: Span,
    },
    /// Indexing `base[idx]`
    Index {
        base: Box<Expr>,
        idx: Box<Expr>,
        span: Span,
    },
    /// Method call `recv.name(args)` (built-in methods on lists/strings)
    Method {
        recv: Box<Expr>,
        name: String,
        name_span: Span,
        args: Vec<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int(_, s)
            | Expr::Float(_, s)
            | Expr::Bool(_, s)
            | Expr::Str(_, s)
            | Expr::Ident(_, s) => *s,
            Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::If { span, .. }
            | Expr::Index { span, .. }
            | Expr::Method { span, .. }
            | Expr::Field { span, .. } => *span,
            Expr::List(_, s) => *s,
            Expr::Map(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: String,
        name_span: Span,
        ty: Option<TypeAnn>,
        init: Expr,
        span: Span,
    },
    Expr(Expr),
    Assign {
        target: String,
        target_span: Span,
        op: AssignOp,
        value: Expr,
        span: Span,
    },
    If {
        cond: Expr,
        then: Block,
        els: Option<Block>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    For {
        var: String,
        var_span: Span,
        start: Expr,
        end: Expr,
        inclusive: bool,
        body: Block,
        span: Span,
    },
    /// `for x in lst { ... }` — iterate a list's elements in order.
    ForIn {
        var: String,
        var_span: Span,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::ForIn { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span } => *span,
            Stmt::Expr(e) => e.span(),
        }
    }
}
