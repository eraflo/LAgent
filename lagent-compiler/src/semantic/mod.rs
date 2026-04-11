// SPDX-License-Identifier: Apache-2.0
//! Semantic analysis: name resolution and basic constraint validation.

use crate::parser::ast::{Expr, FnDef, Item, KernelDef, Stmt, TypeExpr};
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};

/// The output of semantic analysis: the original items plus the resolved type
/// environment mapping semantic type names to their label sets.
pub struct TypedAst {
    pub items: Vec<Item>,
    /// Maps `type Name = semantic(...)` aliases to their label lists.
    /// Used by codegen to populate `InferClassify` with the correct labels.
    pub type_env: HashMap<String, Vec<String>>,
}

/// Perform semantic analysis: name resolution and basic constraint validation.
///
/// # Errors
///
/// Returns an error if an identifier is used before it is declared.
pub fn analyze(items: Vec<Item>) -> Result<TypedAst> {
    // Build type environment from TypeAlias items.
    let mut type_env: HashMap<String, Vec<String>> = HashMap::new();
    for item in &items {
        if let Item::TypeAlias(ta) = item {
            if let TypeExpr::Semantic(labels) = &ta.def {
                type_env.insert(ta.name.clone(), labels.clone());
            }
        }
    }

    // Name-check all fn and kernel definitions.
    for item in &items {
        match item {
            Item::FnDef(f) => check_fn(f)?,
            Item::KernelDef(k) => check_kernel(k)?,
            Item::TypeAlias(_) => {}
        }
    }

    Ok(TypedAst { items, type_env })
}

fn check_fn(f: &FnDef) -> Result<()> {
    let mut scope: HashSet<String> = f.params.iter().map(|p| p.name.clone()).collect();
    check_block(&f.body, &mut scope)
}

fn check_kernel(k: &KernelDef) -> Result<()> {
    let mut scope: HashSet<String> = k.params.iter().map(|p| p.name.clone()).collect();
    check_block(&k.body, &mut scope)
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
            Stmt::Interruptible(block) => {
                let mut inner = scope.clone();
                check_block(block, &mut inner)?;
            }
        }
    }
    Ok(())
}

fn check_expr(expr: &Expr, scope: &HashSet<String>) -> Result<()> {
    match expr {
        Expr::Ident(name) => {
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
            | "ctx_compress"
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

    #[test]
    fn builds_type_env_from_aliases() {
        let src = r#"type Emotion = semantic("joie", "colère", "neutre"); fn main() {}"#;
        let ast = compile_src(src).unwrap();
        let labels = ast.type_env.get("Emotion").unwrap();
        assert_eq!(labels.len(), 3);
    }

    #[test]
    fn accepts_kernel_def() {
        let src = r#"
kernel Foo(x: str) -> str {
    observe(x);
    reason("test");
    let r: str = infer(x);
    verify(r != "");
    return r;
}
fn main() {}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_interruptible_block() {
        let src = r#"
fn main() {
    interruptible {
        println("safe point");
    }
}
"#;
        assert!(compile_src(src).is_ok());
    }
}
