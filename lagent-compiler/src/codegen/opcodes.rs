use serde::{Serialize, Deserialize};

/// L-Agent bytecode instruction set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpCode {
    /// Allocate a context segment of `tokens` tokens.
    CtxAlloc(u32),
    /// Free context segment at register index.
    CtxFree(u8),
    /// Append string (reg) to context segment (reg).
    CtxAppend(u8, u8),
    /// Resize context segment.
    CtxResize(u8, u32),

    /// Push string literal onto stack.
    PushStr(String),
    /// Push integer literal onto stack.
    PushInt(u64),
    /// Push float literal onto stack.
    PushFloat(f64),

    /// Call a named function.
    Call(String),
    /// Call a kernel by index.
    CallKernel(u16),

    /// Probabilistic branch: list of (label, confidence_threshold, jump_offset), default offset.
    Branch {
        cases: Vec<(String, f32, u16)>,
        default: u16,
    },

    /// Perform local inference: dst_reg, model_reg, prompt_reg.
    LocalInfer(u8, u8, u8),

    /// Return from function.
    Return,

    /// Print top of stack.
    Println,

    /// Halt execution.
    Halt,
}

/// A compiled L-Agent program: magic bytes + version + instructions.
#[derive(Debug, Serialize, Deserialize)]
pub struct Bytecode {
    pub magic: [u8; 4],    // b"LAGN"
    pub version: u16,
    pub instructions: Vec<OpCode>,
}

impl Bytecode {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        Self {
            magic: *b"LAGN",
            version: 1,
            instructions,
        }
    }
}
