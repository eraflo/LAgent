// SPDX-License-Identifier: Apache-2.0
//! Bytecode code generator: walks the typed AST and emits [`OpCode`] sequences.

pub mod opcodes;

use crate::parser::ast::{self as ast, Block, Expr, Item, Stmt, TypeExpr};
use crate::semantic::{ConstValue, TypedAst};
use anyhow::{anyhow, Result};
use opcodes::{Bytecode, ExportEntry, ExportKind, KernelBytecode, LibraryBundle, OpCode};
use std::collections::{HashMap, HashSet};

/// Generate bytecode from the typed AST.
///
/// # Errors
///
/// Returns an error if an unsupported AST construct is encountered.
pub fn generate(ast: &TypedAst) -> Result<Vec<u8>> {
    let bytecode = generate_bytecode(ast, false)?;
    let encoded = bincode::serialize(&bytecode)?;
    Ok(encoded)
}

/// Generate a `.lalb` library bundle from the typed AST.
///
/// Only `pub` items are included in the export table.
///
/// # Errors
///
/// Returns an error if an unsupported AST construct is encountered.
#[allow(clippy::cast_possible_truncation)]
pub fn generate_lib(ast: &TypedAst, lib_name: &str) -> Result<Vec<u8>> {
    let bytecode = generate_bytecode(ast, true)?;

    // Build the export table from pub items.
    let mut exports: Vec<ExportEntry> = Vec::new();
    for item in &ast.items {
        match item {
            Item::KernelDef(k) if k.is_pub => {
                let kernel_idx = bytecode
                    .kernels
                    .iter()
                    .position(|kb| kb.name == k.name)
                    .map_or(u16::MAX, |i| i as u16);
                exports.push(ExportEntry {
                    name: k.name.clone(),
                    kind: ExportKind::Kernel,
                    kernel_idx,
                });
            }
            Item::SpellDef(s) if s.is_pub => {
                let kernel_idx = bytecode
                    .kernels
                    .iter()
                    .position(|kb| kb.name == s.name)
                    .map_or(u16::MAX, |i| i as u16);
                exports.push(ExportEntry {
                    name: s.name.clone(),
                    kind: ExportKind::Kernel,
                    kernel_idx,
                });
            }
            Item::SkillDef(s) if s.is_pub => {
                let kernel_idx = bytecode
                    .kernels
                    .iter()
                    .position(|kb| kb.name == s.name)
                    .map_or(u16::MAX, |i| i as u16);
                exports.push(ExportEntry {
                    name: s.name.clone(),
                    kind: ExportKind::Kernel,
                    kernel_idx,
                });
            }
            Item::LoreDecl(l) if l.is_pub => {
                exports.push(ExportEntry {
                    name: l.name.clone(),
                    kind: ExportKind::Lore,
                    kernel_idx: u16::MAX,
                });
            }
            Item::OracleDecl(o) if o.is_pub => {
                exports.push(ExportEntry {
                    name: o.name.clone(),
                    kind: ExportKind::Oracle,
                    kernel_idx: u16::MAX,
                });
            }
            _ => {}
        }
    }

    let bundle = LibraryBundle::new(lib_name.to_string(), bytecode, exports);
    let encoded = bincode::serialize(&bundle)?;
    Ok(encoded)
}

#[allow(clippy::too_many_lines)]
fn generate_bytecode(ast: &TypedAst, _lib_mode: bool) -> Result<Bytecode> {
    let type_env = ast.type_env.clone();
    let oracle_set: HashSet<String> = ast.oracle_names.iter().cloned().collect();
    let constraint_bodies = ast.constraint_bodies.clone();

    // Find the soul definition (at most one), if present.
    let soul_def = ast.items.iter().find_map(|item| {
        if let Item::SoulDef(s) = item {
            Some(s.clone())
        } else {
            None
        }
    });

    // ── Pass 0: emit lore declarations ────────────────────────────────────────
    let mut pre_ops: Vec<OpCode> = Vec::new();
    for item in &ast.items {
        if let Item::LoreDecl(l) = item {
            pre_ops.push(OpCode::StoreLore(l.name.clone(), l.value.clone()));
        }
    }

    // ── Pass 1: compile kernels + spells into the KernelBytecode table ────────
    let mut kernel_index: HashMap<String, u16> = HashMap::new();
    let mut kernels: Vec<KernelBytecode> = Vec::new();

    for item in &ast.items {
        match item {
            Item::KernelDef(k) => {
                #[allow(clippy::cast_possible_truncation)]
                let idx = kernels.len() as u16;
                kernel_index.insert(k.name.clone(), idx);
                let mut gen = Codegen::new(
                    type_env.clone(),
                    kernel_index.clone(),
                    oracle_set.clone(),
                    constraint_bodies.clone(),
                    ast.const_table.clone(),
                );
                gen.emit_block(&k.body)?;
                kernels.push(KernelBytecode {
                    name: k.name.clone(),
                    params: k.params.iter().map(|p| p.name.clone()).collect(),
                    body: gen.ops,
                    max_retries: 3,
                });
            }
            Item::SpellDef(s) => {
                #[allow(clippy::cast_possible_truncation)]
                let idx = kernels.len() as u16;
                kernel_index.insert(s.name.clone(), idx);
                let mut gen = Codegen::new(
                    type_env.clone(),
                    kernel_index.clone(),
                    oracle_set.clone(),
                    constraint_bodies.clone(),
                    ast.const_table.clone(),
                );
                gen.emit_block(&s.body)?;
                kernels.push(KernelBytecode {
                    name: s.name.clone(),
                    params: s.params.iter().map(|p| p.name.clone()).collect(),
                    body: gen.ops,
                    max_retries: 3,
                });
            }
            Item::SkillDef(s) => {
                #[allow(clippy::cast_possible_truncation)]
                let idx = kernels.len() as u16;
                kernel_index.insert(s.name.clone(), idx);
                let mut gen = Codegen::new(
                    type_env.clone(),
                    kernel_index.clone(),
                    oracle_set.clone(),
                    constraint_bodies.clone(),
                    ast.const_table.clone(),
                );
                gen.emit_block(&s.body)?;
                kernels.push(KernelBytecode {
                    name: s.name.clone(),
                    params: s.params.iter().map(|p| p.name.clone()).collect(),
                    body: gen.ops,
                    max_retries: 3,
                });
            }
            _ => {}
        }
    }

    // ── Pass 2: compile fn/skill/memory/oracle into the main instruction stream ─
    let mut gen = Codegen::new(
        type_env,
        kernel_index,
        oracle_set,
        constraint_bodies,
        ast.const_table.clone(),
    );

    // Emit lore pre-ops before any fn body.
    for op in pre_ops {
        gen.emit(op);
    }

    // Emit memory initialisations (top of program, before any fn body).
    for item in &ast.items {
        if let Item::MemoryDecl(m) = item {
            gen.emit_expr(&m.init)?;
            gen.emit(OpCode::AllocMemorySlot(m.name.clone()));
        }
    }

    for item in &ast.items {
        if let Item::FnDef(f) = item {
            // Emit the soul preamble before `fn main`.
            if f.name == "main" {
                if let Some(ref soul) = soul_def {
                    gen.emit(OpCode::SetAgentMeta("soul".to_string()));
                    gen.emit_block(&soul.body)?;
                }
            }
            gen.emit_block(&f.body)?;
            gen.emit(OpCode::Halt);
        }
    }

    if gen.ops.is_empty() {
        gen.emit(OpCode::Halt);
    }

    Ok(Bytecode::new(kernels, gen.ops))
}

// ── Internal code-generation state ───────────────────────────────────────────

/// Context for the innermost enclosing loop — used by break/continue.
#[derive(Debug, Clone)]
struct LoopCtx {
    #[allow(dead_code)]
    start_idx: usize,
    break_patch_idxs: Vec<usize>,
    continue_patch_idxs: Vec<usize>,
}

struct Codegen {
    ops: Vec<OpCode>,
    type_env: HashMap<String, Vec<String>>,
    kernel_index: HashMap<String, u16>,
    oracle_set: HashSet<String>,
    /// Constraint name → body block, used for `apply ConstraintName;` inlining.
    constraint_bodies: HashMap<String, Block>,
    /// When `true`, `verify(...)` emits `ConstraintVerify` (non-retriable)
    /// instead of `VerifyStep` (retriable via kernel retry loop).
    in_constraint: bool,
    /// Stack of enclosing loop contexts — for break/continue code generation.
    loop_stack: Vec<LoopCtx>,
    /// Const name → compile-time evaluated value.
    const_table: HashMap<String, ConstValue>,
}

impl Codegen {
    fn new(
        type_env: HashMap<String, Vec<String>>,
        kernel_index: HashMap<String, u16>,
        oracle_set: HashSet<String>,
        constraint_bodies: HashMap<String, Block>,
        const_table: HashMap<String, ConstValue>,
    ) -> Self {
        Self {
            ops: Vec::new(),
            type_env,
            kernel_index,
            oracle_set,
            constraint_bodies,
            in_constraint: false,
            loop_stack: Vec::new(),
            const_table,
        }
    }

    fn emit(&mut self, op: OpCode) {
        self.ops.push(op);
    }

    fn emit_block(&mut self, block: &[Stmt]) -> Result<()> {
        for stmt in block {
            self.emit_stmt(stmt)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn emit_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            // ── let [mut] x [: T] = expr ─────────────────────────────────────
            Stmt::Let(name, ty, expr_opt, _is_mut) => {
                // Only emit if there's an initializer expression
                if let Some(expr) = expr_opt {
                    // Special case: `let x: SomeSemanticType = infer(arg)`
                    // — inject the resolved labels into InferClassify.
                    if let (Some(TypeExpr::Named(type_name)), Expr::Call(callee, args)) = (ty, expr)
                    {
                        if callee == "infer" {
                            let labels = self.type_env.get(type_name).cloned().unwrap_or_default();
                            self.emit_expr(arg(args, 0, "infer")?)?;
                            self.emit(OpCode::InferClassify(labels));
                            self.emit(OpCode::StoreLocal(name.clone()));
                            return Ok(());
                        }
                    }
                    self.emit_expr(expr)?;
                }
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
                let labels: Vec<String> = b.cases.iter().map(|c| c.label.clone()).collect();
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
                self.emit(OpCode::InferClassify(labels));
                self.emit(OpCode::BranchClassify {
                    var: b.var.clone(),
                    cases: case_ops,
                    default: default_ops,
                });
            }

            Stmt::Interruptible(block) => {
                self.emit(OpCode::BeginInterruptible);
                self.emit_block(block)?;
                self.emit(OpCode::EndInterruptible);
            }

            // instruction "text"; — append literal to in-scope ctx handle.
            Stmt::Instruction(text) => {
                self.emit(OpCode::CtxAppendLiteral(text.clone()));
            }

            // apply ConstraintName; — inline constraint body at this call site.
            Stmt::Apply(name) => {
                let body = self
                    .constraint_bodies
                    .get(name)
                    .ok_or_else(|| anyhow!("unknown constraint `{name}`"))?
                    .clone();
                self.emit(OpCode::BeginConstraint(name.clone()));
                self.in_constraint = true;
                self.emit_block(&body)?;
                self.in_constraint = false;
                self.emit(OpCode::EndConstraint);
            }

            // ── Phase 7: Control flow ────────────────────────────────────
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                // Emit condition
                self.emit_expr(condition)?;

                // Placeholder for jump address - will be patched later
                let jump_false_idx = self.ops.len();
                self.emit(OpCode::JumpIfFalse(0)); // placeholder

                // Emit then branch
                self.emit_block(then_branch)?;

                // Placeholder for end jump
                let jump_end_idx = self.ops.len();
                self.emit(OpCode::Jump(0)); // placeholder

                // Patch JumpIfFalse to jump to else branch (or end if no else)
                let else_start_idx = self.ops.len();
                if let OpCode::JumpIfFalse(ref mut addr) = self.ops[jump_false_idx] {
                    *addr = else_start_idx;
                }

                // Emit else branch if present
                if let Some(else_block) = else_branch {
                    self.emit_block(else_block)?;
                }

                // Patch Jump to jump to end
                let end_idx = self.ops.len();
                if let OpCode::Jump(ref mut addr) = self.ops[jump_end_idx] {
                    *addr = end_idx;
                }
            }

            Stmt::Loop(body) => {
                let loop_start_idx = self.ops.len();

                // Push loop context for break/continue
                self.loop_stack.push(LoopCtx {
                    start_idx: loop_start_idx,
                    break_patch_idxs: Vec::new(),
                    continue_patch_idxs: Vec::new(),
                });

                self.emit_block(body)?;

                // Jump back to start
                self.emit(OpCode::Jump(loop_start_idx));

                // Pop loop context and patch break/continue
                let ctx = self.loop_stack.pop().unwrap();
                let loop_end_idx = self.ops.len();

                // Patch all break jumps to go to loop exit
                for idx in ctx.break_patch_idxs {
                    if let OpCode::Jump(ref mut addr) = self.ops[idx] {
                        *addr = loop_end_idx;
                    }
                }

                // Patch all continue jumps to go to loop start
                for idx in ctx.continue_patch_idxs {
                    if let OpCode::Jump(ref mut addr) = self.ops[idx] {
                        *addr = loop_start_idx;
                    }
                }
            }

            Stmt::While { condition, body } => {
                let condition_idx = self.ops.len();

                // Push loop context
                self.loop_stack.push(LoopCtx {
                    start_idx: condition_idx,
                    break_patch_idxs: Vec::new(),
                    continue_patch_idxs: Vec::new(),
                });

                self.emit_expr(condition)?;
                let jump_false_idx = self.ops.len();
                self.emit(OpCode::JumpIfFalse(0)); // placeholder

                self.emit_block(body)?;

                // Jump back to condition
                self.emit(OpCode::Jump(condition_idx));

                // Patch JumpIfFalse and break jumps
                let end_idx = self.ops.len();
                if let OpCode::JumpIfFalse(ref mut addr) = self.ops[jump_false_idx] {
                    *addr = end_idx;
                }

                // Pop loop context and patch
                let ctx = self.loop_stack.pop().unwrap();
                for idx in ctx.break_patch_idxs {
                    if let OpCode::Jump(ref mut addr) = self.ops[idx] {
                        *addr = end_idx;
                    }
                }
                for idx in ctx.continue_patch_idxs {
                    if let OpCode::Jump(ref mut addr) = self.ops[idx] {
                        *addr = condition_idx;
                    }
                }
            }

            Stmt::For {
                item,
                collection,
                body,
            } => {
                // Generate unique local names for the iteration state
                let vec_name = format!("__for_vec_{body:p}");
                let len_name = format!("__for_len_{body:p}");
                let i_name = format!("__for_i_{body:p}");

                // Evaluate collection and store it
                self.emit_expr(collection)?;
                self.emit(OpCode::StoreLocal(vec_name.clone()));

                // Get length and store it
                self.emit(OpCode::LoadLocal(vec_name.clone()));
                self.emit(OpCode::VecLen);
                self.emit(OpCode::StoreLocal(len_name.clone()));

                // Initialize counter to 0
                self.emit(OpCode::PushInt(0));
                self.emit(OpCode::StoreLocal(i_name.clone()));

                // Condition start index
                let cond_start_idx = self.ops.len();

                // Push loop context
                self.loop_stack.push(LoopCtx {
                    start_idx: cond_start_idx,
                    break_patch_idxs: Vec::new(),
                    continue_patch_idxs: Vec::new(),
                });

                // Loop: check i < len
                self.emit(OpCode::LoadLocal(i_name.clone()));
                self.emit(OpCode::LoadLocal(len_name.clone()));
                self.emit(OpCode::CmpLt);

                let jump_exit_idx = self.ops.len();
                self.emit(OpCode::JumpIfFalse(0)); // placeholder

                // Get vec[i] and store as item
                self.emit(OpCode::LoadLocal(vec_name.clone()));
                self.emit(OpCode::LoadLocal(i_name.clone()));
                self.emit(OpCode::VecGet);
                self.emit(OpCode::StoreLocal(item.clone()));

                // Execute body
                self.emit_block(body)?;

                // Increment index: i = i + 1
                let increment_idx = self.ops.len();
                self.emit(OpCode::LoadLocal(i_name.clone()));
                self.emit(OpCode::PushInt(1));
                self.emit(OpCode::Add);
                self.emit(OpCode::StoreLocal(i_name.clone()));

                // Jump back to condition
                self.emit(OpCode::Jump(cond_start_idx));

                // Patch JumpIfFalse to exit
                let exit_idx = self.ops.len();
                if let OpCode::JumpIfFalse(ref mut addr) = self.ops[jump_exit_idx] {
                    *addr = exit_idx;
                }

                // Patch break jumps to exit
                let ctx = self.loop_stack.pop().unwrap();
                for idx in ctx.break_patch_idxs {
                    if let OpCode::Jump(ref mut addr) = self.ops[idx] {
                        *addr = exit_idx;
                    }
                }
                // Patch continue jumps to increment step
                for idx in ctx.continue_patch_idxs {
                    if let OpCode::Jump(ref mut addr) = self.ops[idx] {
                        *addr = increment_idx;
                    }
                }
            }

            Stmt::Assign(name, expr) => {
                self.emit_expr(expr)?;
                self.emit(OpCode::StoreLocal(name.clone()));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn emit_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::StringLit(s) => self.emit(OpCode::PushStr(s.clone())),
            Expr::IntLit(n) => self.emit(OpCode::PushInt(*n)),
            Expr::FloatLit(f) => self.emit(OpCode::PushFloat(*f)),
            Expr::BoolLit(b) => self.emit(OpCode::PushBool(*b)),
            Expr::Ident(name) => {
                // Check if this is a compile-time constant
                if let Some(value) = self.const_table.get(name) {
                    match value {
                        ConstValue::Int(n) => self.emit(OpCode::PushInt((*n).cast_unsigned())),
                        ConstValue::Float(f) => self.emit(OpCode::PushFloat(*f)),
                        ConstValue::Bool(b) => self.emit(OpCode::PushBool(*b)),
                        ConstValue::Str(s) => self.emit(OpCode::PushStr(s.clone())),
                    }
                } else {
                    self.emit(OpCode::LoadLocal(name.clone()));
                }
            }
            Expr::Call(name, args) => self.emit_call(name, args)?,
            Expr::BinOp(lhs, op, rhs) => {
                self.emit_expr(lhs)?;
                self.emit_expr(rhs)?;
                let cmp = match op {
                    ast::BinOp::NotEq => OpCode::CmpNotEq,
                    ast::BinOp::Eq => OpCode::CmpEq,
                    ast::BinOp::Gt => OpCode::CmpGt,
                    ast::BinOp::Lt => OpCode::CmpLt,
                    ast::BinOp::Add => OpCode::Add,
                    ast::BinOp::Sub => OpCode::Sub,
                    ast::BinOp::Mul => OpCode::Mul,
                    ast::BinOp::Div => OpCode::Div,
                    ast::BinOp::Mod => OpCode::Mod,
                    ast::BinOp::And => OpCode::And,
                    ast::BinOp::Or => OpCode::Or,
                };
                self.emit(cmp);
            }
            Expr::Break => {
                if let Some(_ctx) = self.loop_stack.last() {
                    let idx = self.ops.len();
                    self.emit(OpCode::Jump(0));
                    self.loop_stack
                        .last_mut()
                        .unwrap()
                        .break_patch_idxs
                        .push(idx);
                } else {
                    return Err(anyhow!("`break` outside of loop"));
                }
            }
            Expr::Continue => {
                if let Some(_ctx) = self.loop_stack.last() {
                    let idx = self.ops.len();
                    self.emit(OpCode::Jump(0));
                    self.loop_stack
                        .last_mut()
                        .unwrap()
                        .continue_patch_idxs
                        .push(idx);
                } else {
                    return Err(anyhow!("`continue` outside of loop"));
                }
            }
            Expr::Tuple(exprs) => {
                for e in exprs {
                    self.emit_expr(e)?;
                }
                #[allow(clippy::cast_possible_truncation)]
                self.emit(OpCode::TuplePack(exprs.len() as u8));
            }
            Expr::VecLit(exprs) => {
                for e in exprs {
                    self.emit_expr(e)?;
                }
                #[allow(clippy::cast_possible_truncation)]
                self.emit(OpCode::VecNew(exprs.len() as u8));
            }
            Expr::Index(base, idx) => {
                self.emit_expr(base)?;
                self.emit_expr(idx)?;
                self.emit(OpCode::VecGet);
            }
            Expr::FieldAccess(base, field) => {
                self.emit_expr(base)?;
                self.emit(OpCode::FieldAccess(field.clone()));
            }
            Expr::StructConstruct { name, fields } => {
                for (_, e) in fields {
                    self.emit_expr(e)?;
                }
                let field_names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                self.emit(OpCode::StructConstruct {
                    name: name.clone(),
                    field_names,
                });
            }
            Expr::EnumVariant { variant, payload } => {
                if let Some(e) = payload {
                    self.emit_expr(e)?;
                }
                self.emit(OpCode::EnumVariant {
                    variant: variant.clone(),
                    payload: payload.is_some(),
                });
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, name: &str, args: &[Expr]) -> Result<()> {
        // ── Kernel / spell call ───────────────────────────────────────────────
        if let Some(&idx) = self.kernel_index.get(name) {
            for a in args {
                self.emit_expr(a)?;
            }
            self.emit(OpCode::CallKernel(idx));
            return Ok(());
        }

        // ── Oracle call ───────────────────────────────────────────────────────
        if self.oracle_set.contains(name) {
            for a in args {
                self.emit_expr(a)?;
            }
            #[allow(clippy::cast_possible_truncation)]
            self.emit(OpCode::CallOracle(name.to_string(), args.len() as u8));
            return Ok(());
        }

        match name {
            "ctx_alloc" => {
                let size = extract_int_arg(args, 0, "ctx_alloc")?;
                #[allow(clippy::cast_possible_truncation)]
                self.emit(OpCode::CtxAlloc(size as u32));
            }
            "ctx_free" => {
                self.emit_expr(arg(args, 0, "ctx_free")?)?;
                self.emit(OpCode::CtxFreeStack);
            }
            "ctx_append" => {
                self.emit_expr(arg(args, 0, "ctx_append")?)?;
                self.emit_expr(arg(args, 1, "ctx_append")?)?;
                self.emit(OpCode::CtxAppendStack);
            }
            "ctx_compress" => {
                self.emit_expr(arg(args, 0, "ctx_compress")?)?;
                self.emit(OpCode::CtxCompress);
            }
            "ctx_share" => {
                self.emit_expr(arg(args, 0, "ctx_share")?)?;
                self.emit(OpCode::CtxShare);
            }
            "ctx_resize" => {
                self.emit_expr(arg(args, 0, "ctx_resize")?)?;
                let size = extract_int_arg(args, 1, "ctx_resize")?;
                #[allow(clippy::cast_possible_truncation)]
                self.emit(OpCode::CtxResize(0, size as u32));
            }
            "println" => {
                self.emit_expr(arg(args, 0, "println")?)?;
                self.emit(OpCode::Println);
            }
            "observe" => {
                self.emit_expr(arg(args, 0, "observe")?)?;
                self.emit(OpCode::Observe);
            }
            "reason" => match arg(args, 0, "reason")? {
                Expr::StringLit(s) => self.emit(OpCode::Reason(s.clone())),
                _other => {
                    // Non-literal reason: emit as annotation only, no stack effect.
                    // We don't evaluate the expression since Reason is a no-op in the VM.
                    self.emit(OpCode::Reason(String::new()));
                }
            },
            "act" => {
                self.emit_expr(arg(args, 0, "act")?)?;
                self.emit(OpCode::Act);
            }
            "verify" => {
                self.emit_expr(arg(args, 0, "verify")?)?;
                if self.in_constraint {
                    self.emit(OpCode::ConstraintVerify);
                } else {
                    self.emit(OpCode::VerifyStep);
                }
            }
            "infer" => {
                self.emit_expr(arg(args, 0, "infer")?)?;
                self.emit(OpCode::InferClassify(vec![]));
            }
            "memory_load" => {
                self.emit_expr(arg(args, 0, "memory_load")?)?;
                self.emit(OpCode::PersistLoad);
            }
            "memory_save" => {
                self.emit_expr(arg(args, 0, "memory_save")?)?;
                self.emit_expr(arg(args, 1, "memory_save")?)?;
                self.emit(OpCode::PersistSave);
            }
            "memory_delete" => {
                self.emit_expr(arg(args, 0, "memory_delete")?)?;
                self.emit(OpCode::PersistDelete);
            }
            _ => {
                // This should have been caught by semantic analysis.
                return Err(anyhow!("undefined function: `{name}`"));
            }
        }
        Ok(())
    }

    /// Compile a block into a standalone `Vec<OpCode>` without touching `self.ops`.
    fn compile_block(&mut self, block: &[Stmt]) -> Result<Vec<OpCode>> {
        let saved = std::mem::take(&mut self.ops);
        self.emit_block(block)?;
        Ok(std::mem::replace(&mut self.ops, saved))
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

    fn compile_ops(src: &str) -> Vec<OpCode> {
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        let typed = analyze(items).unwrap();
        let bytes = generate(&typed).unwrap();
        let bc: Bytecode = bincode::deserialize(&bytes).unwrap();
        bc.instructions
    }

    fn compile_full(src: &str) -> Bytecode {
        let tokens = tokenize(src).unwrap();
        let items = parse(tokens).unwrap();
        let typed = analyze(items).unwrap();
        let bytes = generate(&typed).unwrap();
        bincode::deserialize(&bytes).unwrap()
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
        let ops = compile_ops(src);
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
        let ops = compile_ops("fn main() {}");
        assert_eq!(ops, vec![OpCode::Halt]);
    }

    #[test]
    fn compiles_kernel_into_table() {
        let src = r#"
type Sentiment = semantic("positive", "negative");
kernel Classify(text: str) -> Sentiment {
    observe(text);
    let r: Sentiment = infer(text);
    verify(r != "");
    return r;
}
fn main() {}
"#;
        let bc = compile_full(src);
        assert_eq!(bc.kernels.len(), 1);
        assert_eq!(bc.kernels[0].name, "Classify");
        assert_eq!(bc.kernels[0].params, vec!["text"]);
        assert!(bc.kernels[0]
            .body
            .iter()
            .any(|o| matches!(o, OpCode::Observe)));
        assert!(bc.kernels[0]
            .body
            .iter()
            .any(|o| matches!(o, OpCode::VerifyStep)));
        assert!(bc.kernels[0].body.iter().any(|o| {
            matches!(o, OpCode::InferClassify(labels) if labels == &["positive", "negative"])
        }));
    }

    #[test]
    fn compiles_kernel_call() {
        let src = r#"
type Sentiment = semantic("positive", "negative");
kernel Classify(text: str) -> Sentiment {
    let r: Sentiment = infer(text);
    return r;
}
fn main() {
    let ctx = ctx_alloc(256);
    let result = Classify(ctx);
    println(result);
    ctx_free(ctx);
}
"#;
        let bc = compile_full(src);
        assert_eq!(bc.kernels.len(), 1);
        assert!(bc
            .instructions
            .iter()
            .any(|o| matches!(o, OpCode::CallKernel(0))));
    }

    #[test]
    fn compiles_interruptible_block() {
        let ops = compile_ops(r#"fn main() { interruptible { println("safe"); } }"#);
        assert!(ops.iter().any(|o| matches!(o, OpCode::BeginInterruptible)));
        assert!(ops.iter().any(|o| matches!(o, OpCode::EndInterruptible)));
    }

    #[test]
    fn compiles_ctx_compress() {
        let ops = compile_ops(
            r"fn main() { let ctx = ctx_alloc(1024); ctx_compress(ctx); ctx_free(ctx); }",
        );
        assert!(ops.iter().any(|o| matches!(o, OpCode::CtxCompress)));
    }

    #[test]
    fn compiles_agent_soul() {
        let src = r#"
soul {
    instruction "You are a helpful agent.";
}
fn main() {}
"#;
        let ops = compile_ops(src);
        assert!(ops.iter().any(|o| matches!(o, OpCode::SetAgentMeta(_))));
        assert!(ops.iter().any(|o| matches!(o, OpCode::CtxAppendLiteral(_))));
    }

    #[test]
    fn compiles_lore_and_memory() {
        let src = r#"
lore Background = "Some lore text.";
memory LastResult: str = "";
fn main() {}
"#;
        let ops = compile_ops(src);
        assert!(ops
            .iter()
            .any(|o| matches!(o, OpCode::StoreLore(n, _) if n == "Background")));
        assert!(ops
            .iter()
            .any(|o| matches!(o, OpCode::AllocMemorySlot(n) if n == "LastResult")));
    }

    #[test]
    fn compiles_spell_into_kernel_table() {
        let src = "
spell Greet(name: str) -> str {
    return name;
}
fn main() {}
";
        let bc = compile_full(src);
        assert_eq!(bc.kernels.len(), 1);
        assert_eq!(bc.kernels[0].name, "Greet");
    }

    #[test]
    fn compiles_oracle_call() {
        let src = r#"
oracle Lookup(q: str) -> str;
fn main() {
    let r = Lookup("test");
    println(r);
}
"#;
        let ops = compile_ops(src);
        assert!(ops
            .iter()
            .any(|o| matches!(o, OpCode::CallOracle(n, 1) if n == "Lookup")));
    }
}
