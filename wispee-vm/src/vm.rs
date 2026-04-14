// SPDX-License-Identifier: Apache-2.0
//! Stack-based Wispee virtual machine and runtime [`Value`] type.

use crate::backends::InferenceBackend;
use crate::persistent_store::PersistentStore;
use crate::runtime::TokenHeap;
use anyhow::{anyhow, Result};
use wispee_compiler::codegen::opcodes::{Bytecode, OpCode};
use std::collections::HashMap;

// ── Runtime value type ────────────────────────────────────────────────────────

/// A value on the VM stack or in a local variable slot.
#[derive(Debug, Clone)]
pub enum Value {
    /// UTF-8 string.
    Str(String),
    /// Unsigned 64-bit integer.
    Int(u64),
    /// 64-bit float.
    Float(f64),
    /// Boolean value (stored as Int(1) for true, Int(0) for false).
    Bool(bool),
    /// Handle to an allocated context segment in the [`TokenHeap`].
    CtxHandle(u32),
    /// ── Phase 7: Tuple ──────────────────────────────────────────────────
    /// Tuple value — ordered collection of heterogeneous values.
    Tuple(Vec<Value>),
    /// ── Phase 8: Struct/Enum ────────────────────────────────────────────
    /// Struct value — named fields.
    Struct {
        name: String,
        fields: std::collections::HashMap<String, Value>,
    },
    /// Enum variant — named variant with optional payload.
    EnumVariant {
        variant: String,
        payload: Option<Box<Value>>,
    },
    /// ── Phase 8: Vector ─────────────────────────────────────────────────
    /// Dynamic array of values (stored on heap via Rc for shared references).
    Vec(std::rc::Rc<std::cell::RefCell<Vec<Value>>>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{s}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::CtxHandle(h) => write!(f, "<ctx#{h}>"),
            Value::EnumVariant { variant, payload } => {
                write!(f, "{variant}")?;
                if let Some(p) = payload {
                    write!(f, "({p})")?;
                }
                Ok(())
            }
            Value::Struct { name, fields } => {
                write!(f, "{name} {{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, val) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{val}")?;
                }
                write!(f, ")")
            }
            Value::Vec(v) => {
                let inner = v.borrow();
                write!(f, "[")?;
                for (i, val) in inner.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{val}")?;
                }
                write!(f, "]")
            }
        }
    }
}

// ── Virtual Machine ───────────────────────────────────────────────────────────

/// The Wispee Virtual Machine.
pub struct Vm {
    heap: TokenHeap,
    backend: Box<dyn InferenceBackend>,
    /// Soul identity metadata set by `SetAgentMeta`.
    soul_meta: Option<String>,
    /// Persistent memory slots allocated by `AllocMemorySlot`.
    memory: HashMap<String, Value>,
    /// Lore table populated by `StoreLore`.
    lore: HashMap<String, String>,
    /// Optional file-backed persistent store for `memory_load/save/delete`.
    persistent: Option<Box<dyn PersistentStore>>,
}

impl Vm {
    /// Create a new VM with the given context heap capacity and inference backend.
    pub fn new(heap_capacity: usize, backend: Box<dyn InferenceBackend>) -> Self {
        Self {
            heap: TokenHeap::new(heap_capacity),
            backend,
            soul_meta: None,
            memory: HashMap::new(),
            lore: HashMap::new(),
            persistent: None,
        }
    }

    /// Attach a persistent store to the VM for cross-run memory operations.
    #[must_use]
    pub fn with_persistent_store(mut self, store: Box<dyn PersistentStore>) -> Self {
        self.persistent = Some(store);
        self
    }

    /// Execute raw bytecode bytes produced by the compiler.
    ///
    /// # Errors
    ///
    /// Returns an error on invalid bytecode, stack underflow, or a runtime fault.
    pub fn execute(&mut self, bytecode: &[u8]) -> Result<()> {
        let bc: Bytecode = bincode::deserialize(bytecode)
            .map_err(|e| anyhow!("bytecode deserialization failed: {e}"))?;

        let mut frame = Frame::default();
        self.run(&bc, &bc.instructions, &mut frame)
    }

    #[allow(clippy::too_many_lines)]
    fn run(&mut self, bc: &Bytecode, ops: &[OpCode], frame: &mut Frame) -> Result<()> {
        let mut pc = 0; // program counter
        while pc < ops.len() {
            let op = &ops[pc];
            match op {
                // ── Literals ──────────────────────────────────────────────
                OpCode::PushStr(s) => frame.push(Value::Str(s.clone())),
                OpCode::PushInt(n) => frame.push(Value::Int(*n)),
                OpCode::PushFloat(f) => frame.push(Value::Float(*f)),
                OpCode::PushBool(b) => frame.push(Value::Bool(*b)),

                // ── Locals ────────────────────────────────────────────────
                OpCode::StoreLocal(name) => {
                    let val = frame.pop()?;
                    frame.locals.insert(name.clone(), val);
                }
                OpCode::LoadLocal(name) => {
                    // Check memory slots as a fallback for Ident-based access.
                    let val = if let Some(v) = frame.locals.get(name) {
                        v.clone()
                    } else if let Some(v) = self.memory.get(name) {
                        v.clone()
                    } else {
                        return Err(anyhow!("undefined local: `{name}`"));
                    };
                    frame.push(val);
                }

                // ── Context primitives ────────────────────────────────────
                OpCode::CtxAlloc(tokens) => {
                    let id = self.heap.alloc(*tokens as usize)?;
                    frame.push(Value::CtxHandle(id));
                }
                OpCode::CtxFreeStack => {
                    let handle = frame.pop_ctx_handle()?;
                    self.heap.free(handle)?;
                }
                OpCode::CtxAppendStack => {
                    let text = frame.pop_str()?;
                    let handle = frame.pop_ctx_handle()?;
                    self.heap.append(handle, &text)?;
                }
                OpCode::CtxCompress => {
                    let handle = frame.pop_ctx_handle()?;
                    let content = self.heap.get_content(handle)?;
                    let compressed = self.backend.compress(&content)?;
                    self.heap.set_content(handle, compressed)?;
                }
                OpCode::CtxShare => {
                    // Duplicate TOS ctx handle — both references refer to the same segment.
                    let handle = frame
                        .stack
                        .last()
                        .cloned()
                        .ok_or_else(|| anyhow!("stack underflow"))?;
                    frame.push(handle);
                }
                // No-ops: register-indexed resize, reason annotation, skill registration,
                // and constraint boundary markers (diagnostic only).
                OpCode::CtxResize(_, _)
                | OpCode::Reason(_)
                | OpCode::RegisterSkill(_)
                | OpCode::BeginConstraint(_)
                | OpCode::EndConstraint => {}

                // ── Constraint verification (non-retriable) ───────────────
                OpCode::ConstraintVerify => {
                    let val = frame.pop()?;
                    let ok = match &val {
                        Value::Int(0) => false,
                        Value::Str(s) if s.is_empty() => false,
                        _ => true,
                    };
                    if !ok {
                        return Err(anyhow!("ConstraintViolation"));
                    }
                }

                // ── Persistent memory ─────────────────────────────────────
                OpCode::PersistLoad => {
                    let key = frame.pop_str()?;
                    let val = self
                        .persistent
                        .as_ref()
                        .and_then(|s| s.load(&key))
                        .unwrap_or_default();
                    frame.push(Value::Str(val));
                }
                OpCode::PersistSave => {
                    let value = frame.pop_str()?;
                    let key = frame.pop_str()?;
                    if let Some(store) = &mut self.persistent {
                        store.save(&key, &value);
                    }
                }
                OpCode::PersistDelete => {
                    let key = frame.pop_str()?;
                    if let Some(store) = &mut self.persistent {
                        store.delete(&key);
                    }
                }

                // ── I/O ───────────────────────────────────────────────────
                OpCode::Println => {
                    let val = frame.pop()?;
                    println!("{val}");
                }

                // ── Control flow ──────────────────────────────────────────
                OpCode::Return | OpCode::Halt => return Ok(()),
                OpCode::Jump(target) => {
                    pc = *target;
                    continue;
                }
                OpCode::JumpIfFalse(target) => {
                    let val = frame.pop()?;
                    let is_false = match val {
                        Value::Int(0) | Value::Bool(false) => true,
                        Value::Str(ref s) if s.is_empty() => true,
                        _ => false,
                    };
                    if is_false {
                        pc = *target;
                        continue;
                    }
                }

                // ── Call frames ───────────────────────────────────────────
                OpCode::CallKernel(idx) => {
                    let kernel = bc
                        .kernels
                        .get(*idx as usize)
                        .ok_or_else(|| anyhow!("invalid kernel index {idx}"))?;

                    // Pop args (pushed in declaration order; last param on TOS).
                    let mut child = Frame::default();
                    for param in kernel.params.iter().rev() {
                        let val = frame.pop()?;
                        child.locals.insert(param.clone(), val);
                    }

                    // Retry loop: re-run the kernel body on VerifyFail.
                    let mut last_err: Option<anyhow::Error> = None;
                    for _ in 0..=kernel.max_retries {
                        let mut attempt = child.clone();
                        match self.run(bc, &kernel.body, &mut attempt) {
                            Ok(()) => {
                                if let Some(v) = attempt.stack.pop() {
                                    frame.push(v);
                                }
                                last_err = None;
                                break;
                            }
                            Err(e) if e.to_string().contains("VerifyFail") => {
                                last_err = Some(e);
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    if let Some(e) = last_err {
                        return Err(e);
                    }
                }

                // ── Probabilistic branching ───────────────────────────────
                OpCode::InferClassify(labels) => {
                    let prompt = frame.pop_str().unwrap_or_default();
                    let results = self.backend.classify(&prompt, labels)?;
                    let winner = results
                        .into_iter()
                        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .map_or_else(String::new, |(label, _)| label);
                    frame.push(Value::Str(winner));
                }

                OpCode::BranchClassify {
                    var,
                    cases,
                    default,
                } => {
                    let all_labels: Vec<String> = cases.iter().map(|(l, _, _)| l.clone()).collect();
                    let prompt = frame
                        .locals
                        .get(var)
                        .map_or_else(|| var.clone(), Value::to_string);
                    let scores = self.backend.classify(&prompt, &all_labels)?;

                    // Discard the InferClassify result pushed just before this opcode.
                    let _ = frame.pop();

                    let mut matched = false;
                    for (label, threshold, body) in cases {
                        let confidence = scores
                            .iter()
                            .find(|(l, _)| l == label)
                            .map_or(0.0, |(_, c)| *c);
                        if confidence >= *threshold {
                            self.run(bc, body, frame)?;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        self.run(bc, default, frame)?;
                    }
                }

                // ── Kernel step primitives ────────────────────────────────
                OpCode::Observe => {
                    let _ = frame.pop(); // Observation payload — no-op in simulated backend.
                }
                OpCode::Act => {
                    let payload = frame.pop_str().unwrap_or_default();
                    let _ = self.backend.act(&payload)?;
                }
                OpCode::VerifyStep => {
                    let val = frame.pop()?;
                    let ok = match &val {
                        Value::Int(0) => false,
                        Value::Str(s) if s.is_empty() => false,
                        _ => true,
                    };
                    if !ok {
                        return Err(anyhow!("VerifyFail"));
                    }
                }

                // ── Comparisons ───────────────────────────────────────────
                OpCode::CmpEq | OpCode::CmpNotEq | OpCode::CmpGt | OpCode::CmpLt => {
                    let rhs = frame.pop()?;
                    let lhs = frame.pop()?;
                    let result = match op {
                        OpCode::CmpEq => values_eq(&lhs, &rhs),
                        OpCode::CmpNotEq => !values_eq(&lhs, &rhs),
                        OpCode::CmpGt => values_gt(&lhs, &rhs),
                        OpCode::CmpLt => values_gt(&rhs, &lhs),
                        _ => unreachable!(),
                    };
                    frame.push(Value::Int(u64::from(result)));
                }

                // ── Phase 7: Arithmetic operators ─────────────────────────
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::Mod => {
                    let rhs = frame.pop()?;
                    let lhs = frame.pop()?;
                    let result = match (op, &lhs, &rhs) {
                        (OpCode::Add, Value::Int(a), Value::Int(b)) => Value::Int(a + b),
                        (OpCode::Sub, Value::Int(a), Value::Int(b)) => Value::Int(a - b),
                        (OpCode::Mul, Value::Int(a), Value::Int(b)) => Value::Int(a * b),
                        (OpCode::Div, Value::Int(a), Value::Int(b)) => {
                            if *b == 0 {
                                return Err(anyhow!("division by zero"));
                            }
                            Value::Int(a / b)
                        }
                        (OpCode::Mod, Value::Int(a), Value::Int(b)) => {
                            if *b == 0 {
                                return Err(anyhow!("modulo by zero"));
                            }
                            Value::Int(a % b)
                        }
                        (OpCode::Add, Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                        (OpCode::Sub, Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                        (OpCode::Mul, Value::Float(a), Value::Float(b)) => Value::Float(a * b),
                        (OpCode::Div, Value::Float(a), Value::Float(b)) => Value::Float(a / b),
                        (OpCode::Mod, Value::Float(a), Value::Float(b)) => Value::Float(a % b),
                        _ => {
                            return Err(anyhow!(
                                "invalid operands for arithmetic: {lhs:?} {op:?} {rhs:?}"
                            ));
                        }
                    };
                    frame.push(result);
                }

                // ── Phase 7: Logical operators ────────────────────────────
                OpCode::And | OpCode::Or => {
                    let rhs = frame.pop()?;
                    let lhs = frame.pop()?;
                    let result = match (op, &lhs, &rhs) {
                        (OpCode::And, a, b) => {
                            let a_bool = value_to_bool(a);
                            let b_bool = value_to_bool(b);
                            Value::Bool(a_bool && b_bool)
                        }
                        (OpCode::Or, a, b) => {
                            let a_bool = value_to_bool(a);
                            let b_bool = value_to_bool(b);
                            Value::Bool(a_bool || b_bool)
                        }
                        _ => unreachable!(),
                    };
                    frame.push(result);
                }

                // ── Interruptible blocks ──────────────────────────────────
                OpCode::BeginInterruptible => {
                    frame.checkpoint = Some(Box::new(FrameState {
                        stack: frame.stack.clone(),
                        locals: frame.locals.clone(),
                    }));
                }
                OpCode::EndInterruptible => {
                    frame.checkpoint = None;
                }

                // ── Phase 4: agent vocabulary ─────────────────────────────
                OpCode::SetAgentMeta(s) => {
                    self.soul_meta = Some(s.clone());
                }

                OpCode::CtxAppendLiteral(s) => {
                    // Append to the ctx handle stored in the current frame locals, if any.
                    if let Some(Value::CtxHandle(id)) = frame.locals.get("ctx").cloned() {
                        self.heap.append(id, s)?;
                    }
                    // No-op when no ctx is in scope yet (e.g. soul fires before ctx_alloc).
                }

                OpCode::AllocMemorySlot(name) | OpCode::StoreMemory(name) => {
                    let val = frame.pop()?;
                    self.memory.insert(name.clone(), val);
                }

                OpCode::LoadMemory(name) => {
                    let val = self
                        .memory
                        .get(name)
                        .cloned()
                        .unwrap_or(Value::Str(String::new()));
                    frame.push(val);
                }

                OpCode::CallOracle(name, arity) => {
                    let mut arg_strs: Vec<String> = Vec::new();
                    for _ in 0..*arity {
                        let v = frame.pop()?;
                        arg_strs.push(v.to_string());
                    }
                    arg_strs.reverse();
                    let result = self.backend.oracle(name, &arg_strs)?;
                    frame.push(Value::Str(result));
                }

                // ── Lore ──────────────────────────────────────────────────
                OpCode::StoreLore(name, text) => {
                    self.lore.insert(name.clone(), text.clone());
                }
                OpCode::LoadLore(name) => {
                    let val = self.lore.get(name).cloned().unwrap_or_default();
                    frame.push(Value::Str(val));
                }

                // ── Phase 7: Tuple ──────────────────────────────────────
                OpCode::TuplePack(n) => {
                    let mut items = Vec::with_capacity(*n as usize);
                    for _ in 0..*n {
                        items.push(frame.pop()?);
                    }
                    items.reverse();
                    frame.push(Value::Tuple(items));
                }
                OpCode::FieldAccess(field) => {
                    let val = frame.pop()?;
                    match val {
                        Value::Tuple(items) => {
                            if let Ok(idx) = field.parse::<usize>() {
                                let item = items.get(idx).cloned().ok_or_else(|| {
                                    anyhow!("tuple index {idx} out of bounds (len={})", items.len())
                                })?;
                                frame.push(item);
                            } else {
                                return Err(anyhow!("cannot access field `{field}` on tuple"));
                            }
                        }
                        Value::Struct { name, fields } => {
                            let field_val = fields
                                .get(field)
                                .cloned()
                                .ok_or_else(|| anyhow!("struct `{name}` has no field `{field}`"))?;
                            frame.push(field_val);
                        }
                        _ => {
                            return Err(anyhow!(
                                "cannot access field `{field}` on non-struct/tuple value"
                            ));
                        }
                    }
                }

                // ── Phase 8: Struct/Enum construction ────────────────────
                OpCode::StructConstruct { name, field_names } => {
                    let mut fields = std::collections::HashMap::new();
                    for field_name in field_names.iter().rev() {
                        fields.insert(field_name.clone(), frame.pop()?);
                    }
                    frame.push(Value::Struct {
                        name: name.clone(),
                        fields,
                    });
                }
                OpCode::EnumVariant { variant, payload } => {
                    let val = if *payload {
                        Some(Box::new(frame.pop()?))
                    } else {
                        None
                    };
                    frame.push(Value::EnumVariant {
                        variant: variant.clone(),
                        payload: val,
                    });
                }

                // ── Phase 8: Vector operations ────────────────────────────
                OpCode::VecNew(n) => {
                    let mut vec = Vec::with_capacity(*n as usize);
                    for _ in 0..*n {
                        vec.push(frame.pop()?);
                    }
                    vec.reverse(); // was pushed in reverse order
                    frame.push(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec))));
                }
                OpCode::VecGet => {
                    let idx_val = frame.pop()?;
                    let vec_val = frame.pop()?;
                    if let Value::Vec(rc) = vec_val {
                        #[allow(clippy::cast_possible_truncation)]
                        let idx = match idx_val {
                            Value::Int(i) => i as usize,
                            _ => return Err(anyhow!("vector index must be an integer")),
                        };
                        let vec = rc.borrow();
                        let val = vec.get(idx).cloned().ok_or_else(|| {
                            anyhow!("vector index {idx} out of bounds (len={})", vec.len())
                        })?;
                        frame.push(val);
                    } else {
                        return Err(anyhow!("cannot index non-vector value"));
                    }
                }
                OpCode::VecSet => {
                    let val = frame.pop()?;
                    let idx_val = frame.pop()?;
                    let vec_val = frame.pop()?;
                    if let Value::Vec(rc) = vec_val {
                        #[allow(clippy::cast_possible_truncation)]
                        let idx = match idx_val {
                            Value::Int(i) => i as usize,
                            _ => return Err(anyhow!("vector index must be an integer")),
                        };
                        let mut vec = rc.borrow_mut();
                        if idx >= vec.len() {
                            return Err(anyhow!(
                                "vector index {idx} out of bounds (len={})",
                                vec.len()
                            ));
                        }
                        vec[idx] = val;
                    } else {
                        return Err(anyhow!("cannot index non-vector value"));
                    }
                }
                OpCode::VecLen => {
                    let vec_val = frame.pop()?;
                    if let Value::Vec(rc) = vec_val {
                        let len = rc.borrow().len();
                        frame.push(Value::Int(len as u64));
                    } else {
                        return Err(anyhow!("cannot get length of non-vector value"));
                    }
                }
                OpCode::VecPush => {
                    let val = frame.pop()?;
                    let vec_val = frame.pop()?;
                    if let Value::Vec(rc) = vec_val {
                        rc.borrow_mut().push(val);
                    } else {
                        return Err(anyhow!("cannot push to non-vector value"));
                    }
                }
            }
            pc += 1;
        }
        Ok(())
    }
}

// ── Value comparison helpers ──────────────────────────────────────────────────

/// Convert a [`Value`] to a boolean.
fn value_to_bool(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Str(s) => !s.is_empty(),
        _ => false,
    }
}

fn values_eq(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::CtxHandle(a), Value::CtxHandle(b)) => a == b,
        _ => false,
    }
}

fn values_gt(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => a > b,
        (Value::Float(a), Value::Float(b)) => a > b,
        (Value::Str(a), Value::Str(b)) => a > b,
        _ => false,
    }
}

// ── Call frame ────────────────────────────────────────────────────────────────

/// Snapshot of stack + locals for interruptible-block checkpointing.
#[derive(Debug, Clone)]
struct FrameState {
    #[allow(dead_code)]
    stack: Vec<Value>,
    #[allow(dead_code)]
    locals: HashMap<String, Value>,
}

#[derive(Default, Clone)]
struct Frame {
    stack: Vec<Value>,
    locals: HashMap<String, Value>,
    /// Saved checkpoint at the last `BeginInterruptible` (cleared at `EndInterruptible`).
    checkpoint: Option<Box<FrameState>>,
}

impl Frame {
    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Result<Value> {
        self.stack.pop().ok_or_else(|| anyhow!("stack underflow"))
    }

    fn pop_str(&mut self) -> Result<String> {
        match self.pop()? {
            Value::Str(s) => Ok(s),
            other => Err(anyhow!("expected Str on stack, got {other:?}")),
        }
    }

    fn pop_ctx_handle(&mut self) -> Result<u32> {
        match self.pop()? {
            Value::CtxHandle(h) => Ok(h),
            other => Err(anyhow!("expected CtxHandle on stack, got {other:?}")),
        }
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::SimulatedBackend;

    fn make_vm() -> Vm {
        Vm::new(4096, Box::new(SimulatedBackend::new("ok")))
    }

    #[test]
    fn executes_println() {
        let bc = Bytecode::new(
            vec![],
            vec![
                OpCode::PushStr("hello from vm".to_string()),
                OpCode::Println,
                OpCode::Halt,
            ],
        );
        let bytes = bincode::serialize(&bc).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn executes_hello_la() {
        let src = r#"
fn main() {
    let ctx = ctx_alloc(512);
    ctx_append(ctx, "Hello, L-Agent!");
    println("Hello, L-Agent!");
    ctx_free(ctx);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn ctx_alloc_and_free_balance_heap() {
        let bc = Bytecode::new(
            vec![],
            vec![OpCode::CtxAlloc(256), OpCode::CtxFreeStack, OpCode::Halt],
        );
        let bytes = bincode::serialize(&bc).unwrap();
        let mut vm = make_vm();
        vm.execute(&bytes).unwrap();
        assert_eq!(vm.heap.used(), 0);
    }

    #[test]
    fn executes_emotion_analysis() {
        let src = r#"
type Emotion = semantic("joie", "colère", "tristesse", "neutre");

kernel AnalyserMessage(texte: str) -> Emotion {
    observe(texte);
    reason("Déterminer l'émotion dominante");
    let emotion: Emotion = infer(texte);
    verify(emotion != "neutre");
    return emotion;
}

fn main() {
    let ctx = ctx_alloc(4096);
    ctx_append(ctx, "Je suis très mécontent !");

    branch intent {
        case "angry" (confidence > 0.7) => {
            println("Gestion de crise activée");
        }
        case "help" (confidence > 0.4) => {
            println("Support standard");
        }
        default => {
            println("Redirection vers un opérateur humain");
        }
    }

    ctx_free(ctx);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn executes_kernel_call() {
        let src = r#"
type Sentiment = semantic("positive", "negative", "neutral");

kernel AnalyseSentiment(text: str) -> Sentiment {
    observe(text);
    reason("Classify the sentiment of the input text");
    let result: Sentiment = infer(text);
    verify(result != "");
    return result;
}

fn main() {
    let ctx = ctx_alloc(1024);
    ctx_append(ctx, "This product is absolutely fantastic!");
    let sentiment = AnalyseSentiment(ctx);
    println(sentiment);
    ctx_free(ctx);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn verify_retry_exhaustion_returns_error() {
        let src = r#"
kernel AlwaysFail(x: str) -> str {
    verify(0);
    return x;
}
fn main() {
    let result = AlwaysFail("test");
    println(result);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        let err = make_vm().execute(&bytes).unwrap_err();
        assert!(err.to_string().contains("VerifyFail"));
    }

    #[test]
    fn ctx_compress_replaces_content() {
        let src = r#"
fn main() {
    let ctx = ctx_alloc(1024);
    ctx_append(ctx, "Hello world this is a long string");
    ctx_compress(ctx);
    ctx_free(ctx);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn interruptible_block_executes_normally() {
        let src = r#"
fn main() {
    interruptible {
        println("inside interruptible");
    }
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn branch_default_fires_when_no_case_matches() {
        let src = r#"
fn main() {
    branch intent {
        case "angry" (confidence > 0.9) => {
            println("crise");
        }
        case "help" (confidence > 0.9) => {
            println("support");
        }
        default => {
            println("default");
        }
    }
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn executes_oracle_call() {
        let src = r#"
oracle FetchContext(url: str) -> str;
fn main() {
    let r = FetchContext("http://example.com");
    println(r);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn memory_slot_persists() {
        let src = r#"
memory Counter: str = "initial";
fn main() {
    println(Counter);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn executes_agent_soul() {
        let src = r#"
type Mood = semantic("happy", "sad", "neutral");

soul {
    instruction "You are a helpful sentiment analysis agent.";
}

lore Background = "This agent analyses user-provided text.";

memory LastResult: str = "";

oracle FetchContext(url: str) -> str;

skill AnalyseMood(text: str) -> Mood {
    observe(text);
    reason("Classify the mood of the text");
    let result: Mood = infer(text);
    verify(result != "");
    return result;
}

fn main() {
    let ctx = ctx_alloc(1024);
    ctx_append(ctx, "I love this project, it is amazing!");
    let mood = AnalyseMood(ctx);
    println(mood);
    ctx_free(ctx);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    // ── Phase 5 tests ─────────────────────────────────────────────────────────

    #[test]
    fn constraint_violation_is_non_retriable() {
        // A constraint violation must NOT retry; it propagates immediately.
        let src = r"
constraint NonEmpty {
    verify(0);
}

fn main() {
    apply NonEmpty;
}
";
        let bytes = wispee_compiler::compile(src).unwrap();
        let err = make_vm().execute(&bytes).unwrap_err();
        assert!(
            err.to_string().contains("ConstraintViolation"),
            "expected ConstraintViolation, got: {err}"
        );
    }

    #[test]
    fn constraint_passes_when_condition_holds() {
        let src = r#"
constraint AlwaysOk {
    verify(1);
}

fn main() {
    apply AlwaysOk;
    println("passed");
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn persistent_store_load_returns_default_when_absent() {
        let src = r#"
fn main() {
    let v = memory_load("missing_key");
    println(v);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        // No persistent store attached — load returns empty string silently.
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn persistent_store_save_and_load() {
        use crate::persistent_store::InMemoryPersistentStore;
        let src = r#"
fn main() {
    memory_save("key", "hello");
    let v = memory_load("key");
    println(v);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        let store = InMemoryPersistentStore::new();
        let mut vm = Vm::new(4096, Box::new(SimulatedBackend::new("ok")))
            .with_persistent_store(Box::new(store));
        vm.execute(&bytes).unwrap();
    }

    #[test]
    fn persistent_store_delete_removes_key() {
        use crate::persistent_store::InMemoryPersistentStore;
        let src = r#"
fn main() {
    memory_save("k", "v");
    memory_delete("k");
    let v = memory_load("k");
    println(v);
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        let store = InMemoryPersistentStore::new();
        let mut vm = Vm::new(4096, Box::new(SimulatedBackend::new("ok")))
            .with_persistent_store(Box::new(store));
        vm.execute(&bytes).unwrap();
    }

    #[test]
    fn apply_stmt_parses_and_runs() {
        // End-to-end: apply statement inlines the constraint body.
        let src = r#"
constraint Positive {
    verify(1);
}

fn main() {
    apply Positive;
    println("ok");
}
"#;
        let bytes = wispee_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn semantic_rejects_unknown_constraint() {
        // Applying a constraint that was never declared must fail at semantic analysis.
        let src = r"
fn main() {
    apply Undefined;
}
";
        let result = wispee_compiler::compile(src);
        assert!(
            result.is_err(),
            "expected semantic error for unknown constraint"
        );
    }
}
