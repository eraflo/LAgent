// SPDX-License-Identifier: Apache-2.0
//! L-Agent compiler: lexer → parser → semantic analysis → bytecode.
//!
//! The top-level entry point is [`compile`], which runs the full pipeline.

// Phase 1 — API documentation will be added progressively.
#![allow(missing_docs)]

pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod semantic;

use anyhow::Result;

/// Compile L-Agent source code to bytecode.
///
/// # Errors
///
/// Returns an error if the source contains lexer errors, parse errors, semantic
/// errors (e.g. undefined variables), or if bytecode serialisation fails.
pub fn compile(source: &str) -> Result<Vec<u8>> {
    let tokens = lexer::tokenize(source)?;
    let ast = parser::parse(tokens)?;
    let typed_ast = semantic::analyze(ast)?;
    let bytecode = codegen::generate(typed_ast)?;
    Ok(bytecode)
}
