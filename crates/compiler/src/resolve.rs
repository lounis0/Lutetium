//! Name resolution: function table construction, duplicate detection,
//! annotation-to-type conversion and entry-point validation.

use lu_diagnostics::{Diagnostic, Span};
use lu_syntax::ast::{Module, TypeAnn, TypeKind};
use lu_vm::bytecode::BcTy;

/// Resolved signature of a top-level function.
#[derive(Debug, Clone)]
pub struct FnInfo {
    pub name: String,
    /// `None` = parameter type is inferred from call sites.
    pub params: Vec<Option<BcTy>>,
    /// `None` means the return type is inferred by the type checker.
    pub ret: Option<BcTy>,
    pub span: Span,
}

pub fn ann_to_opt(ann: &TypeAnn, diags: &mut Vec<Diagnostic>) -> Option<BcTy> {
    match &ann.ty {
        TypeKind::Named(n) => {
            diags.push(
                Diagnostic::error("E3004", ann.span, format!("unknown type `{}`", n))
                    .with_hint("the built-in types are int, float, bool, str and unit"),
            );
            None
        }
        other => Some(match other {
            TypeKind::Int => BcTy::Int,
            TypeKind::Float => BcTy::Float,
            TypeKind::Bool => BcTy::Bool,
            TypeKind::Str => BcTy::Str,
            TypeKind::Unit => BcTy::Unit,
            TypeKind::Window => BcTy::Window,
            TypeKind::Widget => BcTy::Widget,
            TypeKind::Named(_) => unreachable!(),
        }),
    }
}

/// Build the function table and validate global properties.
pub fn resolve(module: &Module) -> Result<Vec<FnInfo>, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let mut infos: Vec<FnInfo> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for f in &module.funcs {
        if f.name == "print" {
            diags.push(
                Diagnostic::error(
                    "E2101",
                    f.name_span,
                    "cannot define a function named `print` (it is a built-in)",
                )
                .with_hint("choose a different name for your function"),
            );
            continue;
        }
        if seen.contains(&f.name.as_str()) {
            diags.push(
                Diagnostic::error(
                    "E2101",
                    f.name_span,
                    format!("function `{}` is already defined", f.name),
                )
                .with_hint("remove or rename one of the definitions"),
            );
            continue;
        }
        seen.push(&f.name);

        let params = f
            .params
            .iter()
            .map(|p| p.ty.as_ref().and_then(|a| ann_to_opt(a, &mut diags)))
            .collect();
        let ret = f.ret.as_ref().and_then(|a| ann_to_opt(a, &mut diags));
        infos.push(FnInfo {
            name: f.name.clone(),
            params,
            ret,
            span: f.name_span,
        });
    }

    match infos.iter().position(|i| i.name == "main") {
        None => {
            // `main` is optional when the module has executable top-level
            // statements (implicit entry point).
            if module.top.is_empty() {
                let span = module
                    .funcs
                    .last()
                    .map(|f| f.span)
                    .unwrap_or(Span::new(0, 0));
                diags.push(
                    Diagnostic::error("E2102", span, "no `main` function")
                        .with_hint(
                            "add an entry point: `fn main() { ... }`, or put executable statements at the top level",
                        ),
                );
            }
        }
        Some(i) => {
            let main = &infos[i];
            if !main.params.is_empty() {
                diags.push(
                    Diagnostic::error("E3007", main.span, "`main` must take no parameters")
                        .with_hint("remove the parameters from `fn main`"),
                );
            }
            if let Some(ret) = main.ret {
                if ret != BcTy::Unit {
                    diags.push(
                        Diagnostic::error(
                            "E3001",
                            main.span,
                            format!(
                                "`main` must return `unit`, but it is declared to return `{}`",
                                ret.name()
                            ),
                        )
                        .with_hint("remove the return type annotation or return `unit`"),
                    );
                }
            }
        }
    }

    if diags.is_empty() {
        Ok(infos)
    } else {
        Err(diags)
    }
}
