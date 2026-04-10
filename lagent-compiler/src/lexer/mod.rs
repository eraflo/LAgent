use logos::Logos;
use anyhow::Result;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r]+")]
#[logos(skip r"//[^\n]*")]
pub enum Token {
    // Keywords
    #[token("fn")]      Fn,
    #[token("kernel")]  Kernel,
    #[token("branch")]  Branch,
    #[token("case")]    Case,
    #[token("default")] Default,
    #[token("type")]    Type,
    #[token("let")]     Let,
    #[token("return")]  Return,
    #[token("observe")] Observe,
    #[token("reason")]  Reason,
    #[token("act")]     Act,
    #[token("verify")]  Verify,
    #[token("infer")]   Infer,

    // Context primitives
    #[token("ctx_alloc")]  CtxAlloc,
    #[token("ctx_free")]   CtxFree,
    #[token("ctx_append")] CtxAppend,
    #[token("ctx_resize")] CtxResize,

    // Local model primitives
    #[token("local_model_load")]   LocalModelLoad,
    #[token("local_model_infer")]  LocalModelInfer,
    #[token("local_model_unload")] LocalModelUnload,
    #[token("local_model_list")]   LocalModelList,

    // Built-ins
    #[token("println")] Println,
    #[token("semantic")] Semantic,
    #[token("intent")]  Intent,

    // Types
    #[token("str")]  StrType,
    #[token("bool")] BoolType,
    #[token("u32")]  U32Type,
    #[token("f32")]  F32Type,

    // Symbols
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("[")] LBracket,
    #[token("]")] RBracket,
    #[token(";")] Semi,
    #[token(":")] Colon,
    #[token(",")] Comma,
    #[token(".")] Dot,
    #[token("=")] Assign,
    #[token("=>")] FatArrow,
    #[token("->")] Arrow,
    #[token("!")] Bang,
    #[token(">")] Gt,
    #[token("<")] Lt,
    #[token("!=")] NotEq,

    // Literals
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLit(String),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLit(f64),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<u64>().ok())]
    IntLit(u64),

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    let lexer = Token::lexer(source);
    let tokens: Result<Vec<_>, _> = lexer.collect();
    tokens.map_err(|_| anyhow::anyhow!("Lexer error"))
}
