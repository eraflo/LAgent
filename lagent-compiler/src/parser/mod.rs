pub mod ast;

use crate::lexer::Token;
use ast::*;
use anyhow::Result;

/// Parse a token stream into an AST.
/// TODO: implement full chumsky-based parser in Phase 1.
pub fn parse(_tokens: Vec<Token>) -> Result<Vec<Item>> {
    // Placeholder: returns empty program until parser is implemented
    Ok(vec![])
}
