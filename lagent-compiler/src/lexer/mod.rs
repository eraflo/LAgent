// SPDX-License-Identifier: Apache-2.0
//! Lexer for the L-Agent language.
//!
//! Transforms raw `.la` source text into a flat sequence of [`Token`]s
//! using the [`logos`] crate for zero-copy, high-performance tokenisation.

use anyhow::Result;
use logos::Logos;

/// Every token in the L-Agent lexical grammar.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r]+")] // whitespace
#[logos(skip r"//[^\n]*")] // line comments
pub enum Token {
    // ── Control-flow keywords ─────────────────────────────────────────────
    #[token("fn")]
    Fn,
    #[token("kernel")]
    Kernel,
    #[token("branch")]
    Branch,
    #[token("case")]
    Case,
    #[token("default")]
    Default,
    #[token("return")]
    Return,

    // ── Declaration keywords ───────────────────────────────────────────────
    #[token("type")]
    Type,
    #[token("let")]
    Let,
    #[token("pub")]
    Pub,
    #[token("use")]
    Use,

    // ── Agent vocabulary ───────────────────────────────────────────────────
    /// Defines the persistent identity / personality of an agent.
    #[token("soul")]
    Soul,
    /// Declares a reusable declarative capability.
    #[token("skill")]
    Skill,
    /// Injects a typed directive into the system prompt.
    #[token("instruction")]
    Instruction,
    /// Defines a reusable, parameterised prompt template.
    #[token("spell")]
    Spell,
    /// Declares a named persistent state structure.
    #[token("memory")]
    Memory,
    /// Declares an external knowledge source (RAG / vector DB).
    #[token("oracle")]
    Oracle,
    /// Declares a hard invariant — violation halts execution immediately.
    #[token("constraint")]
    Constraint,
    /// Declares a block of few-shot examples injected before inference.
    #[token("lore")]
    Lore,

    // ── Kernel step primitives ─────────────────────────────────────────────
    #[token("observe")]
    Observe,
    #[token("reason")]
    Reason,
    #[token("act")]
    Act,
    #[token("verify")]
    Verify,
    #[token("infer")]
    Infer,

    // ── Context primitives ─────────────────────────────────────────────────
    #[token("ctx_alloc")]
    CtxAlloc,
    #[token("ctx_free")]
    CtxFree,
    #[token("ctx_append")]
    CtxAppend,
    #[token("ctx_resize")]
    CtxResize,

    // ── Local model primitives ─────────────────────────────────────────────
    #[token("local_model_load")]
    LocalModelLoad,
    #[token("local_model_infer")]
    LocalModelInfer,
    #[token("local_model_unload")]
    LocalModelUnload,
    #[token("local_model_list")]
    LocalModelList,

    // ── Built-ins ──────────────────────────────────────────────────────────
    #[token("println")]
    Println,
    #[token("semantic")]
    Semantic,
    #[token("intent")]
    Intent,
    #[token("ask")]
    Ask,

    // ── Primitive types ────────────────────────────────────────────────────
    #[token("str")]
    StrType,
    #[token("bool")]
    BoolType,
    #[token("u32")]
    U32Type,
    #[token("f32")]
    F32Type,

    // ── Symbols ────────────────────────────────────────────────────────────
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(";")]
    Semi,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("=")]
    Assign,
    #[token("=>")]
    FatArrow,
    #[token("->")]
    Arrow,
    #[token("!")]
    Bang,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("!=")]
    NotEq,

    // ── Literals ───────────────────────────────────────────────────────────
    /// A double-quoted string literal. The stored value includes the surrounding quotes.
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLit(String),

    /// A floating-point literal (must contain a decimal point).
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLit(f64),

    /// An unsigned integer literal.
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<u64>().ok())]
    IntLit(u64),

    // ── Identifiers ────────────────────────────────────────────────────────
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

/// Tokenise L-Agent source code into a [`Vec`] of [`Token`]s.
///
/// # Errors
///
/// Returns an error if the source contains an unrecognised lexeme.
pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    let lexer = Token::lexer(source);
    let tokens: std::result::Result<Vec<_>, _> = lexer.collect();
    tokens.map_err(|()| anyhow::anyhow!("Lexer error: unrecognised token"))
}

// ─── Hash / Eq impls ──────────────────────────────────────────────────────────
// Required for chumsky's `Simple<Token>` error type.
// f64 is normalised through its bit representation; NaN is impossible in
// well-formed source code, so this is sound in practice.

impl Eq for Token {}

impl std::hash::Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Token::StringLit(s) | Token::Ident(s) => s.hash(state),
            #[allow(clippy::cast_possible_truncation)]
            Token::FloatLit(f) => f.to_bits().hash(state),
            Token::IntLit(n) => n.hash(state),
            _ => (),
        }
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_fn_keyword() {
        let tokens = tokenize("fn").unwrap();
        assert_eq!(tokens, vec![Token::Fn]);
    }

    #[test]
    fn tokenizes_ctx_alloc_call() {
        let tokens = tokenize("ctx_alloc(4096)").unwrap();
        assert!(tokens.contains(&Token::CtxAlloc));
        assert!(tokens.contains(&Token::LParen));
        assert!(tokens.contains(&Token::RParen));
    }

    #[test]
    fn tokenizes_string_literal() {
        let tokens = tokenize(r#""hello""#).unwrap();
        // The lexer stores the full slice including surrounding quotes.
        assert_eq!(tokens, vec![Token::StringLit(r#""hello""#.to_string())]);
    }

    #[test]
    fn tokenizes_agent_vocabulary_keywords() {
        let src = "soul skill instruction spell memory oracle constraint lore";
        let tokens = tokenize(src).unwrap();
        assert!(tokens.contains(&Token::Soul));
        assert!(tokens.contains(&Token::Skill));
        assert!(tokens.contains(&Token::Instruction));
        assert!(tokens.contains(&Token::Spell));
        assert!(tokens.contains(&Token::Memory));
        assert!(tokens.contains(&Token::Oracle));
        assert!(tokens.contains(&Token::Constraint));
        assert!(tokens.contains(&Token::Lore));
    }

    #[test]
    fn tokenizes_module_keywords() {
        let tokens = tokenize("pub use").unwrap();
        assert_eq!(tokens, vec![Token::Pub, Token::Use]);
    }

    #[test]
    fn tokenizes_integer_literal() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens, vec![Token::IntLit(42)]);
    }

    #[test]
    fn tokenizes_full_fn_signature() {
        let tokens = tokenize("fn main()").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Fn,
                Token::Ident("main".to_string()),
                Token::LParen,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn skips_line_comments() {
        let tokens = tokenize("fn // this is a comment\nmain").unwrap();
        assert_eq!(tokens, vec![Token::Fn, Token::Ident("main".to_string())]);
    }
}
