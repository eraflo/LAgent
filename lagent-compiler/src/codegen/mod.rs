pub mod opcodes;

use crate::semantic::TypedAst;
use opcodes::{Bytecode, OpCode};
use anyhow::Result;

/// Generate bytecode from the typed AST.
pub fn generate(ast: TypedAst) -> Result<Vec<u8>> {
    let mut instructions = Vec::new();

    // TODO: walk TypedAst and emit OpCodes in Phase 1
    // For now, emit a minimal program that halts.
    let _ = ast;
    instructions.push(OpCode::Halt);

    let bytecode = Bytecode::new(instructions);
    let encoded = bincode::serialize(&bytecode)?;
    Ok(encoded)
}
