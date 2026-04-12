// SPDX-License-Identifier: Apache-2.0
//! L-Agent compiler: lexer → parser → semantic analysis → bytecode.
//!
//! The top-level entry points are:
//! - [`compile`] — compile from a source string (no `use` resolution).
//! - [`compile_file`] — compile from a file path, resolving `use` imports.

// Phase 1 — API documentation will be added progressively.
#![allow(missing_docs)]

pub mod codegen;
pub mod fmt;
pub mod lexer;
pub mod parser;
pub mod project;
pub mod resolver;
pub mod semantic;

use anyhow::Result;

/// Compile L-Agent source code to bytecode.
///
/// `use "path"` declarations are parsed but not resolved (no filesystem access).
/// Use [`compile_file`] when imports must be expanded.
///
/// # Errors
///
/// Returns an error if the source contains lexer errors, parse errors, semantic
/// errors (e.g. undefined variables), or if bytecode serialisation fails.
pub fn compile(source: &str) -> Result<Vec<u8>> {
    let tokens = lexer::tokenize(source)?;
    let ast = parser::parse(tokens)?;
    let typed_ast = semantic::analyze(ast)?;
    let bytecode = codegen::generate(&typed_ast)?;
    Ok(bytecode)
}

/// Compile a `.la` source file to bytecode, resolving `use "path"` imports
/// relative to the directory containing `path`.
///
/// # Errors
///
/// Returns an error if the file cannot be read, or if any compilation step fails.
pub fn compile_file(path: &std::path::Path) -> Result<Vec<u8>> {
    let source = std::fs::read_to_string(path)?;
    let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tokens = lexer::tokenize(&source)?;
    let ast = parser::parse(tokens)?;
    let ast = resolver::resolve_uses(ast, base_dir)?;
    let typed_ast = semantic::analyze(ast)?;
    let bytecode = codegen::generate(&typed_ast)?;
    Ok(bytecode)
}

/// Compile a `.la` source file into a `.lalb` library bundle.
///
/// Only `pub` items are included in the export table.
/// `lib_name` is used as the bundle name (typically from `lagent.toml`).
///
/// # Errors
///
/// Returns an error if the file cannot be read, or if any compilation step fails.
pub fn compile_library_file(path: &std::path::Path, lib_name: &str) -> Result<Vec<u8>> {
    let source = std::fs::read_to_string(path)?;
    let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tokens = lexer::tokenize(&source)?;
    let ast = parser::parse(tokens)?;
    let ast = resolver::resolve_uses(ast, base_dir)?;
    let typed_ast = semantic::analyze(ast)?;
    let bundle = codegen::generate_lib(&typed_ast, lib_name)?;
    Ok(bundle)
}

/// Format L-Agent source code by round-tripping through the AST.
///
/// Returns the normalised source string. Only operates on the given string;
/// `use` declarations are not resolved.
///
/// # Errors
///
/// Returns an error if the source cannot be lexed or parsed.
pub fn format_source(source: &str) -> Result<String> {
    let tokens = lexer::tokenize(source)?;
    let items = parser::parse(tokens)?;
    Ok(fmt::format_items(&items))
}
