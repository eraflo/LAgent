// SPDX-License-Identifier: Apache-2.0
//! Recursive-descent parser for Wispee source files.
//!
//! Converts a flat [`Vec<Token>`](crate::lexer::Token) produced by the lexer
//! into a typed [`Vec<Item>`](crate::parser::ast::Item) abstract syntax tree.

pub mod ast;

use crate::lexer::Token;
use anyhow::{anyhow, Result};
use ast::{
    BinOp, Block, BranchCase, BranchStmt, ConstDef, ConstraintDef, EnumDef, EnumVariant, Expr,
    FnDef, Item, KernelDef, LoreDecl, MemoryDecl, OracleDecl, Param, PrimType, SkillDef, SoulDef,
    SpellDef, Stmt, StructDef, StructField, TypeAlias, TypeExpr, UseDecl,
};
use chumsky::prelude::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract an identifier string from an `Ident` token.
fn ident() -> impl Parser<Token, String, Error = Simple<Token>> {
    filter(|t| matches!(t, Token::Ident(_))).map(|t| {
        if let Token::Ident(s) = t {
            s
        } else {
            unreachable!()
        }
    })
}

/// Accept an `Ident` token *or* the `intent` keyword token, yielding a String.
/// Used in `branch <var>` where the subject may be a keyword like `intent`.
fn name() -> impl Parser<Token, String, Error = Simple<Token>> {
    filter(|t| matches!(t, Token::Ident(_) | Token::Intent)).map(|t| match t {
        Token::Ident(s) => s,
        Token::Intent => "intent".to_string(),
        _ => unreachable!(),
    })
}

// ── String literal helper ─────────────────────────────────────────────────────

fn str_inner() -> impl Parser<Token, String, Error = Simple<Token>> {
    filter(|t| matches!(t, Token::StringLit(_))).map(|t| {
        if let Token::StringLit(s) = t {
            s[1..s.len() - 1].to_string()
        } else {
            unreachable!()
        }
    })
}

// ── Type expressions ─────────────────────────────────────────────────────────

fn type_expr() -> impl Parser<Token, TypeExpr, Error = Simple<Token>> {
    let prim = just(Token::StrType)
        .to(TypeExpr::Primitive(PrimType::Str))
        .or(just(Token::BoolType).to(TypeExpr::Primitive(PrimType::Bool)))
        .or(just(Token::U32Type).to(TypeExpr::Primitive(PrimType::U32)))
        .or(just(Token::F32Type).to(TypeExpr::Primitive(PrimType::F32)));

    // semantic("label1", "label2", …)
    let semantic = just(Token::Semantic)
        .ignore_then(
            str_inner()
                .separated_by(just(Token::Comma))
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .map(TypeExpr::Semantic);

    // Vec<T> — recursive type
    let vec_ty = recursive(|te| {
        just(Token::Ident("Vec".to_string()))
            .ignore_then(te.delimited_by(just(Token::Lt), just(Token::Gt)))
            .map(|inner| TypeExpr::Vec(Box::new(inner)))
    });

    prim.or(semantic)
        .or(vec_ty)
        .or(ident().map(TypeExpr::Named))
}

/// Postfix operator applied to an expression.
enum PostfixOp {
    Index(Expr),
    Field(String),
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
fn expr() -> impl Parser<Token, Expr, Error = Simple<Token>> {
    recursive(|expr| {
        let str_lit = str_inner().map(Expr::StringLit);

        let int_lit = filter(|t| matches!(t, Token::IntLit(_))).map(|t| {
            if let Token::IntLit(n) = t {
                Expr::IntLit(n)
            } else {
                unreachable!()
            }
        });

        let float_lit = filter(|t| matches!(t, Token::FloatLit(_))).map(|t| {
            if let Token::FloatLit(f) = t {
                Expr::FloatLit(f)
            } else {
                unreachable!()
            }
        });

        let bool_lit = just(Token::Ident("true".to_string()))
            .to(Expr::BoolLit(true))
            .or(just(Token::Ident("false".to_string())).to(Expr::BoolLit(false)));

        let break_expr = just(Token::Break).to(Expr::Break);
        let continue_expr = just(Token::Continue).to(Expr::Continue);

        // Tuple: (expr1, expr2, ...)
        let tuple_expr = expr
            .clone()
            .separated_by(just(Token::Comma))
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .map(|exprs| {
                if exprs.len() == 1 {
                    // Single parenthesized expression, not a tuple
                    exprs.into_iter().next().unwrap()
                } else {
                    Expr::Tuple(exprs)
                }
            });

        // Vector literal: [a, b, c]
        let vec_lit = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(Expr::VecLit);

        let args = expr
            .clone()
            .separated_by(just(Token::Comma))
            .delimited_by(just(Token::LParen), just(Token::RParen));

        // Built-in call keywords that are not plain identifiers in the lexer.
        let builtin_call = filter(|t| {
            matches!(
                t,
                Token::Println
                    | Token::CtxAlloc
                    | Token::CtxFree
                    | Token::CtxAppend
                    | Token::CtxResize
                    | Token::CtxCompress
                    | Token::CtxShare
                    | Token::Observe
                    | Token::Reason
                    | Token::Act
                    | Token::Verify
                    | Token::Infer
                    | Token::MemoryLoad
                    | Token::MemorySave
                    | Token::MemoryDelete
            )
        })
        .map(|t| match t {
            Token::Println => "println".to_string(),
            Token::CtxAlloc => "ctx_alloc".to_string(),
            Token::CtxFree => "ctx_free".to_string(),
            Token::CtxAppend => "ctx_append".to_string(),
            Token::CtxResize => "ctx_resize".to_string(),
            Token::CtxCompress => "ctx_compress".to_string(),
            Token::CtxShare => "ctx_share".to_string(),
            Token::Observe => "observe".to_string(),
            Token::Reason => "reason".to_string(),
            Token::Act => "act".to_string(),
            Token::Verify => "verify".to_string(),
            Token::Infer => "infer".to_string(),
            Token::MemoryLoad => "memory_load".to_string(),
            Token::MemorySave => "memory_save".to_string(),
            Token::MemoryDelete => "memory_delete".to_string(),
            _ => unreachable!(),
        })
        .then(args.clone())
        .map(|(name, a)| Expr::Call(name, a));

        // User-defined function call: Ident followed immediately by '('.
        let user_call = ident().then(args).map(|(name, a)| Expr::Call(name, a));

        let ident_expr = ident().map(Expr::Ident);

        // Struct construction: Name { field: expr, ... }
        let struct_construct = ident()
            .then(
                ident()
                    .then_ignore(just(Token::Colon))
                    .then(expr.clone())
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|(name, fields)| Expr::StructConstruct { name, fields });

        // Enum variant: Variant(expr) — requires parentheses to distinguish from plain Ident
        let enum_construct = ident()
            .then(
                expr.clone()
                    .map(Box::new)
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map(|(variant, payload)| Expr::EnumVariant {
                variant,
                payload: Some(payload),
            });

        // Base atoms (without postfix)
        let atom_base = builtin_call
            .or(user_call)
            .or(str_lit)
            .or(float_lit)
            .or(int_lit)
            .or(bool_lit.clone())
            .or(break_expr)
            .or(continue_expr)
            .or(tuple_expr)
            .or(vec_lit.clone())
            .or(struct_construct)
            .or(enum_construct)
            .or(ident_expr)
            .boxed();

        // Postfix chaining: expr[index] and expr.field
        let index_op = just(Token::LBracket)
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RBracket))
            .map(PostfixOp::Index);

        let field_op = just(Token::Dot)
            .ignore_then(
                filter(|t| matches!(t, Token::IntLit(_)))
                    .map(|t| {
                        if let Token::IntLit(n) = t {
                            n.to_string()
                        } else {
                            unreachable!()
                        }
                    })
                    .or(ident()),
            )
            .map(PostfixOp::Field);

        let postfix = atom_base
            .then(index_op.or(field_op).repeated())
            .foldl(|base, op| match op {
                PostfixOp::Index(idx) => Expr::Index(Box::new(base), Box::new(idx)),
                PostfixOp::Field(name) => Expr::FieldAccess(Box::new(base), name),
            });

        // Box the final expression with postfix for use in binary ops
        let atom = postfix.boxed();

        // Binary operators with proper precedence using Pratt parsing
        // Lowest to highest: || < && < ==,!= < <,> < +,- < *,/,%

        let op_or = just(Token::Or).to(BinOp::Or);
        let op_and = just(Token::And).to(BinOp::And);
        let op_cmp_eq = just(Token::Eq)
            .to(BinOp::Eq)
            .or(just(Token::NotEq).to(BinOp::NotEq));
        let op_cmp_ord = just(Token::Gt)
            .to(BinOp::Gt)
            .or(just(Token::Lt).to(BinOp::Lt));
        let op_plus = just(Token::Plus)
            .to(BinOp::Add)
            .or(just(Token::Minus).to(BinOp::Sub));
        #[allow(clippy::redundant_clone)]
        let op_mul = just(Token::Star)
            .to(BinOp::Mul)
            .or(just(Token::Slash).to(BinOp::Div))
            .or(just(Token::Percent).to(BinOp::Mod));

        // Level 6: atoms
        let level6 = atom.clone();
        // Level 5: *, /, %
        let level5 = level6
            .clone()
            .then(op_mul.then(level6.clone()).repeated())
            .foldl(|lhs, (op, rhs)| Expr::BinOp(Box::new(lhs), op, Box::new(rhs)));
        // Level 4: +, -
        let level4 = level5
            .clone()
            .then(op_plus.then(level5.clone()).repeated())
            .foldl(|lhs, (op, rhs)| Expr::BinOp(Box::new(lhs), op, Box::new(rhs)));
        // Level 3: <, >
        let level3 = level4
            .clone()
            .then(op_cmp_ord.then(level4.clone()).repeated())
            .foldl(|lhs, (op, rhs)| Expr::BinOp(Box::new(lhs), op, Box::new(rhs)));
        // Level 2: ==, !=
        let level2 = level3
            .clone()
            .then(op_cmp_eq.then(level3.clone()).repeated())
            .foldl(|lhs, (op, rhs)| Expr::BinOp(Box::new(lhs), op, Box::new(rhs)));
        // Level 1: &&
        let level1 = level2
            .clone()
            .then(op_and.then(level2.clone()).repeated())
            .foldl(|lhs, (op, rhs)| Expr::BinOp(Box::new(lhs), op, Box::new(rhs)));
        // Level 0: || (highest level, lowest precedence)
        level1
            .clone()
            .then(op_or.then(level1).repeated())
            .foldl(|lhs, (op, rhs)| Expr::BinOp(Box::new(lhs), op, Box::new(rhs)))
    })
}

// ── Statements ────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
fn stmt() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    // Forward-declare so that block() can be used inside branch_stmt().
    recursive(|stmt| {
        let block_inner = stmt
            .repeated()
            .delimited_by(just(Token::LBrace), just(Token::RBrace));

        // let [mut] name [: type] = expr;
        let let_stmt = just(Token::Let)
            .ignore_then(just(Token::Mut).or_not())
            .then(ident())
            .then(just(Token::Colon).ignore_then(type_expr()).or_not())
            .then_ignore(just(Token::Assign))
            .then(expr())
            .then_ignore(just(Token::Semi))
            .map(|(((mut_opt, name), ty), val)| Stmt::Let(name, ty, Some(val), mut_opt.is_some()));

        let return_stmt = just(Token::Return)
            .ignore_then(expr())
            .then_ignore(just(Token::Semi))
            .map(Stmt::Return);

        let expr_stmt = expr().then_ignore(just(Token::Semi)).map(Stmt::Expr);

        // instruction "text";
        let instruction_stmt = just(Token::Instruction)
            .ignore_then(str_inner())
            .then_ignore(just(Token::Semi))
            .map(Stmt::Instruction);

        // branch <name> { case "label" (confidence > N) => { ... } ... default => { ... } }
        let threshold = filter(|t| matches!(t, Token::FloatLit(_)))
            .map(|t| {
                if let Token::FloatLit(f) = t {
                    f
                } else {
                    unreachable!()
                }
            })
            .or(filter(|t| matches!(t, Token::IntLit(_))).map(|t| {
                if let Token::IntLit(n) = t {
                    #[allow(clippy::cast_precision_loss)]
                    {
                        n as f64
                    }
                } else {
                    unreachable!()
                }
            }));

        // (confidence > N) — we only care about the threshold value.
        let confidence_guard = just(Token::LParen)
            .ignore_then(
                ident()
                    .ignore_then(just(Token::Gt).or(just(Token::Lt)))
                    .ignore_then(threshold),
            )
            .then_ignore(just(Token::RParen));

        let branch_case = just(Token::Case)
            .ignore_then(str_inner())
            .then(confidence_guard)
            .then_ignore(just(Token::FatArrow))
            .then(block_inner.clone())
            .map(|((label, confidence), body)| BranchCase {
                label,
                confidence,
                body,
            });

        let default_case = just(Token::Default)
            .ignore_then(just(Token::FatArrow))
            .ignore_then(block_inner.clone());

        let branch_stmt = just(Token::Branch)
            .ignore_then(name())
            .then(
                branch_case
                    .repeated()
                    .then(default_case.or_not())
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|(var, (cases, default))| {
                Stmt::Branch(BranchStmt {
                    var,
                    cases,
                    default,
                })
            });

        let interruptible_stmt = just(Token::Interruptible)
            .ignore_then(block_inner.clone())
            .map(Stmt::Interruptible);

        // apply ConstraintName;
        let apply_stmt = just(Token::Apply)
            .ignore_then(ident())
            .then_ignore(just(Token::Semi))
            .map(Stmt::Apply);

        // Phase 7: if condition { ... } else { ... }
        let if_stmt = just(Token::If)
            .ignore_then(expr())
            .then(block_inner.clone())
            .then(just(Token::Else).ignore_then(block_inner.clone()).or_not())
            .map(|((condition, then_branch), else_branch)| Stmt::If {
                condition,
                then_branch,
                else_branch,
            });

        // Phase 7: loop { ... }
        let loop_stmt = just(Token::Loop)
            .ignore_then(block_inner.clone())
            .map(Stmt::Loop);

        // Phase 7: while condition { ... }
        let while_stmt = just(Token::While)
            .ignore_then(expr())
            .then(block_inner.clone())
            .map(|(condition, body)| Stmt::While { condition, body });

        // Phase 7: for item in collection { ... }
        let for_stmt = just(Token::For)
            .ignore_then(ident())
            .then_ignore(just(Token::In))
            .then(expr())
            .then(block_inner.clone())
            .map(|((item, collection), body)| Stmt::For {
                item,
                collection,
                body,
            });

        // Phase 7: x = expr; — mutable reassignment
        let assign_stmt = ident()
            .then_ignore(just(Token::Assign))
            .then(expr())
            .then_ignore(just(Token::Semi))
            .map(|(name, expr)| Stmt::Assign(name, expr));

        instruction_stmt
            .or(apply_stmt)
            .or(if_stmt)
            .or(loop_stmt)
            .or(while_stmt)
            .or(for_stmt)
            .or(assign_stmt)
            .or(let_stmt)
            .or(return_stmt)
            .or(branch_stmt)
            .or(interruptible_stmt)
            .or(expr_stmt)
    })
}

fn block() -> impl Parser<Token, Block, Error = Simple<Token>> {
    stmt()
        .repeated()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
}

// ── Parameters ────────────────────────────────────────────────────────────────

fn param() -> impl Parser<Token, Param, Error = Simple<Token>> {
    ident()
        .then_ignore(just(Token::Colon))
        .then(type_expr())
        .map(|(name, ty)| Param { name, ty })
}

fn params() -> impl Parser<Token, Vec<Param>, Error = Simple<Token>> {
    param()
        .separated_by(just(Token::Comma))
        .delimited_by(just(Token::LParen), just(Token::RParen))
}

// ── Top-level items ───────────────────────────────────────────────────────────

fn fn_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Fn))
        .then(ident())
        .then(params())
        .then(just(Token::Arrow).ignore_then(type_expr()).or_not())
        .then(block())
        .map(|((((is_pub_opt, name), params), return_type), body)| {
            Item::FnDef(FnDef {
                name,
                params,
                return_type,
                body,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn kernel_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Kernel))
        .then(ident())
        .then(params())
        .then_ignore(just(Token::Arrow))
        .then(type_expr())
        .then(block())
        .map(|((((is_pub_opt, name), params), return_type), body)| {
            Item::KernelDef(KernelDef {
                name,
                params,
                return_type,
                body,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn type_alias() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Type))
        .then(ident())
        .then_ignore(just(Token::Assign))
        .then(type_expr())
        .then_ignore(just(Token::Semi))
        .map(|((is_pub_opt, name), def)| {
            Item::TypeAlias(TypeAlias {
                name,
                def,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

// ── Phase 4 top-level parsers ─────────────────────────────────────────────────

fn soul_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Soul)
        .ignore_then(block())
        .map(|body| Item::SoulDef(SoulDef { body }))
}

fn spell_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Spell))
        .then(ident())
        .then(params())
        .then_ignore(just(Token::Arrow))
        .then(type_expr())
        .then(block())
        .map(|((((is_pub_opt, name), params), ret), body)| {
            Item::SpellDef(SpellDef {
                name,
                params,
                ret,
                body,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn skill_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Skill))
        .then(ident())
        .then(params())
        .then(just(Token::Arrow).ignore_then(type_expr()).or_not())
        .then(block())
        .map(|((((is_pub_opt, name), params), return_type), body)| {
            Item::SkillDef(SkillDef {
                name,
                params,
                return_type,
                body,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn memory_decl() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Memory)
        .ignore_then(ident())
        .then_ignore(just(Token::Colon))
        .then(type_expr())
        .then_ignore(just(Token::Assign))
        .then(expr())
        .then_ignore(just(Token::Semi))
        .map(|((name, ty), init)| Item::MemoryDecl(MemoryDecl { name, ty, init }))
}

fn oracle_decl() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Oracle))
        .then(ident())
        .then(params())
        .then_ignore(just(Token::Arrow))
        .then(type_expr())
        .then_ignore(just(Token::Semi))
        .map(|(((is_pub_opt, name), params), ret)| {
            Item::OracleDecl(OracleDecl {
                name,
                params,
                ret,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn constraint_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Constraint))
        .then(ident())
        .then(block())
        .map(|((is_pub_opt, name), body)| {
            Item::ConstraintDef(ConstraintDef {
                name,
                body,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn lore_decl() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Lore))
        .then(ident())
        .then_ignore(just(Token::Assign))
        .then(str_inner())
        .then_ignore(just(Token::Semi))
        .map(|((is_pub_opt, name), value)| {
            Item::LoreDecl(LoreDecl {
                name,
                value,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn use_decl() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Use)
        .ignore_then(str_inner())
        .then_ignore(just(Token::Semi))
        .map(|path| Item::UseDecl(UseDecl { path }))
}

// ── Phase 7: Composite types & constants ──────────────────────────────────────

fn struct_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Struct))
        .then(ident())
        .then(
            ident()
                .then_ignore(just(Token::Colon))
                .then(type_expr())
                .map(|(name, ty)| StructField { name, ty })
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((is_pub_opt, name), fields)| {
            Item::StructDef(StructDef {
                name,
                fields,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn enum_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Enum))
        .then(ident())
        .then(
            ident()
                .then(
                    just(Token::LParen)
                        .ignore_then(type_expr())
                        .then_ignore(just(Token::RParen))
                        .or_not(),
                )
                .map(|(name, payload)| EnumVariant { name, payload })
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((is_pub_opt, name), variants)| {
            Item::EnumDef(EnumDef {
                name,
                variants,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn const_def() -> impl Parser<Token, Item, Error = Simple<Token>> {
    just(Token::Pub)
        .or_not()
        .then_ignore(just(Token::Const))
        .then(ident())
        .then_ignore(just(Token::Colon))
        .then(type_expr())
        .then_ignore(just(Token::Assign))
        .then(expr())
        .then_ignore(just(Token::Semi))
        .map(|(((is_pub_opt, name), ty), value)| {
            Item::ConstDef(ConstDef {
                name,
                ty,
                value,
                is_pub: is_pub_opt.is_some(),
            })
        })
}

fn program() -> impl Parser<Token, Vec<Item>, Error = Simple<Token>> {
    type_alias()
        .or(kernel_def())
        .or(spell_def())
        .or(skill_def())
        .or(soul_def())
        .or(memory_decl())
        .or(oracle_decl())
        .or(constraint_def())
        .or(lore_decl())
        .or(use_decl())
        .or(struct_def())
        .or(enum_def())
        .or(const_def())
        .or(fn_def())
        .repeated()
        .then_ignore(end())
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a token stream into an AST.
///
/// # Errors
///
/// Returns an error if the token sequence does not conform to the grammar.
pub fn parse(tokens: Vec<Token>) -> Result<Vec<Item>> {
    let len = tokens.len();
    // Assign synthetic unit spans (each token occupies 1 position).
    let stream = chumsky::Stream::from_iter(
        len..len + 1,
        tokens.into_iter().enumerate().map(|(i, t)| (t, i..i + 1)),
    );
    program().parse(stream).map_err(|errs| {
        let msg = errs
            .into_iter()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>()
            .join("; ");
        anyhow!("parse error: {msg}")
    })
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn parses_empty_fn() {
        let tokens = tokenize("fn main() {}").unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.name, "main");
            assert!(f.body.is_empty());
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_hello_la() {
        let src = r#"
fn main() {
    let ctx = ctx_alloc(512);
    ctx_append(ctx, "Hello, L-Agent!");
    println("Hello, L-Agent!");
    ctx_free(ctx);
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.name, "main");
            assert_eq!(f.body.len(), 4);
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_fn_with_param_and_return_type() {
        let src = "fn greet(name: str) -> str { return name; }";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.params.len(), 1);
            assert!(f.return_type.is_some());
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_type_alias() {
        let src = r#"type Emotion = semantic("joie", "colère", "tristesse", "neutre");"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::TypeAlias(ta) = &items[0] {
            assert_eq!(ta.name, "Emotion");
            if let TypeExpr::Semantic(labels) = &ta.def {
                assert_eq!(labels.len(), 4);
            } else {
                panic!("expected Semantic type");
            }
        } else {
            panic!("expected TypeAlias");
        }
    }

    #[test]
    fn parses_kernel_def() {
        let src = r"kernel Foo(x: str) -> str { return x; }";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], Item::KernelDef(_)));
    }

    #[test]
    fn parses_branch_stmt() {
        let src = r#"
fn main() {
    branch intent {
        case "angry" (confidence > 0.7) => {
            println("crise");
        }
        case "help" (confidence > 0.4) => {
            println("support");
        }
        default => {
            println("operateur");
        }
    }
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.body.len(), 1);
            assert!(matches!(&f.body[0], Stmt::Branch(_)));
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_emotion_analysis() {
        let src = r#"
type Emotion = semantic("joie", "colère", "tristesse", "neutre");

kernel AnalyserMessage(texte: str) -> Emotion {
    observe(texte);
    reason("Déterminer l'émotion dominante dans le texte");
    let emotion: Emotion = infer(texte);
    verify(emotion != "neutre");
    return emotion;
}

fn main() {
    let ctx = ctx_alloc(4096);
    ctx_append(ctx, "Je suis très mécontent de ce service !");

    branch intent {
        case "angry" (confidence > 0.7) => {
            println("Gestion de crise activée");
        }
        case "help" (confidence > 0.4) => {
            println("Support standard");
        }
        default => {
            println("Redirection vers un opérateur humain");
        }
    }

    ctx_free(ctx);
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        // TypeAlias, KernelDef, FnDef
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], Item::TypeAlias(_)));
        assert!(matches!(&items[1], Item::KernelDef(_)));
        assert!(matches!(&items[2], Item::FnDef(_)));
    }

    // ── Phase 4 parser tests ──────────────────────────────────────────────────

    #[test]
    fn parses_soul_def() {
        let src = r#"
soul {
    instruction "You are a helpful agent.";
    instruction "Always be concise.";
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::SoulDef(s) = &items[0] {
            assert_eq!(s.body.len(), 2);
            assert!(matches!(&s.body[0], Stmt::Instruction(_)));
            assert!(matches!(&s.body[1], Stmt::Instruction(_)));
        } else {
            panic!("expected SoulDef");
        }
    }

    #[test]
    fn parses_skill_def() {
        let src = r"skill Greet(name: str) -> str { return name; }";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::SkillDef(s) = &items[0] {
            assert_eq!(s.name, "Greet");
            assert_eq!(s.params.len(), 1);
            assert!(!s.is_pub);
        } else {
            panic!("expected SkillDef");
        }
    }

    #[test]
    fn parses_pub_skill_def() {
        let src = r"pub skill Greet(name: str) -> str { return name; }";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::SkillDef(s) = &items[0] {
            assert!(s.is_pub);
        } else {
            panic!("expected SkillDef");
        }
    }

    #[test]
    fn parses_spell_def() {
        let src = r"spell Analyse(text: str) -> str { return text; }";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], Item::SpellDef(_)));
    }

    #[test]
    fn parses_memory_decl() {
        let src = r#"memory LastResult: str = "";"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::MemoryDecl(m) = &items[0] {
            assert_eq!(m.name, "LastResult");
            assert!(matches!(&m.ty, TypeExpr::Primitive(PrimType::Str)));
            assert!(matches!(&m.init, Expr::StringLit(s) if s.is_empty()));
        } else {
            panic!("expected MemoryDecl");
        }
    }

    #[test]
    fn parses_oracle_decl() {
        let src = r"oracle FetchContext(url: str) -> str;";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::OracleDecl(o) = &items[0] {
            assert_eq!(o.name, "FetchContext");
            assert_eq!(o.params.len(), 1);
        } else {
            panic!("expected OracleDecl");
        }
    }

    #[test]
    fn parses_lore_decl() {
        let src = r#"lore Background = "This agent analyses sentiment.";"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::LoreDecl(l) = &items[0] {
            assert_eq!(l.name, "Background");
            assert_eq!(l.value, "This agent analyses sentiment.");
        } else {
            panic!("expected LoreDecl");
        }
    }

    #[test]
    fn parses_use_decl() {
        let src = r#"use "utils.la";"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::UseDecl(u) = &items[0] {
            assert_eq!(u.path, "utils.la");
        } else {
            panic!("expected UseDecl");
        }
    }

    #[test]
    fn parses_constraint_def() {
        let src = r#"constraint PositiveOnly { println("checking"); }"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        if let Item::ConstraintDef(c) = &items[0] {
            assert_eq!(c.name, "PositiveOnly");
        } else {
            panic!("expected ConstraintDef");
        }
    }

    // ── Phase 7 parser tests ──────────────────────────────────────────────

    #[test]
    fn parses_if_else() {
        let src = r#"
fn main() {
    if 1 {
        println("yes");
    } else {
        println("no");
    }
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.body.len(), 1);
            assert!(matches!(&f.body[0], Stmt::If { .. }));
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_loop() {
        let src = r#"
fn main() {
    loop {
        println("looping");
    }
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert!(matches!(&f.body[0], Stmt::Loop(_)));
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_while() {
        let src = r"
fn main() {
    while x != 0 {
        println(x);
    }
}
";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert!(matches!(&f.body[0], Stmt::While { .. }));
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_let_mut() {
        let src = r"
fn main() {
    let mut x = 42;
    x = 100;
}
";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.body.len(), 2);
            // First: let mut
            if let Stmt::Let(_, _, _, is_mut) = &f.body[0] {
                assert!(*is_mut);
            } else {
                panic!("expected Let");
            }
            // Second: assignment
            assert!(matches!(&f.body[1], Stmt::Assign(_, _)));
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_const() {
        let src = r"
const MAX: u32 = 100;
fn main() {}
";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 2);
        if let Item::ConstDef(c) = &items[0] {
            assert_eq!(c.name, "MAX");
        } else {
            panic!("expected ConstDef");
        }
    }

    #[test]
    fn parses_struct_def() {
        let src = r"
struct Point {
    x: f32,
    y: f32,
}
fn main() {}
";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 2);
        if let Item::StructDef(s) = &items[0] {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
        } else {
            panic!("expected StructDef");
        }
    }

    #[test]
    fn parses_enum_def() {
        let src = r"
enum Result {
    Ok,
    Err,
}
fn main() {}
";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::EnumDef(e) = &items[0] {
            assert_eq!(e.name, "Result");
            assert_eq!(e.variants.len(), 2);
        } else {
            panic!("expected EnumDef");
        }
    }

    #[test]
    fn parses_arithmetic_operators() {
        let src = r"
fn main() {
    let a = 1 + 2 * 3;
    let b = 10 - 5;
    let c = 20 / 4;
    let d = 7 % 3;
}
";
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.body.len(), 4);
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_logical_operators() {
        let src = r#"
fn main() {
    if x > 0 && x < 10 {
        println("in range");
    }
    if x == 0 || x == 100 {
        println("boundary");
    }
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.body.len(), 2);
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn parses_tuple() {
        let src = r#"
fn main() {
    let t = (1, "hello", 3.14);
}
"#;
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        if let Item::FnDef(f) = &items[0] {
            assert_eq!(f.body.len(), 1);
        } else {
            panic!("expected FnDef");
        }
    }
}
