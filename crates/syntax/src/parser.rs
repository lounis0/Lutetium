//! Recursive-descent parser for the Lutetium 0.1 grammar.
//!
//! Grammar (informal EBNF) — the authoritative version lives in
//! `docs/LANGUAGE_SPEC.md`:
//!
//! ```ebnf
//! module      = { newline , function } ;
//! function    = "fn" , IDENT , "(" , [ params ] , ")" , [ "->" , type ] , block ;
//! params      = IDENT , ":" , type , { "," , IDENT , ":" , type } ;
//! type        = "int" | "float" | "bool" | "str" | "unit" ;
//! block       = "{" , { statement } , "}" ;
//! statement   = "let" [ "mut" ] IDENT [ ":" type ] "=" expr
//!             | "if" expr block [ "else" ( if_stmt | block ) ]
//!             | "while" expr block
//!             | "return" [ expr ]
//!             | "break" | "continue"
//!             | expr [ assign_op expr ] ;
//! ```

use crate::ast::*;
use crate::lexer::{Tok, Token};
use lu_diagnostics::{Diagnostic, Span};

pub fn parse(toks: Vec<Token>) -> Result<Module, Vec<Diagnostic>> {
    let mut p = Parser {
        toks,
        pos: 0,
        diags: Vec::new(),
    };
    let module = p.parse_module();
    if p.diags.is_empty() {
        Ok(module)
    } else {
        Err(p.diags)
    }
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    diags: Vec<Diagnostic>,
}

impl Parser {
    fn peek(&self) -> &Tok {
        let i = self.pos.min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn peek_span(&self) -> Span {
        let i = self.pos.min(self.toks.len() - 1);
        self.toks[i].span
    }

    fn bump(&mut self) -> Token {
        let i = self.pos.min(self.toks.len() - 1);
        let t = self.toks[i].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at(&self, t: &Tok) -> bool {
        self.peek() == t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.at(t) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn err_unexpected(&mut self, what: &str) {
        let tok = self.peek().clone();
        let span = self.peek_span();
        self.diags.push(Diagnostic::error(
            "E2001",
            span,
            format!("expected {}, found {}", what, tok.describe()),
        ));
    }

    fn expect(&mut self, t: &Tok, what: &str) -> Result<Token, ()> {
        if self.at(t) {
            Ok(self.bump())
        } else {
            self.err_unexpected(what);
            Err(())
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<(String, Span), ()> {
        if let Tok::Ident(name) = self.peek().clone() {
            let span = self.peek_span();
            self.bump();
            Ok((name, span))
        } else {
            self.err_unexpected(what);
            Err(())
        }
    }

    fn skip_sep(&mut self) {
        while self.at(&Tok::Newline) || self.at(&Tok::Semi) {
            self.bump();
        }
    }

    fn sync(&mut self) {
        while !self.at(&Tok::Newline)
            && !self.at(&Tok::Semi)
            && !self.at(&Tok::RBrace)
            && !self.at(&Tok::Eof)
        {
            self.bump();
        }
    }

    fn parse_module(&mut self) -> Module {
        let mut funcs = Vec::new();
        let mut top = Vec::new();
        loop {
            self.skip_sep();
            if self.at(&Tok::Eof) {
                break;
            }
            if self.at(&Tok::KwFn) {
                match self.parse_fn() {
                    Ok(f) => funcs.push(f),
                    Err(()) => {
                        // recover: skip to the next top-level `fn`
                        while !self.at(&Tok::Eof) && !self.at(&Tok::KwFn) {
                            self.bump();
                        }
                    }
                }
            } else {
                let before = self.diags.len();
                let before_pos = self.pos;
                if let Ok(Some(s)) = self.parse_stmt() {
                    top.push(s);
                }
                if self.diags.len() > before {
                    self.sync();
                }
                if self.pos == before_pos {
                    // Recovery made no progress (e.g. a stray `}` at top
                    // level, where `sync` stops): force one token forward so
                    // the module loop cannot spin forever.
                    self.bump();
                }
            }
        }
        Module { funcs, top }
    }

    fn parse_fn(&mut self) -> Result<FnDecl, ()> {
        let start = self.bump().span; // `fn`
        let (name, name_span) = self.expect_ident("function name")?;
        self.expect(&Tok::LParen, "'('")?;
        let mut params = Vec::new();
        self.skip_sep();
        if !self.at(&Tok::RParen) {
            loop {
                self.skip_sep();
                let (pname, pspan) = self.expect_ident("parameter name")?;
                let ty = if self.eat(&Tok::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                params.push(Param {
                    name: pname,
                    name_span: pspan,
                    ty,
                });
                self.skip_sep();
                if self.eat(&Tok::Comma) {
                    continue;
                }
                break;
            }
        }
        self.expect(&Tok::RParen, "')'")?;
        self.skip_sep();
        let ret = if self.eat(&Tok::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.skip_sep();
        let body = self.parse_block()?;
        let span = start.merge(body.span);
        Ok(FnDecl {
            name,
            name_span,
            params,
            ret,
            body,
            span,
        })
    }

    fn parse_type(&mut self) -> Result<TypeAnn, ()> {
        let span = self.peek_span();
        let (name, _span) = self.expect_ident("a type name")?;
        let ty = match name.as_str() {
            "int" => TypeKind::Int,
            "float" => TypeKind::Float,
            "bool" => TypeKind::Bool,
            "str" => TypeKind::Str,
            "unit" => TypeKind::Unit,
            "window" => TypeKind::Window,
            "widget" => TypeKind::Widget,
            other => TypeKind::Named(other.to_string()),
        };
        Ok(TypeAnn { ty, span })
    }

    fn parse_block(&mut self) -> Result<Block, ()> {
        let lb = self.expect(&Tok::LBrace, "'{'")?;
        let mut stmts = Vec::new();
        loop {
            self.skip_sep();
            if self.at(&Tok::RBrace) {
                let rb = self.bump();
                return Ok(Block {
                    stmts,
                    span: lb.span.merge(rb.span),
                });
            }
            if self.at(&Tok::Eof) {
                self.diags.push(Diagnostic::error(
                    "E2001",
                    self.peek_span(),
                    "expected '}', found end of file",
                ));
                return Err(());
            }
            let before = self.diags.len();
            if let Ok(Some(s)) = self.parse_stmt() {
                stmts.push(s);
            }
            if self.diags.len() > before {
                self.sync();
            }
        }
    }

    fn parse_stmt(&mut self) -> Result<Option<Stmt>, ()> {
        let start = self.peek_span();
        match self.peek().clone() {
            Tok::KwLet => {
                self.bump();
                let mutable = self.eat(&Tok::KwMut);
                let (name, name_span) = self.expect_ident("variable name")?;
                let ty = if self.eat(&Tok::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.expect(&Tok::Eq, "'=' (variables need an initial value)")?;
                let init = self.parse_expr()?;
                let span = start.merge(init.span());
                Ok(Some(Stmt::Let {
                    mutable,
                    name,
                    name_span,
                    ty,
                    init,
                    span,
                }))
            }
            Tok::KwIf => self.parse_if().map(Some),
            Tok::KwWhile => {
                self.bump();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                let span = start.merge(body.span);
                Ok(Some(Stmt::While { cond, body, span }))
            }
            Tok::KwFor => {
                self.bump();
                let (var, var_span) = self.expect_ident("loop variable name")?;
                self.expect(&Tok::KwIn, "'in' after the loop variable")?;
                let start = self.parse_add()?;
                if self.at(&Tok::DotDotEq) || self.at(&Tok::DotDot) {
                    let inclusive = if self.eat(&Tok::DotDotEq) {
                        true
                    } else {
                        self.eat(&Tok::DotDot);
                        false
                    };
                    let end = self.parse_add()?;
                    let body = self.parse_block()?;
                    let span = start.span().merge(body.span);
                    Ok(Some(Stmt::For {
                        var,
                        var_span,
                        start,
                        end,
                        inclusive,
                        body,
                        span,
                    }))
                } else if self.at(&Tok::LBrace) {
                    // `for x in lst { ... }` — list iteration.
                    let body = self.parse_block()?;
                    let span = start.span().merge(body.span);
                    Ok(Some(Stmt::ForIn {
                        var,
                        var_span,
                        iterable: start,
                        body,
                        span,
                    }))
                } else {
                    self.err_unexpected("'..', '..=' or '{' after the loop expression");
                    Err(())
                }
            }
            Tok::KwReturn => {
                self.bump();
                let value = if self.at(&Tok::Newline)
                    || self.at(&Tok::Semi)
                    || self.at(&Tok::RBrace)
                    || self.at(&Tok::Eof)
                {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                let span = match &value {
                    Some(e) => start.merge(e.span()),
                    None => start,
                };
                Ok(Some(Stmt::Return { value, span }))
            }
            Tok::KwBreak => {
                self.bump();
                Ok(Some(Stmt::Break { span: start }))
            }
            Tok::KwContinue => {
                self.bump();
                Ok(Some(Stmt::Continue { span: start }))
            }
            _ => {
                let e = self.parse_expr()?;
                let aop = match self.peek() {
                    Tok::Eq => Some(AssignOp::Set),
                    Tok::PlusEq => Some(AssignOp::Add),
                    Tok::MinusEq => Some(AssignOp::Sub),
                    Tok::StarEq => Some(AssignOp::Mul),
                    Tok::SlashEq => Some(AssignOp::Div),
                    _ => None,
                };
                if let Some(op) = aop {
                    self.bump();
                    let (target, target_span) = match &e {
                        Expr::Ident(n, s) => (n.clone(), *s),
                        _ => {
                            self.diags.push(Diagnostic::error(
                                "E2001",
                                e.span(),
                                "invalid assignment target: expected a variable name",
                            ));
                            return Err(());
                        }
                    };
                    let value = self.parse_expr()?;
                    let span = start.merge(value.span());
                    Ok(Some(Stmt::Assign {
                        target,
                        target_span,
                        op,
                        value,
                        span,
                    }))
                } else {
                    Ok(Some(Stmt::Expr(e)))
                }
            }
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, ()> {
        let start = self.bump().span; // `if`
        let cond = self.parse_expr()?;
        let then = self.parse_block()?;
        self.skip_sep();
        let els = if self.eat(&Tok::KwElse) {
            self.skip_sep();
            if self.at(&Tok::KwIf) {
                let inner = self.parse_if()?;
                let span = inner.span();
                Some(Block {
                    stmts: vec![inner],
                    span,
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        let end = els.as_ref().map(|b| b.span).unwrap_or(then.span);
        let span = start.merge(end);
        Ok(Stmt::If {
            cond,
            then,
            els,
            span,
        })
    }

    /// Expression-position `if`: `result = if c { a } else { b }`.
    fn parse_if_expr(&mut self) -> Result<Expr, ()> {
        let start = self.bump().span; // `if`
        let cond = self.parse_expr()?;
        let then = self.parse_block()?;
        self.skip_sep();
        let els = if self.eat(&Tok::KwElse) {
            self.skip_sep();
            if self.at(&Tok::KwIf) {
                let inner = self.parse_if_expr()?;
                let span = inner.span();
                Some(Block {
                    stmts: vec![Stmt::Expr(inner)],
                    span,
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        let end = els.as_ref().map(|b| b.span).unwrap_or(then.span);
        let span = start.merge(end);
        Ok(Expr::If {
            cond: Box::new(cond),
            then,
            els,
            span,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_and()?;
        while self.at(&Tok::PipePipe) {
            self.bump();
            let rhs = self.parse_and()?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_cmp()?;
        while self.at(&Tok::AmpAmp) {
            self.bump();
            let rhs = self.parse_cmp()?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, ()> {
        let lhs = self.parse_add()?;
        let op = match self.peek() {
            Tok::EqEq => Some(BinOp::Eq),
            Tok::NotEq => Some(BinOp::Ne),
            Tok::Lt => Some(BinOp::Lt),
            Tok::Le => Some(BinOp::Le),
            Tok::Gt => Some(BinOp::Gt),
            Tok::Ge => Some(BinOp::Ge),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let rhs = self.parse_add()?;
            let span = lhs.span().merge(rhs.span());
            Ok(Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            })
        } else {
            Ok(lhs)
        }
    }

    fn parse_add(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_unary()?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ()> {
        let span = self.peek_span();
        if self.eat(&Tok::Minus) {
            let e = self.parse_unary()?;
            let span = span.merge(e.span());
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(e),
                span,
            });
        }
        if self.eat(&Tok::Bang) {
            let e = self.parse_unary()?;
            let span = span.merge(e.span());
            return Ok(Expr::Unary {
                op: UnOp::Not,
                expr: Box::new(e),
                span,
            });
        }
        let e = self.parse_primary()?;
        self.parse_postfix(e)
    }

    /// Postfix operations: indexing `e[i]` and method calls `e.name(args)`.
    fn parse_postfix(&mut self, mut e: Expr) -> Result<Expr, ()> {
        loop {
            if self.at(&Tok::LBracket) {
                self.bump();
                self.skip_sep();
                let idx = self.parse_expr()?;
                self.skip_sep();
                let rb = self.expect(&Tok::RBracket, "']'")?;
                let span = e.span().merge(rb.span);
                e = Expr::Index {
                    base: Box::new(e),
                    idx: Box::new(idx),
                    span,
                };
                continue;
            }
            if self.at(&Tok::Dot) {
                self.bump();
                let (name, name_span) = self.expect_ident("field or method name after '.'")?;
                if self.at(&Tok::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    self.skip_sep();
                    if !self.at(&Tok::RParen) {
                        loop {
                            self.skip_sep();
                            args.push(self.parse_expr()?);
                            self.skip_sep();
                            if self.eat(&Tok::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    let rparen = self.expect(&Tok::RParen, "')' after arguments")?;
                    let span = e.span().merge(rparen.span);
                    e = Expr::Method {
                        recv: Box::new(e),
                        name,
                        name_span,
                        args,
                        span,
                    };
                } else {
                    let span = e.span().merge(name_span);
                    e = Expr::Field {
                        recv: Box::new(e),
                        name,
                        name_span,
                        span,
                    };
                }
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, ()> {
        let span = self.peek_span();
        match self.peek().clone() {
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Int(v, span))
            }
            Tok::Float(v) => {
                self.bump();
                Ok(Expr::Float(v, span))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Str(s, span))
            }
            Tok::KwTrue => {
                self.bump();
                Ok(Expr::Bool(true, span))
            }
            Tok::KwFalse => {
                self.bump();
                Ok(Expr::Bool(false, span))
            }
            Tok::Ident(name) => {
                self.bump();
                let e = if self.at(&Tok::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    self.skip_sep();
                    if !self.at(&Tok::RParen) {
                        loop {
                            self.skip_sep();
                            args.push(self.parse_expr()?);
                            self.skip_sep();
                            if self.eat(&Tok::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    let rparen = self.expect(&Tok::RParen, "')' after arguments")?;
                    Expr::Call {
                        callee: name.clone(),
                        callee_span: span,
                        args,
                        span: span.merge(rparen.span),
                    }
                } else {
                    Expr::Ident(name, span)
                };
                Ok(e)
            }
            Tok::LParen => {
                self.bump();
                self.skip_sep();
                let e = self.parse_expr()?;
                self.skip_sep();
                self.expect(&Tok::RParen, "')'")?;
                Ok(e)
            }
            Tok::LBracket => {
                self.bump();
                let mut elems = Vec::new();
                self.skip_sep();
                if !self.at(&Tok::RBracket) {
                    loop {
                        self.skip_sep();
                        elems.push(self.parse_expr()?);
                        self.skip_sep();
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        break;
                    }
                }
                let rb = self.expect(&Tok::RBracket, "']'")?;
                Ok(Expr::List(elems, span.merge(rb.span)))
            }
            Tok::LBrace => {
                // In expression position a `{` always starts a map literal
                // (`{name: "x"}`); blocks only appear after fn/if/while/for.
                self.bump();
                let mut entries = Vec::new();
                self.skip_sep();
                if !self.at(&Tok::RBrace) {
                    loop {
                        self.skip_sep();
                        let (key, key_span) =
                            self.expect_ident("a map key (identifier) after '{'")?;
                        self.expect(&Tok::Colon, "':' after the map key")?;
                        let value = self.parse_expr()?;
                        entries.push((key, key_span, value));
                        self.skip_sep();
                        if self.eat(&Tok::Comma) {
                            if self.at(&Tok::RBrace) {
                                break; // trailing comma
                            }
                            continue;
                        }
                        break;
                    }
                }
                let rb = self.expect(&Tok::RBrace, "'}' to close the map literal")?;
                Ok(Expr::Map(entries, span.merge(rb.span)))
            }
            Tok::KwIf => self.parse_if_expr(),
            _ => {
                self.diags.push(Diagnostic::error(
                    "E2002",
                    span,
                    format!("expected an expression, found {}", self.peek().describe()),
                ));
                Err(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_ok(src: &str) -> Module {
        let (toks, diags) = lex(src);
        assert!(diags.is_empty(), "lex diagnostics: {:?}", diags);
        match parse(toks) {
            Ok(m) => m,
            Err(e) => panic!("parse failed: {:?}", e),
        }
    }

    #[test]
    fn parses_fibonacci() {
        let m = parse_ok(
            "fn fibonacci(n: int) -> int {\n    if n <= 1 {\n        return n\n    }\n    return fibonacci(n - 1) + fibonacci(n - 2)\n}\n\nfn main() {\n    print(fibonacci(10))\n}\n",
        );
        assert_eq!(m.funcs.len(), 2);
        let fib = &m.funcs[0];
        assert_eq!(fib.name, "fibonacci");
        assert_eq!(fib.params.len(), 1);
        assert_eq!(fib.params[0].ty.as_ref().unwrap().ty, TypeKind::Int);
        assert_eq!(fib.ret.as_ref().unwrap().ty, TypeKind::Int);
    }

    #[test]
    fn parses_control_flow() {
        let m = parse_ok(
            "fn main() {\n    let mut i = 0\n    while i < 10 {\n        i += 1\n        if i == 5 {\n            continue\n        }\n        if i == 8 {\n            break\n        }\n    }\n}\n",
        );
        let body = &m.funcs[0].body.stmts;
        assert!(matches!(body[0], Stmt::Let { mutable: true, .. }));
        assert!(matches!(body[1], Stmt::While { .. }));
    }

    #[test]
    fn for_in_parses() {
        let m = parse_ok("fn main() {\n    for x in lst {\n        print(x)\n    }\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::ForIn {
                var,
                iterable,
                body,
                ..
            } => {
                assert_eq!(var, "x");
                assert!(matches!(iterable, Expr::Ident(name, _) if name == "lst"));
                assert_eq!(body.stmts.len(), 1);
            }
            other => panic!("unexpected stmt {:?}", other),
        }
    }

    #[test]
    fn for_in_missing_brace_is_error() {
        // A for header without a range or a body block is a syntax error.
        let (toks, _) = lex("fn main() {\n    for x in lst\n}\n");
        assert!(parse(toks).is_err());
    }

    #[test]
    fn parses_else_if() {
        let m = parse_ok(
            "fn f(x: int) -> int {\n    if x < 0 {\n        return 0 - x\n    } else if x == 0 {\n        return 1\n    } else {\n        return x\n    }\n}\n\nfn main() { }\n",
        );
        assert_eq!(m.funcs[0].body.stmts.len(), 1);
    }

    #[test]
    fn parses_precedence() {
        let m = parse_ok("fn main() {\n    print(1 + 2 * 3 - 4 / 2 % 3)\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::Expr(Expr::Call { args, .. }) => match &args[0] {
                // top level: (1 + (2*3)) - ((4/2) % 3)
                Expr::Binary {
                    op: BinOp::Sub,
                    lhs,
                    rhs,
                    ..
                } => {
                    assert!(matches!(**lhs, Expr::Binary { op: BinOp::Add, .. }));
                    assert!(matches!(**rhs, Expr::Binary { op: BinOp::Mod, .. }));
                }
                other => panic!("unexpected expr {:?}", other),
            },
            other => panic!("unexpected stmt {:?}", other),
        }
    }

    #[test]
    fn missing_brace_is_error() {
        let (toks, _) = lex("fn main() {\n    print(1)\n");
        assert!(parse(toks).is_err());
    }

    #[test]
    fn method_call_parses() {
        // `recv.name(args)` is a supported feature (built-in methods on
        // lists/strings); the old 0.1 parser rejected it with E2005.
        let m = parse_ok("fn main() {\n    let x = \"a\".len()\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::Let { init, .. } => {
                assert!(matches!(init, Expr::Method { name, .. } if name == "len"));
            }
            other => panic!("unexpected stmt {:?}", other),
        }
    }

    #[test]
    fn map_literal_parses() {
        let m = parse_ok("fn main() {\n    let u = {name: \"Ada\", score: 95}\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::Let { init, .. } => match init {
                Expr::Map(entries, _) => {
                    assert_eq!(entries.len(), 2);
                    assert_eq!(entries[0].0, "name");
                    assert!(matches!(&entries[0].2, Expr::Str(s, _) if s == "Ada"));
                    assert_eq!(entries[1].0, "score");
                    assert!(matches!(entries[1].2, Expr::Int(95, _)));
                }
                other => panic!("unexpected expr {:?}", other),
            },
            other => panic!("unexpected stmt {:?}", other),
        }
    }

    #[test]
    fn empty_map_literal_parses() {
        let m = parse_ok("fn main() {\n    let e = {}\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::Let { init, .. } => {
                assert!(matches!(init, Expr::Map(entries, _) if entries.is_empty()));
            }
            other => panic!("unexpected stmt {:?}", other),
        }
    }

    #[test]
    fn field_access_parses() {
        let m = parse_ok("fn main() {\n    print(u.name)\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::Expr(Expr::Call { args, .. }) => {
                assert!(matches!(args[0], Expr::Field { ref name, .. } if name == "name"));
            }
            other => panic!("unexpected stmt {:?}", other),
        }
    }

    #[test]
    fn map_key_missing_colon_is_error() {
        let (toks, _) = lex("fn main() {\n    let u = {name \"Ada\"}\n}\n");
        assert!(parse(toks).is_err());
    }

    #[test]
    fn map_non_ident_key_is_error() {
        let (toks, _) = lex("fn main() {\n    let u = {\"name\": 1}\n}\n");
        assert!(parse(toks).is_err());
    }

    #[test]
    fn annotation_on_let() {
        let m = parse_ok("fn main() {\n    let x: float = 1.5\n}\n");
        match &m.funcs[0].body.stmts[0] {
            Stmt::Let {
                ty: Some(t),
                mutable: false,
                ..
            } => {
                assert_eq!(t.ty, TypeKind::Float)
            }
            other => panic!("unexpected {:?}", other),
        }
    }
}
