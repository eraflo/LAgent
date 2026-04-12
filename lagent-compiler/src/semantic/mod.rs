// SPDX-License-Identifier: Apache-2.0
//! Semantic analysis: name resolution and basic constraint validation.

use crate::parser::ast::{
    ConstraintDef, Expr, FnDef, Item, KernelDef, SkillDef, SpellDef, Stmt, TypeExpr,
};
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};

/// The output of semantic analysis: the original items plus the resolved type
/// environment, oracle names, and lore table.
pub struct TypedAst {
    pub items: Vec<Item>,
    /// Maps `type Name = semantic(...)` aliases to their label lists.
    /// Used by codegen to populate `InferClassify` with the correct labels.
    pub type_env: HashMap<String, Vec<String>>,
    /// Names of declared `oracle` callables, for oracle-call codegen dispatch.
    pub oracle_names: Vec<String>,
    /// Lore name → text, passed to codegen for `StoreLore` emission.
    pub lore_table: HashMap<String, String>,
}

/// Perform semantic analysis: name resolution and basic constraint validation.
///
/// # Errors
///
/// Returns an error if an identifier is used before it is declared.
pub fn analyze(items: Vec<Item>) -> Result<TypedAst> {
    // ── Build global environment from declarations ─────────────────────────
    let mut type_env: HashMap<String, Vec<String>> = HashMap::new();
    let mut oracle_names: Vec<String> = Vec::new();
    let mut lore_table: HashMap<String, String> = HashMap::new();
    // Names that are callable: fn/kernel/skill/spell/oracle.
    let mut callable_names: HashSet<String> = HashSet::new();
    // Names that are memory slots (valid as identifiers).
    let mut memory_names: HashSet<String> = HashSet::new();
    // Names that are lore keys (valid as identifiers).
    let mut lore_names: HashSet<String> = HashSet::new();

    for item in &items {
        match item {
            Item::TypeAlias(ta) => {
                if let TypeExpr::Semantic(labels) = &ta.def {
                    type_env.insert(ta.name.clone(), labels.clone());
                }
            }
            Item::FnDef(f) => {
                callable_names.insert(f.name.clone());
            }
            Item::KernelDef(k) => {
                callable_names.insert(k.name.clone());
            }
            Item::SkillDef(s) => {
                callable_names.insert(s.name.clone());
            }
            Item::SpellDef(s) => {
                callable_names.insert(s.name.clone());
            }
            Item::OracleDecl(o) => {
                callable_names.insert(o.name.clone());
                oracle_names.push(o.name.clone());
            }
            Item::MemoryDecl(m) => {
                memory_names.insert(m.name.clone());
            }
            Item::LoreDecl(l) => {
                lore_names.insert(l.name.clone());
                lore_table.insert(l.name.clone(), l.value.clone());
            }
            Item::SoulDef(_) | Item::ConstraintDef(_) | Item::UseDecl(_) => {}
        }
    }

    // ── Name-check all bodies ─────────────────────────────────────────────
    for item in &items {
        match item {
            Item::FnDef(f) => check_fn(f, &callable_names, &memory_names, &lore_names)?,
            Item::KernelDef(k) => check_kernel(k, &callable_names, &memory_names, &lore_names)?,
            Item::SkillDef(s) => check_skill(s, &callable_names, &memory_names, &lore_names)?,
            Item::SpellDef(s) => check_spell(s, &callable_names, &memory_names, &lore_names)?,
            Item::SoulDef(s) => {
                let mut scope = HashSet::new();
                check_block(
                    &s.body,
                    &mut scope,
                    &callable_names,
                    &memory_names,
                    &lore_names,
                )?;
            }
            Item::MemoryDecl(m) => {
                let scope = HashSet::new();
                check_expr(&m.init, &scope, &callable_names, &memory_names, &lore_names)?;
            }
            Item::ConstraintDef(c) => {
                // Constraint bodies may reference locals from the call site —
                // skip name-checking in Phase 4 (inlined at call site in Phase 5).
                check_constraint_relaxed(c);
            }
            Item::TypeAlias(_) | Item::OracleDecl(_) | Item::LoreDecl(_) | Item::UseDecl(_) => {}
        }
    }

    Ok(TypedAst {
        items,
        type_env,
        oracle_names,
        lore_table,
    })
}

fn check_fn(
    f: &FnDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashSet<String> = f.params.iter().map(|p| p.name.clone()).collect();
    check_block(&f.body, &mut scope, callable, memory, lore)
}

fn check_kernel(
    k: &KernelDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashSet<String> = k.params.iter().map(|p| p.name.clone()).collect();
    check_block(&k.body, &mut scope, callable, memory, lore)
}

fn check_skill(
    s: &SkillDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashSet<String> = s.params.iter().map(|p| p.name.clone()).collect();
    check_block(&s.body, &mut scope, callable, memory, lore)
}

fn check_spell(
    s: &SpellDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashSet<String> = s.params.iter().map(|p| p.name.clone()).collect();
    check_block(&s.body, &mut scope, callable, memory, lore)
}

/// Constraint bodies may reference locals not yet in scope; skip name-checking in Phase 4.
fn check_constraint_relaxed(_c: &ConstraintDef) {}

fn check_block(
    block: &[Stmt],
    scope: &mut HashSet<String>,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
) -> Result<()> {
    for stmt in block {
        match stmt {
            Stmt::Let(name, _ty, expr) => {
                check_expr(expr, scope, callable, memory, lore)?;
                scope.insert(name.clone());
            }
            Stmt::Return(expr) | Stmt::Expr(expr) => {
                check_expr(expr, scope, callable, memory, lore)?;
            }
            Stmt::Branch(b) => {
                check_expr(&Expr::Ident(b.var.clone()), scope, callable, memory, lore)?;
                for case in &b.cases {
                    let mut inner = scope.clone();
                    check_block(&case.body, &mut inner, callable, memory, lore)?;
                }
                if let Some(default) = &b.default {
                    let mut inner = scope.clone();
                    check_block(default, &mut inner, callable, memory, lore)?;
                }
            }
            Stmt::Interruptible(block) => {
                let mut inner = scope.clone();
                check_block(block, &mut inner, callable, memory, lore)?;
            }
            // instruction "text"; — always valid, no names to resolve.
            Stmt::Instruction(_) => {}
        }
    }
    Ok(())
}

fn check_expr(
    expr: &Expr,
    scope: &HashSet<String>,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
) -> Result<()> {
    match expr {
        Expr::Ident(name) => {
            if is_builtin(name)
                || scope.contains(name.as_str())
                || callable.contains(name.as_str())
                || memory.contains(name.as_str())
                || lore.contains(name.as_str())
            {
                Ok(())
            } else {
                Err(anyhow!("undefined variable: `{name}`"))
            }
        }
        Expr::Call(_, args) => {
            for arg in args {
                check_expr(arg, scope, callable, memory, lore)?;
            }
            Ok(())
        }
        Expr::BinOp(lhs, _, rhs) => {
            check_expr(lhs, scope, callable, memory, lore)?;
            check_expr(rhs, scope, callable, memory, lore)
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
            | "ctx_share"
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

    #[test]
    fn accepts_soul_def() {
        let src = r#"
soul {
    instruction "You are a helpful agent.";
}
fn main() {}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_skill_def() {
        let src = "
skill Greet(name: str) -> str {
    return name;
}
fn main() {}
";
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_oracle_callable() {
        let src = r#"
oracle Lookup(q: str) -> str;
fn main() {
    let r = Lookup("test");
    println(r);
}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn registers_oracle_names() {
        let src = "
oracle Lookup(q: str) -> str;
fn main() {}
";
        let ast = compile_src(src).unwrap();
        assert!(ast.oracle_names.contains(&"Lookup".to_string()));
    }

    #[test]
    fn registers_lore_table() {
        let src = r#"
lore Background = "This agent analyses sentiment.";
fn main() {}
"#;
        let ast = compile_src(src).unwrap();
        assert_eq!(
            ast.lore_table.get("Background").unwrap(),
            "This agent analyses sentiment."
        );
    }

    #[test]
    fn accepts_memory_decl() {
        let src = r#"
memory LastResult: str = "";
fn main() {}
"#;
        assert!(compile_src(src).is_ok());
    }
}
