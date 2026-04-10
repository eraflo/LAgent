pub mod lexer;
pub mod parser;
pub mod semantic;
pub mod codegen;

use anyhow::Result;

/// Compile L-Agent source code to bytecode.
pub fn compile(source: &str) -> Result<Vec<u8>> {
    let tokens = lexer::tokenize(source)?;
    let ast = parser::parse(tokens)?;
    let typed_ast = semantic::analyze(ast)?;
    let bytecode = codegen::generate(typed_ast)?;
    Ok(bytecode)
}
