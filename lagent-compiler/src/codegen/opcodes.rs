// SPDX-License-Identifier: Apache-2.0
//! L-Agent bytecode instruction set and serialisable [`Bytecode`] container.

use serde::{Deserialize, Serialize};

/// L-Agent bytecode instruction set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Probabilistic branch over inference output.
    Branch {
        /// Per-case `(label, confidence_threshold, jump_offset)` triples.
        cases: Vec<(String, f32, u16)>,
        /// Instruction offset to jump to when no case matches.
        default: u16,
    },

    /// Perform local inference (`dst_reg` ← model at `model_reg` with prompt at `prompt_reg`).
    LocalInfer(u8, u8, u8),

    // ── Stack-based local variable access (Phase 1) ────────────────────────
    /// Pop value from stack and store it in a named local variable.
    StoreLocal(String),
    /// Push the value of a named local variable onto the stack.
    LoadLocal(String),

    // ── Stack-based context primitives (Phase 1) ───────────────────────────
    /// Pop a context handle from the stack and free the segment.
    CtxFreeStack,
    /// Pop a string (top) then a context handle (next) and append the string
    /// to the segment.
    CtxAppendStack,

    // ── Phase 2: probabilistic branching ──────────────────────────────────
    /// Interpreter-style probabilistic branch.
    ///
    /// The VM calls `backend.classify(var, labels)`, then executes the first
    /// case body whose label has confidence ≥ its threshold, or `default`.
    BranchClassify {
        /// Local variable name or built-in (e.g. `"intent"`) to classify.
        var: String,
        /// `(label, confidence_threshold, body_instructions)` triples checked in order.
        cases: Vec<(String, f32, Vec<OpCode>)>,
        /// Body to execute when no case matches.
        default: Vec<OpCode>,
    },

    // ── Phase 2: kernel primitives ─────────────────────────────────────────
    /// Pop a string from the stack and append it to the kernel's context buffer.
    Observe,
    /// Log a reasoning annotation string (no-op in simulated backend).
    Reason(String),
    /// Classify the top-of-stack prompt against `labels` using the inference
    /// backend; push the winning label as a [`Value::Str`].
    InferClassify(Vec<String>),

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
    /// Magic header — always `b"LAGN"`.
    pub magic: [u8; 4],
    /// Bytecode format version.
    pub version: u16,
    /// The instruction stream.
    pub instructions: Vec<OpCode>,
}

impl Bytecode {
    /// Create a new [`Bytecode`] with the standard magic header and version 1.
    #[must_use]
    pub fn new(instructions: Vec<OpCode>) -> Self {
        Self {
            magic: *b"LAGN",
            version: 1,
            instructions,
        }
    }
}
