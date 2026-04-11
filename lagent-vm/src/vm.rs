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
}

impl Vm {
    /// Create a new VM with the given context heap capacity and inference backend.
    pub fn new(heap_capacity: usize, backend: Box<dyn InferenceBackend>) -> Self {
        Self {
            heap: TokenHeap::new(heap_capacity),
            backend,
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
        self.run(&bc.instructions, &mut frame)
    }

    fn run(&mut self, ops: &[OpCode], frame: &mut Frame) -> Result<()> {
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
                    let val = frame
                        .locals
                        .get(name)
                        .ok_or_else(|| anyhow!("undefined local: `{name}`"))?
                        .clone();
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
                    // Stack order: handle was pushed first, text second.
                    let text = frame.pop_str()?;
                    let handle = frame.pop_ctx_handle()?;
                    self.heap.append(handle, &text)?;
                }
                OpCode::CtxResize(reg, new_size) => {
                    // Register-indexed variant — not used in Phase 1.
                    let _ = (reg, new_size);
                }

                // ── I/O ───────────────────────────────────────────────────
                OpCode::Println => {
                    let val = frame.pop()?;
                    println!("{val}");
                }

                // ── Control flow ──────────────────────────────────────────
                OpCode::Return | OpCode::Halt => break,

                // ── Phase 2: inference classify ───────────────────────────
                OpCode::InferClassify(labels) => {
                    // Pop prompt string, classify, push winning label.
                    let prompt = frame.pop_str().unwrap_or_default();
                    let results = self.backend.classify(&prompt, labels)?;
                    let winner = results
                        .into_iter()
                        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .map_or_else(String::new, |(label, _)| label);
                    frame.push(Value::Str(winner));
                }

                // ── Phase 2: branch classify ──────────────────────────────
                OpCode::BranchClassify { var, cases, default } => {
                    // Classify the subject against all case labels.
                    let all_labels: Vec<String> =
                        cases.iter().map(|(label, _, _)| label.clone()).collect();
                    let prompt = frame
                        .locals
                        .get(var)
                        .map_or_else(|| var.clone(), Value::to_string);
                    let scores = self.backend.classify(&prompt, &all_labels)?;

                    // Pop any InferClassify result already on the stack (from emit_branch).
                    let _ = frame.pop();

                    // Execute first matching case or default.
                    let mut matched = false;
                    for (label, threshold, body) in cases {
                        let confidence = scores
                            .iter()
                            .find(|(l, _)| l == label)
                            .map_or(0.0, |(_, c)| *c);
                        if confidence >= *threshold {
                            self.run(body, frame)?;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        self.run(default, frame)?;
                    }
                }

                // ── Phase 2: kernel step primitives ──────────────────────
                OpCode::Observe => {
                    // Pop value — observation recorded (no-op in simulated backend).
                    let _ = frame.pop();
                }
                OpCode::Reason(_annotation) => {
                    // Reasoning annotation — no-op in simulated backend.
                }

                // ── Inference (Phase 2+) ──────────────────────────────────
                OpCode::LocalInfer(_, _, _) => {
                    // Not yet implemented.
                    let _ = &self.backend;
                }

                // ── Remaining register-based opcodes (Phase 2+) ───────────
                OpCode::CtxFree(_)
                | OpCode::CtxAppend(_, _)
                | OpCode::Call(_)
                | OpCode::CallKernel(_)
                | OpCode::Branch { .. } => {
                    // Phase 2 — silently skip for now.
                }
            }
        }
        Ok(())
    }
}

// ── Call frame ────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Frame {
    stack: Vec<Value>,
    locals: HashMap<String, Value>,
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
        use lagent_compiler::codegen::opcodes::Bytecode;
        let bc = Bytecode::new(vec![
            OpCode::PushStr("hello from vm".to_string()),
            OpCode::Println,
            OpCode::Halt,
        ]);
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
        // Simulated backend returns uniform weights over labels:
        // "angry"=0.5 < 0.7, "help"=0.5 >= 0.4 → "Support standard" branch fires.
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
        default => {
            println("default");
        }
    }
}
"#;
        // Simulated backend returns 1.0 for single label "angry".
        // But threshold is 0.9 and weight = 1/1 = 1.0 >= 0.9, so "crise" fires.
        // Use two labels so each gets 0.5 < 0.9 → default fires.
        let src2 = r#"
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
        let _ = src; // single-label version fires the case, kept for documentation
        let bytes = lagent_compiler::compile(src2).unwrap();
        make_vm().execute(&bytes).unwrap();
    }

    #[test]
    fn ctx_alloc_and_free_balance_heap() {
        use lagent_compiler::codegen::opcodes::Bytecode;
        let bc = Bytecode::new(vec![
            OpCode::CtxAlloc(256),
            OpCode::CtxFreeStack,
            OpCode::Halt,
        ]);
        let bytes = bincode::serialize(&bc).unwrap();
        let mut vm = make_vm();
        vm.execute(&bytes).unwrap();
        // After alloc+free the heap should be empty.
        assert_eq!(vm.heap.used(), 0);
    }
}
