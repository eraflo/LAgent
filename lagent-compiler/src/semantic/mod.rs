// SPDX-License-Identifier: Apache-2.0
//! Semantic analysis: name resolution and basic constraint validation.

use crate::parser::ast::{Expr, FnDef, Item, Stmt};
use anyhow::{anyhow, Result};
use std::collections::HashSet;

/// The output of semantic analysis: the original items plus (future) type info.
pub struct TypedAst {
    pub items: Vec<Item>,
}

/// Perform semantic analysis: name resolution and basic constraint validation.
///
/// # Errors
///
/// Returns an error if an identifier is used before it is declared.
pub fn analyze(items: Vec<Item>) -> Result<TypedAst> {
    for item in &items {
        match item {
            Item::FnDef(f) => check_fn(f)?,
            Item::KernelDef(_) | Item::TypeAlias(_) => {}
        }
    }
    Ok(TypedAst { items })
}

fn check_fn(f: &FnDef) -> Result<()> {
    // Seed scope with parameter names.
    let mut scope: HashSet<String> = f.params.iter().map(|p| p.name.clone()).collect();
    check_block(&f.body, &mut scope)
}

fn check_block(block: &[Stmt], scope: &mut HashSet<String>) -> Result<()> {
    for stmt in block {
        match stmt {
            Stmt::Let(name, _ty, expr) => {
                check_expr(expr, scope)?;
                scope.insert(name.clone());
            }
            Stmt::Return(expr) | Stmt::Expr(expr) => check_expr(expr, scope)?,
            Stmt::Branch(b) => {
                check_expr(&Expr::Ident(b.var.clone()), scope)?;
                for case in &b.cases {
                    let mut inner = scope.clone();
                    check_block(&case.body, &mut inner)?;
                }
                if let Some(default) = &b.default {
                    let mut inner = scope.clone();
                    check_block(default, &mut inner)?;
                }
            }
        }
    }
    Ok(())
}

fn check_expr(expr: &Expr, scope: &HashSet<String>) -> Result<()> {
    match expr {
        Expr::Ident(name) => {
            // Built-in names are always in scope.
            if is_builtin(name) || scope.contains(name.as_str()) {
                Ok(())
            } else {
                Err(anyhow!("undefined variable: `{name}`"))
            }
        }
        Expr::Call(_, args) => {
            for arg in args {
                check_expr(arg, scope)?;
            }
            Ok(())
        }
        Expr::BinOp(lhs, _, rhs) => {
            check_expr(lhs, scope)?;
            check_expr(rhs, scope)
        }
        Expr::StringLit(_) | Expr::IntLit(_) | Expr::FloatLit(_) => Ok(()),
    }
}

fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "println"
            | "ctx_alloc"
            | "ctx_free"
            | "ctx_append"
            | "ctx_resize"
            | "observe"
            | "reason"
            | "act"
            | "verify"
            | "infer"
            // built-in branch subjects
            | "intent"
    )
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn compile_src(src: &str) -> Result<TypedAst> {
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        analyze(items)
    }

    #[test]
    fn accepts_hello_la() {
        let src = r#"
fn main() {
    let ctx = ctx_alloc(512);
    ctx_append(ctx, "Hello, L-Agent!");
    println("Hello, L-Agent!");
    ctx_free(ctx);
}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn rejects_undefined_variable() {
        let src = "fn main() { println(x); }";
        assert!(compile_src(src).is_err());
    }

    #[test]
    fn accepts_parameter_in_body() {
        let src = "fn greet(msg: str) { println(msg); }";
        assert!(compile_src(src).is_ok());
    }
}
