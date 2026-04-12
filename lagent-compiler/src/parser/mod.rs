// SPDX-License-Identifier: Apache-2.0
//! Recursive-descent parser for L-Agent source files.
//!
//! Converts a flat [`Vec<Token>`](crate::lexer::Token) produced by the lexer
//! into a typed [`Vec<Item>`](crate::parser::ast::Item) abstract syntax tree.

pub mod ast;

use crate::lexer::Token;
use anyhow::{anyhow, Result};
use ast::{
    BinOp, Block, BranchCase, BranchStmt, ConstraintDef, Expr, FnDef, Item, KernelDef, LoreDecl,
    MemoryDecl, OracleDecl, Param, PrimType, SkillDef, SoulDef, SpellDef, Stmt, TypeAlias,
    TypeExpr, UseDecl,
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

    prim.or(semantic).or(ident().map(TypeExpr::Named))
}

// ── Expressions ───────────────────────────────────────────────────────────────

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

        let args = expr
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

        // Box the atom so that the `Clone` required for the rhs branch works.
        let atom = builtin_call
            .or(user_call)
            .or(str_lit)
            .or(float_lit)
            .or(int_lit)
            .or(ident_expr)
            .boxed();

        // Optional binary comparison: lhs (!=|>|<) rhs
        let op = just(Token::NotEq)
            .to(BinOp::NotEq)
            .or(just(Token::Gt).to(BinOp::Gt))
            .or(just(Token::Lt).to(BinOp::Lt));

        atom.clone().then(op.then(atom).or_not()).map(|(lhs, rhs)| {
            if let Some((op, rhs)) = rhs {
                Expr::BinOp(Box::new(lhs), op, Box::new(rhs))
            } else {
                lhs
            }
        })
    })
}

// ── Statements ────────────────────────────────────────────────────────────────

fn stmt() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    // Forward-declare so that block() can be used inside branch_stmt().
    recursive(|stmt| {
        let block_inner = stmt
            .repeated()
            .delimited_by(just(Token::LBrace), just(Token::RBrace));

        let let_stmt = just(Token::Let)
            .ignore_then(ident())
            .then(just(Token::Colon).ignore_then(type_expr()).or_not())
            .then_ignore(just(Token::Assign))
            .then(expr())
            .then_ignore(just(Token::Semi))
            .map(|((name, ty), val)| Stmt::Let(name, ty, val));

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

        instruction_stmt
            .or(apply_stmt)
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
}
