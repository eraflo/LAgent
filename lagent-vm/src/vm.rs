// SPDX-License-Identifier: Apache-2.0
//! Stack-based L-Agent virtual machine and runtime [`Value`] type.

use crate::backends::InferenceBackend;
use crate::runtime::TokenHeap;
use anyhow::{anyhow, Result};
use lagent_compiler::codegen::opcodes::{Bytecode, OpCode};
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
    /// Handle to an allocated context segment in the [`TokenHeap`].
    CtxHandle(u32),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{s}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::CtxHandle(h) => write!(f, "<ctx#{h}>"),
        }
    }
}

// ── Virtual Machine ───────────────────────────────────────────────────────────

/// The L-Agent Virtual Machine.
pub struct Vm {
    heap: TokenHeap,
    backend: Box<dyn InferenceBackend>,
    /// Soul identity metadata set by `SetAgentMeta`.
    soul_meta: Option<String>,
    /// Persistent memory slots allocated by `AllocMemorySlot`.
    memory: HashMap<String, Value>,
    /// Lore table populated by `StoreLore`.
    lore: HashMap<String, String>,
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
        }
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
        for op in ops {
            match op {
                // ── Literals ──────────────────────────────────────────────
                OpCode::PushStr(s) => frame.push(Value::Str(s.clone())),
                OpCode::PushInt(n) => frame.push(Value::Int(*n)),
                OpCode::PushFloat(f) => frame.push(Value::Float(*f)),

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
                // and constraint markers (Phase 4 stubs).
                OpCode::CtxResize(_, _)
                | OpCode::Reason(_)
                | OpCode::RegisterSkill(_)
                | OpCode::BeginConstraint(_)
                | OpCode::EndConstraint => {}

                // ── I/O ───────────────────────────────────────────────────
                OpCode::Println => {
                    let val = frame.pop()?;
                    println!("{val}");
                }

                // ── Control flow ──────────────────────────────────────────
                OpCode::Return | OpCode::Halt => break,

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
                    arg_strs.reverse(); // restore original argument order
                    let result = self.backend.oracle(name, &arg_strs)?;
                    frame.push(Value::Str(result));
                }

                OpCode::StoreLore(name, text) => {
                    self.lore.insert(name.clone(), text.clone());
                }

                OpCode::LoadLore(name) => {
                    let text = self.lore.get(name).cloned().unwrap_or_default();
                    frame.push(Value::Str(text));
                }
            }
        }
        Ok(())
    }
}

// ── Value comparison helpers ──────────────────────────────────────────────────

fn values_eq(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
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
        let bytes = lagent_compiler::compile(src).unwrap();
        make_vm().execute(&bytes).unwrap();
    }
}
