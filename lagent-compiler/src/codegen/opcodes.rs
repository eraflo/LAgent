// SPDX-License-Identifier: Apache-2.0
//! L-Agent bytecode instruction set and serialisable [`Bytecode`] container.

use serde::{Deserialize, Serialize};

/// L-Agent bytecode instruction set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OpCode {
    // ── Literals ──────────────────────────────────────────────────────────────
    /// Push a string literal onto the stack.
    PushStr(String),
    /// Push an unsigned 64-bit integer literal onto the stack.
    PushInt(u64),
    /// Push a 64-bit float literal onto the stack.
    PushFloat(f64),

    // ── Local variables ───────────────────────────────────────────────────────
    /// Pop TOS and store it in a named local variable slot.
    StoreLocal(String),
    /// Push the value of a named local variable onto the stack.
    LoadLocal(String),

    // ── Context primitives ────────────────────────────────────────────────────
    /// Allocate a context segment of `n` tokens; push a `CtxHandle`.
    CtxAlloc(u32),
    /// Pop a `CtxHandle` and free the segment.
    CtxFreeStack,
    /// Pop a string (TOS) then a `CtxHandle` (next) and append the string.
    CtxAppendStack,
    /// Pop a `CtxHandle`, compress the segment via the inference backend, and
    /// replace the segment content with the compressed result.
    CtxCompress,
    /// Resize the context segment at the given register index.
    CtxResize(u8, u32),

    // ── Control flow ──────────────────────────────────────────────────────────
    /// Return from a function or kernel; TOS becomes the return value.
    Return,
    /// Halt the top-level program.
    Halt,

    // ── Call frames ───────────────────────────────────────────────────────────
    /// Call a kernel by index into [`Bytecode::kernels`].
    /// Arguments must be pushed onto the stack in declaration order before this
    /// instruction; the kernel body receives them as named locals.
    CallKernel(u16),

    // ── I/O ───────────────────────────────────────────────────────────────────
    /// Pop TOS and print it to stdout.
    Println,

    // ── Probabilistic branching ───────────────────────────────────────────────
    /// Classify the subject `var` against the case labels, then execute the
    /// first case body whose label confidence ≥ its threshold, or `default`.
    BranchClassify {
        /// Local variable name or built-in (e.g. `"intent"`) to classify.
        var: String,
        /// `(label, confidence_threshold, body)` triples — checked in order.
        cases: Vec<(String, f32, Vec<OpCode>)>,
        /// Body to execute when no case matches.
        default: Vec<OpCode>,
    },

    // ── Inference ─────────────────────────────────────────────────────────────
    /// Classify top-of-stack prompt against `labels`; push the winning label.
    InferClassify(Vec<String>),

    // ── Kernel step primitives ────────────────────────────────────────────────
    /// Pop TOS (observation payload) — forwarded to the inference backend.
    Observe,
    /// Emit a reasoning annotation; no-op in the simulated backend.
    Reason(String),
    /// Pop TOS (action payload) and execute an action via the inference backend.
    Act,
    /// Pop TOS; if falsy (`Int(0)` or empty `Str`) raise a `VerifyFail` signal
    /// so the enclosing kernel retries the attempt.
    VerifyStep,

    // ── Interruptible blocks ──────────────────────────────────────────────────
    /// Save a snapshot of the current frame as a Safe Interaction Point.
    BeginInterruptible,
    /// Clear the saved checkpoint after the interruptible block completes.
    EndInterruptible,

    // ── Comparisons (pop rhs then lhs, push Int(1) if true else Int(0)) ──────
    /// `Int(1)` if lhs == rhs, else `Int(0)`.
    CmpEq,
    /// `Int(1)` if lhs != rhs, else `Int(0)`.
    CmpNotEq,
    /// `Int(1)` if lhs > rhs, else `Int(0)`.
    CmpGt,
    /// `Int(1)` if lhs < rhs, else `Int(0)`.
    CmpLt,

    // ── Phase 4: agent vocabulary ─────────────────────────────────────────────
    /// Store the agent soul identity string in the VM for introspection.
    SetAgentMeta(String),
    /// Append a literal string to the context handle named `ctx` in scope, if any.
    CtxAppendLiteral(String),
    /// Record a skill name in VM metadata (no runtime effect).
    RegisterSkill(String),
    /// Pop TOS (initial value) and allocate a named persistent memory slot.
    AllocMemorySlot(String),
    /// Push the current value of a named persistent memory slot onto the stack.
    LoadMemory(String),
    /// Pop TOS and store it in a named persistent memory slot.
    StoreMemory(String),
    /// Pop N args from the stack and call a named oracle; push the result string.
    CallOracle(String, u8),
    /// Mark the start of a named constraint block (for diagnostics).
    BeginConstraint(String),
    /// Mark the end of a constraint block.
    EndConstraint,
    /// Store a lore entry (`name → text`) in the VM lore table.
    StoreLore(String, String),
    /// Push a lore string by name onto the stack.
    LoadLore(String),
    /// Duplicate the top-of-stack context handle (both references the same segment).
    CtxShare,
}

/// Per-kernel compiled bytecode stored alongside the main instruction stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelBytecode {
    /// Kernel name (used for diagnostics).
    pub name: String,
    /// Parameter names in declaration order — bound from the call-site stack.
    pub params: Vec<String>,
    /// The kernel's instruction body.
    pub body: Vec<OpCode>,
    /// Maximum number of retry attempts when `VerifyStep` fails (default: 3).
    pub max_retries: u8,
}

/// A compiled L-Agent program: magic bytes + version + kernel table + instructions.
#[derive(Debug, Serialize, Deserialize)]
pub struct Bytecode {
    /// Magic header — always `b"LAGN"`.
    pub magic: [u8; 4],
    /// Bytecode format version.
    pub version: u16,
    /// Compiled kernel definitions, indexed by [`OpCode::CallKernel`].
    pub kernels: Vec<KernelBytecode>,
    /// The main instruction stream (entry point).
    pub instructions: Vec<OpCode>,
}

impl Bytecode {
    /// Create a new [`Bytecode`] with the standard magic header and version 1.
    #[must_use]
    pub fn new(kernels: Vec<KernelBytecode>, instructions: Vec<OpCode>) -> Self {
        Self {
            magic: *b"LAGN",
            version: 1,
            kernels,
            instructions,
        }
    }
}
