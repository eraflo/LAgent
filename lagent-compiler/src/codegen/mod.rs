// SPDX-License-Identifier: Apache-2.0
//! Bytecode code generator: walks the typed AST and emits [`OpCode`](opcodes::OpCode) sequences.

pub mod opcodes;

use crate::parser::ast::{Expr, Item, Stmt};
use crate::semantic::TypedAst;
use anyhow::{anyhow, Result};
use opcodes::{Bytecode, OpCode};

/// Generate bytecode from the typed AST.
///
/// # Errors
///
/// Returns an error if an unsupported AST construct is encountered.
pub fn generate(ast: TypedAst) -> Result<Vec<u8>> {
    let mut gen = Codegen::default();

    for item in ast.items {
        match item {
            Item::FnDef(f) => {
                gen.emit_block(&f.body)?;
                // Every function ends with Halt (Phase 1/2: no call frames yet).
                gen.emit(OpCode::Halt);
            }
            // KernelDef and TypeAlias are parsed and type-checked but not yet
            // lowered to a callable — proper call frames come in Phase 3.
            Item::KernelDef(_) | Item::TypeAlias(_) => {}
        }
    }

    if gen.ops.is_empty() {
        gen.emit(OpCode::Halt);
    }

    let bytecode = Bytecode::new(gen.ops);
    let encoded = bincode::serialize(&bytecode)?;
    Ok(encoded)
}

// ── Internal code-generation state ───────────────────────────────────────────

#[derive(Default)]
struct Codegen {
    ops: Vec<OpCode>,
}

impl Codegen {
    fn emit(&mut self, op: OpCode) {
        self.ops.push(op);
    }

    fn emit_block(&mut self, block: &[Stmt]) -> Result<()> {
        for stmt in block {
            self.emit_stmt(stmt)?;
        }
        Ok(())
    }

    fn emit_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Let(name, _ty, expr) => {
                self.emit_expr(expr)?;
                self.emit(OpCode::StoreLocal(name.clone()));
            }
            Stmt::Expr(expr) => {
                self.emit_expr(expr)?;
            }
            Stmt::Return(expr) => {
                self.emit_expr(expr)?;
                self.emit(OpCode::Return);
            }
            Stmt::Branch(b) => {
                // Collect all unique labels from cases for classify call.
                let labels: Vec<String> = b.cases.iter().map(|c| c.label.clone()).collect();

                // Compile each case body independently.
                let mut case_ops = Vec::new();
                for case in &b.cases {
                    let body = self.compile_block(&case.body)?;
                    #[allow(clippy::cast_possible_truncation)]
                    case_ops.push((case.label.clone(), case.confidence as f32, body));
                }

                let default_ops = match &b.default {
                    Some(block) => self.compile_block(block)?,
                    None => vec![],
                };

                // Emit InferClassify first so the VM has a result label on the stack,
                // then BranchClassify dispatches based on that result.
                self.emit(OpCode::InferClassify(labels));
                self.emit(OpCode::BranchClassify {
                    var: b.var.clone(),
                    cases: case_ops,
                    default: default_ops,
                });
            }
        }
        Ok(())
    }

    /// Compile a block into a standalone instruction sequence without touching `self.ops`.
    fn compile_block(&mut self, block: &[Stmt]) -> Result<Vec<OpCode>> {
        let saved = std::mem::take(&mut self.ops);
        self.emit_block(block)?;
        let result = std::mem::replace(&mut self.ops, saved);
        Ok(result)
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::StringLit(s) => {
                self.emit(OpCode::PushStr(s.clone()));
            }
            Expr::IntLit(n) => {
                self.emit(OpCode::PushInt(*n));
            }
            Expr::FloatLit(f) => {
                self.emit(OpCode::PushFloat(*f));
            }
            Expr::Ident(name) => {
                self.emit(OpCode::LoadLocal(name.clone()));
            }
            Expr::Call(name, args) => {
                self.emit_call(name, args)?;
            }
            Expr::BinOp(lhs, _op, rhs) => {
                // Phase 2 — just emit both sides for now so locals are loaded.
                self.emit_expr(lhs)?;
                self.emit_expr(rhs)?;
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, name: &str, args: &[Expr]) -> Result<()> {
        match name {
            // ctx_alloc(size: int) → CtxAlloc(size)  [pushes handle]
            "ctx_alloc" => {
                let size = extract_int_arg(args, 0, "ctx_alloc")?;
                #[allow(clippy::cast_possible_truncation)]
                self.emit(OpCode::CtxAlloc(size as u32));
            }

            // ctx_free(handle)
            "ctx_free" => {
                self.emit_expr(arg(args, 0, "ctx_free")?)?;
                self.emit(OpCode::CtxFreeStack);
            }

            // ctx_append(handle, text)
            "ctx_append" => {
                self.emit_expr(arg(args, 0, "ctx_append")?)?;
                self.emit_expr(arg(args, 1, "ctx_append")?)?;
                self.emit(OpCode::CtxAppendStack);
            }

            // println(value)
            "println" => {
                self.emit_expr(arg(args, 0, "println")?)?;
                self.emit(OpCode::Println);
            }

            // observe(value) — push value then Observe
            "observe" => {
                self.emit_expr(arg(args, 0, "observe")?)?;
                self.emit(OpCode::Observe);
            }

            // reason("annotation") — emit Reason with string literal
            "reason" => match arg(args, 0, "reason")? {
                Expr::StringLit(s) => self.emit(OpCode::Reason(s.clone())),
                other => {
                    self.emit_expr(other)?;
                    self.emit(OpCode::Reason(String::new()));
                }
            },

            // infer(value) — push value then InferClassify (labels resolved at branch)
            "infer" => {
                self.emit_expr(arg(args, 0, "infer")?)?;
                self.emit(OpCode::InferClassify(vec![]));
            }

            // verify(expr) — evaluate for side-effects, result discarded
            "verify" => {
                self.emit_expr(arg(args, 0, "verify")?)?;
            }

            // Generic user-defined function call.
            _ => {
                for a in args {
                    self.emit_expr(a)?;
                }
                self.emit(OpCode::Call(name.to_string()));
            }
        }
        Ok(())
    }
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn arg<'a>(args: &'a [Expr], idx: usize, builtin: &str) -> Result<&'a Expr> {
    args.get(idx)
        .ok_or_else(|| anyhow!("{builtin}: missing argument {idx}"))
}

fn extract_int_arg(args: &[Expr], idx: usize, builtin: &str) -> Result<u64> {
    match arg(args, idx, builtin)? {
        Expr::IntLit(n) => Ok(*n),
        other => Err(anyhow!(
            "{builtin}: expected integer literal at argument {idx}, got {other:?}"
        )),
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::semantic::analyze;

    fn compile(src: &str) -> Vec<OpCode> {
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        let typed = analyze(items).unwrap();
        let bytes = generate(typed).unwrap();
        let bc: Bytecode = bincode::deserialize(&bytes).unwrap();
        bc.instructions
    }

    #[test]
    fn compiles_hello_la() {
        let src = r#"
fn main() {
    let ctx = ctx_alloc(512);
    ctx_append(ctx, "Hello, L-Agent!");
    println("Hello, L-Agent!");
    ctx_free(ctx);
}
"#;
        let ops = compile(src);
        assert!(ops.iter().any(|o| matches!(o, OpCode::CtxAlloc(512))));
        assert!(ops
            .iter()
            .any(|o| matches!(o, OpCode::StoreLocal(n) if n == "ctx")));
        assert!(ops.iter().any(|o| matches!(o, OpCode::Println)));
        assert!(ops.iter().any(|o| matches!(o, OpCode::CtxFreeStack)));
        assert!(matches!(ops.last(), Some(OpCode::Halt)));
    }

    #[test]
    fn empty_fn_emits_halt() {
        let ops = compile("fn main() {}");
        assert_eq!(ops, vec![OpCode::Halt]);
    }
}
