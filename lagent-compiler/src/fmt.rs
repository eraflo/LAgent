// SPDX-License-Identifier: Apache-2.0
//! AST pretty-printer for `lagent fmt`.
//!
//! Uses a `Pp<'a, T>` newtype wrapper implementing [`std::fmt::Display`] for
//! each AST node. The formatter produces normalised 4-space-indented source.
//! Goal: `fmt(parse(src))` is a valid, round-trippable source file.

use crate::parser::ast::{
    BinOp, BranchCase, BranchStmt, ConstraintDef, Expr, FnDef, Item, KernelDef, LoreDecl,
    MemoryDecl, OracleDecl, Param, PrimType, SkillDef, SoulDef, SpellDef, Stmt, TypeAlias,
    TypeExpr, UseDecl,
};
use std::fmt;

/// Newtype wrapper: `Pp(node, indent_level)`.
struct Pp<'a, T>(pub &'a T, pub usize);

const INDENT: &str = "    ";

fn ind(level: usize) -> String {
    INDENT.repeat(level)
}

// ── Top-level entry point ─────────────────────────────────────────────────────

/// Format a slice of top-level items into a normalised source string.
#[must_use]
pub fn format_items(items: &[Item]) -> String {
    items
        .iter()
        .map(|item| format!("{}", Pp(item, 0)))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Item ──────────────────────────────────────────────────────────────────────

impl fmt::Display for Pp<'_, Item> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Item::FnDef(x) => write!(f, "{}", Pp(x, self.1)),
            Item::KernelDef(x) => write!(f, "{}", Pp(x, self.1)),
            Item::TypeAlias(x) => write!(f, "{}", Pp(x, self.1)),
            Item::SoulDef(x) => write!(f, "{}", Pp(x, self.1)),
            Item::SpellDef(x) => write!(f, "{}", Pp(x, self.1)),
            Item::SkillDef(x) => write!(f, "{}", Pp(x, self.1)),
            Item::MemoryDecl(x) => write!(f, "{}", Pp(x, self.1)),
            Item::OracleDecl(x) => write!(f, "{}", Pp(x, self.1)),
            Item::ConstraintDef(x) => write!(f, "{}", Pp(x, self.1)),
            Item::LoreDecl(x) => write!(f, "{}", Pp(x, self.1)),
            Item::UseDecl(x) => write!(f, "{}", Pp(x, self.1)),
        }
    }
}

// ── Declarations ──────────────────────────────────────────────────────────────

impl fmt::Display for Pp<'_, FnDef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub ")?;
        } else {
            write!(f, "{i}")?;
        }
        write!(f, "fn {}(", self.0.name)?;
        fmt_params(f, &self.0.params)?;
        write!(f, ")")?;
        if let Some(rt) = &self.0.return_type {
            write!(f, " -> {}", Pp(rt, 0))?;
        }
        write!(f, " {}", fmt_block(&self.0.body, self.1))
    }
}

impl fmt::Display for Pp<'_, KernelDef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub kernel {}(", self.0.name)?;
        } else {
            write!(f, "{i}kernel {}(", self.0.name)?;
        }
        fmt_params(f, &self.0.params)?;
        write!(
            f,
            ") -> {} {}",
            Pp(&self.0.return_type, 0),
            fmt_block(&self.0.body, self.1)
        )
    }
}

impl fmt::Display for Pp<'_, TypeAlias> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub type {} = {};", self.0.name, Pp(&self.0.def, 0))
        } else {
            write!(f, "{i}type {} = {};", self.0.name, Pp(&self.0.def, 0))
        }
    }
}

impl fmt::Display for Pp<'_, SoulDef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        write!(f, "{i}soul {}", fmt_block(&self.0.body, self.1))
    }
}

impl fmt::Display for Pp<'_, SpellDef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub spell {}(", self.0.name)?;
        } else {
            write!(f, "{i}spell {}(", self.0.name)?;
        }
        fmt_params(f, &self.0.params)?;
        write!(
            f,
            ") -> {} {}",
            Pp(&self.0.ret, 0),
            fmt_block(&self.0.body, self.1)
        )
    }
}

impl fmt::Display for Pp<'_, SkillDef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub skill {}(", self.0.name)?;
        } else {
            write!(f, "{i}skill {}(", self.0.name)?;
        }
        fmt_params(f, &self.0.params)?;
        write!(f, ")")?;
        if let Some(rt) = &self.0.return_type {
            write!(f, " -> {}", Pp(rt, 0))?;
        }
        write!(f, " {}", fmt_block(&self.0.body, self.1))
    }
}

impl fmt::Display for Pp<'_, MemoryDecl> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        write!(
            f,
            "{i}memory {}: {} = {};",
            self.0.name,
            Pp(&self.0.ty, 0),
            Pp(&self.0.init, 0)
        )
    }
}

impl fmt::Display for Pp<'_, OracleDecl> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub oracle {}(", self.0.name)?;
        } else {
            write!(f, "{i}oracle {}(", self.0.name)?;
        }
        fmt_params(f, &self.0.params)?;
        write!(f, ") -> {};", Pp(&self.0.ret, 0))
    }
}

impl fmt::Display for Pp<'_, ConstraintDef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(
                f,
                "{i}pub constraint {} {}",
                self.0.name,
                fmt_block(&self.0.body, self.1)
            )
        } else {
            write!(
                f,
                "{i}constraint {} {}",
                self.0.name,
                fmt_block(&self.0.body, self.1)
            )
        }
    }
}

impl fmt::Display for Pp<'_, LoreDecl> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        if self.0.is_pub {
            write!(f, "{i}pub lore {} = \"{}\";", self.0.name, self.0.value)
        } else {
            write!(f, "{i}lore {} = \"{}\";", self.0.name, self.0.value)
        }
    }
}

impl fmt::Display for Pp<'_, UseDecl> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        write!(f, "{i}use \"{}\";", self.0.path)
    }
}

// ── Statements ────────────────────────────────────────────────────────────────

fn fmt_block(stmts: &[Stmt], indent: usize) -> String {
    if stmts.is_empty() {
        return "{}".to_string();
    }
    let inner_indent = indent + 1;
    let body: String = stmts
        .iter()
        .map(|s| format!("{}{}", ind(inner_indent), Pp(s, inner_indent)))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{{\n{body}\n{}}}", ind(indent))
}

impl fmt::Display for Pp<'_, Stmt> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Stmt::Let(name, ty, expr) => {
                if let Some(t) = ty {
                    write!(f, "let {name}: {} = {};", Pp(t, 0), Pp(expr, 0))
                } else {
                    write!(f, "let {name} = {};", Pp(expr, 0))
                }
            }
            Stmt::Return(expr) => write!(f, "return {};", Pp(expr, 0)),
            Stmt::Expr(expr) => write!(f, "{};", Pp(expr, 0)),
            Stmt::Instruction(text) => write!(f, "instruction \"{text}\";"),
            Stmt::Apply(name) => write!(f, "apply {name};"),
            Stmt::Branch(b) => write!(f, "{}", Pp(b, self.1)),
            Stmt::Interruptible(block) => {
                write!(f, "interruptible {}", fmt_block(block, self.1))
            }
        }
    }
}

impl fmt::Display for Pp<'_, BranchStmt> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i = ind(self.1);
        let inner = self.1 + 1;
        writeln!(f, "branch {} {{", self.0.var)?;
        for case in &self.0.cases {
            write!(f, "{}", Pp(&(case, inner), 0))?;
        }
        if let Some(default) = &self.0.default {
            writeln!(f, "{}default => {}", ind(inner), fmt_block(default, inner))?;
        }
        write!(f, "{i}}}")
    }
}

impl fmt::Display for Pp<'_, (&BranchCase, usize)> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (case, indent) = self.0;
        let i = ind(*indent);
        writeln!(
            f,
            "{i}case \"{}\" (confidence > {}) => {}",
            case.label,
            case.confidence,
            fmt_block(&case.body, *indent)
        )
    }
}

// ── Expressions ───────────────────────────────────────────────────────────────

impl fmt::Display for Pp<'_, Expr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Expr::StringLit(s) => write!(f, "\"{s}\""),
            Expr::IntLit(n) => write!(f, "{n}"),
            Expr::FloatLit(v) => write!(f, "{v}"),
            Expr::Ident(name) => write!(f, "{name}"),
            Expr::Call(name, args) => {
                write!(f, "{name}(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", Pp(arg, 0))?;
                }
                write!(f, ")")
            }
            Expr::BinOp(lhs, op, rhs) => {
                write!(
                    f,
                    "{} {} {}",
                    Pp(lhs.as_ref(), 0),
                    Pp(op, 0),
                    Pp(rhs.as_ref(), 0)
                )
            }
        }
    }
}

impl fmt::Display for Pp<'_, BinOp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            BinOp::NotEq => write!(f, "!="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Lt => write!(f, "<"),
        }
    }
}

// ── Types ─────────────────────────────────────────────────────────────────────

impl fmt::Display for Pp<'_, TypeExpr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            TypeExpr::Named(name) => write!(f, "{name}"),
            TypeExpr::Primitive(p) => write!(f, "{}", Pp(p, 0)),
            TypeExpr::Semantic(labels) => {
                write!(f, "semantic(")?;
                for (i, label) in labels.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{label}\"")?;
                }
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for Pp<'_, PrimType> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            PrimType::Str => write!(f, "str"),
            PrimType::Bool => write!(f, "bool"),
            PrimType::U32 => write!(f, "u32"),
            PrimType::F32 => write!(f, "f32"),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fmt_params(f: &mut fmt::Formatter<'_>, params: &[Param]) -> fmt::Result {
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{}: {}", p.name, Pp(&p.ty, 0))?;
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn roundtrip(src: &str) -> String {
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        format_items(&items)
    }

    #[test]
    fn formats_empty_fn() {
        let out = roundtrip("fn main() {}");
        assert!(out.contains("fn main()"));
        assert!(out.contains("{}"));
    }

    #[test]
    fn formats_pub_fn() {
        let out = roundtrip("pub fn helper() {}");
        assert!(out.contains("pub fn helper()"));
    }

    #[test]
    fn formats_lore_decl() {
        let out = roundtrip(r#"lore Background = "Some text.";"#);
        assert!(out.contains("lore Background"));
        assert!(out.contains("Some text."));
    }

    #[test]
    fn formats_memory_decl() {
        let out = roundtrip(r#"memory LastResult: str = "";"#);
        assert!(out.contains("memory LastResult: str"));
    }

    #[test]
    fn formats_constraint_def() {
        let out = roundtrip(r#"constraint NonEmpty { verify(result != ""); }"#);
        assert!(out.contains("constraint NonEmpty"));
        assert!(out.contains("verify"));
    }
}
