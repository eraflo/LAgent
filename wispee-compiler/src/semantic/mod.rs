// SPDX-License-Identifier: Apache-2.0
//! Semantic analysis: name resolution and basic constraint validation.

use crate::parser::ast::{
    BinOp, Block, ConstraintDef, Expr, FnDef, Item, KernelDef, SkillDef, SpellDef, Stmt, TypeExpr,
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
    /// Constraint name → body block, for inline codegen (Phase 5).
    pub constraint_bodies: HashMap<String, Block>,
    /// Struct name → fields, for struct construction codegen.
    pub struct_table: HashMap<String, Vec<(String, TypeExpr)>>,
    /// Enum name → variants, for enum codegen.
    pub enum_table: HashMap<String, Vec<String>>,
    /// Const name → compile-time evaluated value.
    pub const_table: HashMap<String, ConstValue>,
}

/// Perform semantic analysis: name resolution and basic constraint validation.
///
/// # Errors
///
/// Returns an error if an identifier is used before it is declared.
pub fn analyze(items: Vec<Item>) -> Result<TypedAst> {
    let mut env = GlobalEnv::build(&items);
    env.evaluate_consts(&items);
    env.check_all(&items)?;
    Ok(TypedAst {
        items,
        type_env: env.type_env,
        oracle_names: env.oracle_names,
        lore_table: env.lore_table,
        constraint_bodies: env.constraint_bodies,
        struct_table: env.struct_table,
        enum_table: env.enum_table,
        const_table: env.const_table,
    })
}

/// Evaluate an expression at compile-time. Returns None if the expression is not a constant.
fn eval_const_expr(expr: &Expr, const_values: &HashMap<String, ConstValue>) -> Option<ConstValue> {
    match expr {
        Expr::IntLit(n) => Some(ConstValue::Int((*n).cast_signed())),
        Expr::FloatLit(f) => Some(ConstValue::Float(*f)),
        Expr::BoolLit(b) => Some(ConstValue::Bool(*b)),
        Expr::StringLit(s) => Some(ConstValue::Str(s.clone())),
        Expr::Ident(name) => const_values.get(name).cloned(),
        Expr::BinOp(lhs, op, rhs) => {
            let l = eval_const_expr(lhs, const_values)?;
            let r = eval_const_expr(rhs, const_values)?;
            eval_const_binop(op, &l, &r)
        }
        _ => None,
    }
}

/// Evaluate a binary operation on constant values.
fn eval_const_binop(op: &BinOp, lhs: &ConstValue, rhs: &ConstValue) -> Option<ConstValue> {
    match (op, lhs, rhs) {
        // Arithmetic on integers
        (BinOp::Add, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a + b)),
        (BinOp::Sub, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a - b)),
        (BinOp::Mul, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a * b)),
        (BinOp::Div, ConstValue::Int(a), ConstValue::Int(b)) => {
            if *b == 0 {
                None
            } else {
                Some(ConstValue::Int(a / b))
            }
        }
        (BinOp::Mod, ConstValue::Int(a), ConstValue::Int(b)) => {
            if *b == 0 {
                None
            } else {
                Some(ConstValue::Int(a % b))
            }
        }
        // Arithmetic on floats
        (BinOp::Add, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a + b)),
        (BinOp::Sub, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a - b)),
        (BinOp::Mul, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a * b)),
        (BinOp::Div, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a / b)),
        // Comparisons on integers
        (BinOp::Eq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a == b)),
        (BinOp::NotEq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a != b)),
        (BinOp::Gt, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a > b)),
        (BinOp::Lt, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a < b)),
        // Comparisons on floats
        (BinOp::Eq, ConstValue::Float(a), ConstValue::Float(b)) => {
            Some(ConstValue::Bool((*a - *b).abs() < f64::EPSILON))
        }
        (BinOp::NotEq, ConstValue::Float(a), ConstValue::Float(b)) => {
            Some(ConstValue::Bool((*a - *b).abs() >= f64::EPSILON))
        }
        (BinOp::Gt, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Bool(a > b)),
        (BinOp::Lt, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Bool(a < b)),
        // Logical operators
        (BinOp::And, ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(*a && *b)),
        (BinOp::Or, ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(*a || *b)),
        // Comparisons on strings
        (BinOp::Eq, ConstValue::Str(a), ConstValue::Str(b)) => Some(ConstValue::Bool(a == b)),
        (BinOp::NotEq, ConstValue::Str(a), ConstValue::Str(b)) => Some(ConstValue::Bool(a != b)),
        _ => None,
    }
}
struct GlobalEnv {
    type_env: HashMap<String, Vec<String>>,
    oracle_names: Vec<String>,
    lore_table: HashMap<String, String>,
    constraint_bodies: HashMap<String, Block>,
    callable_names: HashSet<String>,
    memory_names: HashSet<String>,
    lore_names: HashSet<String>,
    constraint_names: HashSet<String>,
    struct_table: HashMap<String, Vec<(String, TypeExpr)>>,
    enum_table: HashMap<String, Vec<String>>,
    const_table: HashMap<String, ConstValue>,
    const_names: HashSet<String>,
}

/// A compile-time constant value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

impl GlobalEnv {
    fn build(items: &[Item]) -> Self {
        let mut env = Self {
            type_env: HashMap::new(),
            oracle_names: Vec::new(),
            lore_table: HashMap::new(),
            constraint_bodies: HashMap::new(),
            callable_names: HashSet::new(),
            memory_names: HashSet::new(),
            lore_names: HashSet::new(),
            constraint_names: HashSet::new(),
            struct_table: HashMap::new(),
            enum_table: HashMap::new(),
            const_table: HashMap::new(),
            const_names: HashSet::new(),
        };
        for item in items {
            match item {
                Item::TypeAlias(ta) => {
                    if let TypeExpr::Semantic(labels) = &ta.def {
                        env.type_env.insert(ta.name.clone(), labels.clone());
                    }
                }
                Item::FnDef(f) => {
                    env.callable_names.insert(f.name.clone());
                }
                Item::KernelDef(k) => {
                    env.callable_names.insert(k.name.clone());
                }
                Item::SkillDef(s) => {
                    env.callable_names.insert(s.name.clone());
                }
                Item::SpellDef(s) => {
                    env.callable_names.insert(s.name.clone());
                }
                Item::OracleDecl(o) => {
                    env.callable_names.insert(o.name.clone());
                    env.oracle_names.push(o.name.clone());
                }
                Item::MemoryDecl(m) => {
                    env.memory_names.insert(m.name.clone());
                }
                Item::LoreDecl(l) => {
                    env.lore_names.insert(l.name.clone());
                    env.lore_table.insert(l.name.clone(), l.value.clone());
                }
                Item::ConstraintDef(c) => {
                    env.constraint_names.insert(c.name.clone());
                    env.constraint_bodies.insert(c.name.clone(), c.body.clone());
                }
                Item::StructDef(s) => {
                    let fields: Vec<(String, TypeExpr)> = s
                        .fields
                        .iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    env.struct_table.insert(s.name.clone(), fields);
                    env.callable_names.insert(s.name.clone());
                }
                Item::EnumDef(e) => {
                    let variants: Vec<String> = e.variants.iter().map(|v| v.name.clone()).collect();
                    env.enum_table.insert(e.name.clone(), variants);
                    env.callable_names.insert(e.name.clone());
                }
                Item::ConstDef(c) => {
                    env.const_names.insert(c.name.clone());
                }
                Item::SoulDef(_) | Item::UseDecl(_) => {}
            }
        }
        env
    }

    fn evaluate_consts(&mut self, items: &[Item]) {
        // Collect const definitions with their raw expressions
        let const_defs: Vec<(String, Expr)> = items
            .iter()
            .filter_map(|item| {
                if let Item::ConstDef(c) = item {
                    Some((c.name.clone(), c.value.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Iteratively evaluate consts until no more progress is made
        let mut changed = true;
        while changed {
            changed = false;
            for (name, expr) in &const_defs {
                if self.const_table.contains_key(name) {
                    continue;
                }
                if let Some(value) = eval_const_expr(expr, &self.const_table) {
                    self.const_table.insert(name.clone(), value);
                    changed = true;
                }
            }
        }
    }

    fn check_all(&self, items: &[Item]) -> Result<()> {
        let Self {
            callable_names: callable,
            memory_names: memory,
            lore_names: lore,
            constraint_names: constraints,
            const_names,
            ..
        } = self;
        for item in items {
            match item {
                Item::FnDef(f) => check_fn(f, callable, memory, lore, constraints, const_names)?,
                Item::KernelDef(k) => {
                    check_kernel(k, callable, memory, lore, constraints, const_names)?;
                }
                Item::SkillDef(s) => {
                    check_skill(s, callable, memory, lore, constraints, const_names)?;
                }
                Item::SpellDef(s) => {
                    check_spell(s, callable, memory, lore, constraints, const_names)?;
                }
                Item::SoulDef(s) => {
                    let mut scope: HashMap<String, VarInfo> = HashMap::new();
                    check_block(
                        &s.body,
                        &mut scope,
                        callable,
                        memory,
                        lore,
                        constraints,
                        const_names,
                    )?;
                }
                Item::MemoryDecl(m) => {
                    let scope: HashMap<String, VarInfo> = HashMap::new();
                    check_expr(&m.init, &scope, callable, memory, lore, const_names)?;
                }
                Item::ConstraintDef(c) => check_constraint_relaxed(c),
                Item::TypeAlias(_)
                | Item::OracleDecl(_)
                | Item::LoreDecl(_)
                | Item::UseDecl(_)
                | Item::StructDef(_)
                | Item::EnumDef(_)
                | Item::ConstDef(_) => {}
            }
        }
        Ok(())
    }
}

fn check_fn(
    f: &FnDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
    constraints: &HashSet<String>,
    const_names: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashMap<String, VarInfo> = f
        .params
        .iter()
        .map(|p| (p.name.clone(), VarInfo { is_mut: false }))
        .collect();
    check_block(
        &f.body,
        &mut scope,
        callable,
        memory,
        lore,
        constraints,
        const_names,
    )
}

fn check_kernel(
    k: &KernelDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
    constraints: &HashSet<String>,
    const_names: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashMap<String, VarInfo> = k
        .params
        .iter()
        .map(|p| (p.name.clone(), VarInfo { is_mut: false }))
        .collect();
    check_block(
        &k.body,
        &mut scope,
        callable,
        memory,
        lore,
        constraints,
        const_names,
    )
}

fn check_skill(
    s: &SkillDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
    constraints: &HashSet<String>,
    const_names: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashMap<String, VarInfo> = s
        .params
        .iter()
        .map(|p| (p.name.clone(), VarInfo { is_mut: false }))
        .collect();
    check_block(
        &s.body,
        &mut scope,
        callable,
        memory,
        lore,
        constraints,
        const_names,
    )
}

fn check_spell(
    s: &SpellDef,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
    constraints: &HashSet<String>,
    const_names: &HashSet<String>,
) -> Result<()> {
    let mut scope: HashMap<String, VarInfo> = s
        .params
        .iter()
        .map(|p| (p.name.clone(), VarInfo { is_mut: false }))
        .collect();
    check_block(
        &s.body,
        &mut scope,
        callable,
        memory,
        lore,
        constraints,
        const_names,
    )
}

/// Constraint bodies may reference locals from the call site; skip strict name-checking.
fn check_constraint_relaxed(_c: &ConstraintDef) {}

/// A variable in the current scope level.
#[derive(Debug, Clone)]
struct VarInfo {
    /// Whether the variable is mutable.
    is_mut: bool,
}

#[allow(clippy::too_many_lines)]
fn check_block(
    block: &[Stmt],
    scope: &mut HashMap<String, VarInfo>,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
    constraints: &HashSet<String>,
    const_names: &HashSet<String>,
) -> Result<()> {
    for stmt in block {
        match stmt {
            Stmt::Let(name, _ty, expr_opt, is_mut) => {
                if let Some(expr) = expr_opt {
                    check_expr(expr, scope, callable, memory, lore, const_names)?;
                }
                scope.insert(name.clone(), VarInfo { is_mut: *is_mut });
            }
            Stmt::Return(expr) | Stmt::Expr(expr) => {
                check_expr(expr, scope, callable, memory, lore, const_names)?;
            }
            Stmt::Branch(b) => {
                check_expr(
                    &Expr::Ident(b.var.clone()),
                    scope,
                    callable,
                    memory,
                    lore,
                    const_names,
                )?;
                for case in &b.cases {
                    let mut inner = scope.clone();
                    check_block(
                        &case.body,
                        &mut inner,
                        callable,
                        memory,
                        lore,
                        constraints,
                        const_names,
                    )?;
                }
                if let Some(default) = &b.default {
                    let mut inner = scope.clone();
                    check_block(
                        default,
                        &mut inner,
                        callable,
                        memory,
                        lore,
                        constraints,
                        const_names,
                    )?;
                }
            }
            Stmt::Interruptible(block) => {
                let mut inner = scope.clone();
                check_block(
                    block,
                    &mut inner,
                    callable,
                    memory,
                    lore,
                    constraints,
                    const_names,
                )?;
            }
            // instruction "text"; — always valid, no names to resolve.
            Stmt::Instruction(_) => {}
            Stmt::Apply(name) => {
                if !constraints.contains(name.as_str()) {
                    return Err(anyhow!("undefined constraint: `{name}`"));
                }
            }
            // Phase 7: Reassignment — only allowed on mutable variables.
            Stmt::Assign(name, expr) => {
                check_expr(expr, scope, callable, memory, lore, const_names)?;
                if let Some(var) = scope.get(name) {
                    if !var.is_mut {
                        return Err(anyhow!("cannot assign to immutable variable `{name}` — declare it with `let mut {name}`"));
                    }
                } else if is_builtin(name)
                    || callable.contains(name.as_str())
                    || memory.contains(name.as_str())
                    || lore.contains(name.as_str())
                    || const_names.contains(name.as_str())
                {
                    return Err(anyhow!(
                        "cannot assign to builtin/callable/memory/lore/const `{name}`"
                    ));
                } else {
                    return Err(anyhow!("undefined variable: `{name}`"));
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                check_expr(condition, scope, callable, memory, lore, const_names)?;
                let mut inner = scope.clone();
                check_block(
                    then_branch,
                    &mut inner,
                    callable,
                    memory,
                    lore,
                    constraints,
                    const_names,
                )?;
                if let Some(else_block) = else_branch {
                    let mut inner = scope.clone();
                    check_block(
                        else_block,
                        &mut inner,
                        callable,
                        memory,
                        lore,
                        constraints,
                        const_names,
                    )?;
                }
            }
            Stmt::Loop(body) => {
                let mut inner = scope.clone();
                check_block(
                    body,
                    &mut inner,
                    callable,
                    memory,
                    lore,
                    constraints,
                    const_names,
                )?;
            }
            Stmt::While { condition, body } => {
                check_expr(condition, scope, callable, memory, lore, const_names)?;
                let mut inner = scope.clone();
                check_block(
                    body,
                    &mut inner,
                    callable,
                    memory,
                    lore,
                    constraints,
                    const_names,
                )?;
            }
            Stmt::For {
                item,
                collection,
                body,
            } => {
                check_expr(collection, scope, callable, memory, lore, const_names)?;
                let mut inner = scope.clone();
                inner.insert(item.clone(), VarInfo { is_mut: false });
                check_block(
                    body,
                    &mut inner,
                    callable,
                    memory,
                    lore,
                    constraints,
                    const_names,
                )?;
            }
        }
    }
    Ok(())
}

fn check_expr(
    expr: &Expr,
    scope: &HashMap<String, VarInfo>,
    callable: &HashSet<String>,
    memory: &HashSet<String>,
    lore: &HashSet<String>,
    const_names: &HashSet<String>,
) -> Result<()> {
    match expr {
        Expr::Ident(name) => {
            if is_builtin(name)
                || scope.contains_key(name.as_str())
                || callable.contains(name.as_str())
                || memory.contains(name.as_str())
                || lore.contains(name.as_str())
                || const_names.contains(name.as_str())
            {
                Ok(())
            } else {
                Err(anyhow!("undefined variable: `{name}`"))
            }
        }
        Expr::Call(name, args) => {
            if !is_builtin(name)
                && !callable.contains(name.as_str())
                && !memory.contains(name.as_str())
                && !lore.contains(name.as_str())
                && !const_names.contains(name.as_str())
            {
                return Err(anyhow!("undefined function: `{name}`"));
            }
            for arg in args {
                check_expr(arg, scope, callable, memory, lore, const_names)?;
            }
            Ok(())
        }
        Expr::BinOp(lhs, op, rhs) => {
            check_expr(lhs, scope, callable, memory, lore, const_names)?;
            check_expr(rhs, scope, callable, memory, lore, const_names)?;
            // Reject obviously wrong operations on literals.
            let is_arith = matches!(
                op,
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
            );
            let is_logical = matches!(op, BinOp::And | BinOp::Or);
            let lhs_is_str = matches!(lhs.as_ref(), Expr::StringLit(_));
            let rhs_is_str = matches!(rhs.as_ref(), Expr::StringLit(_));
            let lhs_is_non_bool = matches!(
                lhs.as_ref(),
                Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StringLit(_)
            );
            let rhs_is_non_bool = matches!(
                rhs.as_ref(),
                Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StringLit(_)
            );
            if is_arith && (lhs_is_str || rhs_is_str) {
                return Err(anyhow!("cannot perform arithmetic on strings"));
            }
            if is_logical && (lhs_is_non_bool || rhs_is_non_bool) {
                return Err(anyhow!("logical `&&`/`||` requires boolean operands"));
            }
            Ok(())
        }
        Expr::StringLit(_) | Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) => Ok(()),
        // Phase 7: Control flow expressions
        Expr::Break | Expr::Continue => {
            // These are only valid inside loops - we'll check this in codegen
            Ok(())
        }
        Expr::Tuple(exprs) | Expr::VecLit(exprs) => {
            for e in exprs {
                check_expr(e, scope, callable, memory, lore, const_names)?;
            }
            Ok(())
        }
        Expr::Index(base, idx) => {
            check_expr(base, scope, callable, memory, lore, const_names)?;
            check_expr(idx, scope, callable, memory, lore, const_names)
        }
        Expr::FieldAccess(base, _field) => {
            check_expr(base, scope, callable, memory, lore, const_names)
        }
        Expr::StructConstruct { fields, .. } => {
            for (_, e) in fields {
                check_expr(e, scope, callable, memory, lore, const_names)?;
            }
            Ok(())
        }
        Expr::EnumVariant { payload, .. } => {
            if let Some(e) = payload {
                check_expr(e, scope, callable, memory, lore, const_names)?;
            }
            Ok(())
        }
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
            | "memory_load"
            | "memory_save"
            | "memory_delete"
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

    // ── Phase 7 tests ──────────────────────────────────────────────────────

    #[test]
    fn accepts_let_mut() {
        let src = r"
fn main() {
    let mut x = 42;
}
";
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_const_decl() {
        let src = r"
const MAX: u32 = 100;
fn main() {}
";
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn const_table_populated() {
        let src = r#"
const THRESHOLD: f32 = 0.75;
const NAME: str = "test";
fn main() {}
"#;
        let ast = compile_src(src).unwrap();
        assert_eq!(
            ast.const_table.get("THRESHOLD"),
            Some(&ConstValue::Float(0.75))
        );
        assert_eq!(
            ast.const_table.get("NAME"),
            Some(&ConstValue::Str("test".to_string()))
        );
    }

    #[test]
    fn const_arithmetic_evaluated_at_compile_time() {
        let src = r"
const DOUBLE: u32 = 5 * 2;
const SUM: u32 = 10 + 20;
const IS_BIG: bool = 100 > 50;
fn main() {}
";
        let ast = compile_src(src).unwrap();
        assert_eq!(ast.const_table.get("DOUBLE"), Some(&ConstValue::Int(10)));
        assert_eq!(ast.const_table.get("SUM"), Some(&ConstValue::Int(30)));
        assert_eq!(ast.const_table.get("IS_BIG"), Some(&ConstValue::Bool(true)));
    }

    #[test]
    fn accepts_if_else() {
        let src = r#"
fn main() {
    if 1 {
        println("yes");
    } else {
        println("no");
    }
}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_loop() {
        let src = r#"
fn main() {
    loop {
        println("spinning");
    }
}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_while() {
        let src = r#"
fn main() {
    while 1 {
        println("looping");
    }
}
"#;
        assert!(compile_src(src).is_ok());
    }

    #[test]
    fn accepts_struct_def() {
        let src = r"
struct User {
    name: str,
    age: u32,
}
fn main() {}
";
        let ast = compile_src(src).unwrap();
        assert!(ast.struct_table.contains_key("User"));
        let fields = ast.struct_table.get("User").unwrap();
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn accepts_enum_def() {
        let src = r"
enum Color {
    Red,
    Green,
    Blue,
}
fn main() {}
";
        let ast = compile_src(src).unwrap();
        assert!(ast.enum_table.contains_key("Color"));
        let variants = ast.enum_table.get("Color").unwrap();
        assert_eq!(variants.len(), 3);
    }
}
