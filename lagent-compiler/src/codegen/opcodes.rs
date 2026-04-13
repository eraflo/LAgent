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
    /// Unconditional jump to the given instruction index.
    Jump(usize),
    /// Jump to the given instruction index if TOS is Int(0) (false).
    JumpIfFalse(usize),

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

    // ── Phase 7: Arithmetic operators ────────────────────────────────────────
    /// Pop rhs and lhs, push lhs + rhs.
    Add,
    /// Pop rhs and lhs, push lhs - rhs.
    Sub,
    /// Pop rhs and lhs, push lhs * rhs.
    Mul,
    /// Pop rhs and lhs, push lhs / rhs.
    Div,
    /// Pop rhs and lhs, push lhs % rhs.
    Mod,

    // ── Phase 7: Logical operators ───────────────────────────────────────────
    /// Pop rhs and lhs, push lhs && rhs (Int(1) if both non-zero, else Int(0)).
    And,
    /// Pop rhs and lhs, push lhs || rhs (Int(1) if either non-zero, else Int(0)).
    Or,

    // ── Phase 7: Boolean and control flow ────────────────────────────────────
    /// Push a boolean value (Int(1) for true, Int(0) for false) onto the stack.
    PushBool(bool),
    /// Pop N values from the stack and pack them into a single tuple value.
    TuplePack(u8),

    /// ── Phase 8: Vector operations ──────────────────────────────────────────
    /// Create a Vec from the top N values on the stack.
    VecNew(u8),
    /// Pop index (TOS) and vec ref (next), push element at index.
    VecGet,
    /// Pop index (TOS), vec ref (next), and value (next); store value at index.
    VecSet,
    /// Pop vec ref (TOS), push its length as Int.
    VecLen,
    /// Pop value (TOS) and vec ref (next); push value onto vec.
    VecPush,

    // ── Phase 7: Tuple/struct field access ───────────────────────────────
    /// Pop struct/tuple (TOS), push the named field's value.
    FieldAccess(String),

    // ── Phase 8: Struct/Enum construction ───────────────────────────────
    /// Pop N field values (in declaration order) and pack into a struct value.
    StructConstruct {
        name: String,
        field_names: Vec<String>,
    },
    /// Push an enum variant value.
    EnumVariant { variant: String, payload: bool },

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
    /// Pop TOS; if falsy, immediately abort with a non-retriable `ConstraintViolation` error.
    /// Distinct from `VerifyStep` which triggers kernel retry logic.
    ConstraintVerify,

    // ── Phase 5: persistent memory ────────────────────────────────────────────
    /// Pop TOS (key string); push the persisted value string, or empty str if absent.
    PersistLoad,
    /// Pop TOS (value string) then next (key string); persist the key-value pair.
    PersistSave,
    /// Pop TOS (key string); delete that key from the persistent store.
    PersistDelete,
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

// ── Phase 5: Library Bundle (.lalb) ──────────────────────────────────────────

/// Kind of an exported item in a `.lalb` library bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportKind {
    /// A callable kernel, spell, or skill.
    Kernel,
    /// A static lore string.
    Lore,
    /// An oracle declaration stub.
    Oracle,
}

/// A single export entry in a `.lalb` library bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEntry {
    /// Exported item name.
    pub name: String,
    /// Kind of export.
    pub kind: ExportKind,
    /// Index into `LibraryBundle::bytecode::kernels` for callable exports;
    /// `u16::MAX` for non-kernel exports.
    pub kernel_idx: u16,
}

/// A precompiled L-Agent library bundle (`.lalb`).
///
/// Contains the same executable bytecode as `.lbc` plus an export table that
/// describes which items are visible to importing programs.
#[derive(Debug, Serialize, Deserialize)]
pub struct LibraryBundle {
    /// Magic header — always `b"LALB"`.
    pub magic: [u8; 4],
    /// Bundle format version.
    pub version: u16,
    /// Library name (from `lagent.toml` or CLI flag).
    pub name: String,
    /// The compiled bytecode (kernels + instructions).
    pub bytecode: Bytecode,
    /// Export table: only `pub` items appear here.
    pub exports: Vec<ExportEntry>,
}

impl LibraryBundle {
    /// Create a new [`LibraryBundle`] with the standard magic header and version 1.
    #[must_use]
    pub fn new(name: String, bytecode: Bytecode, exports: Vec<ExportEntry>) -> Self {
        Self {
            magic: *b"LALB",
            version: 1,
            name,
            bytecode,
            exports,
        }
    }
}
