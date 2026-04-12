// SPDX-License-Identifier: Apache-2.0
//! Abstract syntax tree node definitions for the L-Agent language.

/// Top-level items in a .la source file.
#[derive(Debug, Clone)]
pub enum Item {
    FnDef(FnDef),
    KernelDef(KernelDef),
    TypeAlias(TypeAlias),
    // ── Phase 4: agent vocabulary ──────────────────────────────────────────
    /// Agent identity/personality block — instructions injected into every context.
    SoulDef(SoulDef),
    /// Multi-step workflow, compiled identically to `KernelDef`.
    SpellDef(SpellDef),
    /// Annotated capability function, compiled identically to `FnDef`.
    SkillDef(SkillDef),
    /// Named persistent memory slot.
    MemoryDecl(MemoryDecl),
    /// External knowledge source (RAG / vector DB) stub.
    OracleDecl(OracleDecl),
    /// Named invariant guard block.
    ConstraintDef(ConstraintDef),
    /// Named static knowledge string.
    LoreDecl(LoreDecl),
    /// Module import.
    UseDecl(UseDecl),
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct KernelDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: String,
    pub def: TypeExpr,
}

// ── Phase 4 structs ───────────────────────────────────────────────────────────

/// `soul { stmt* }` — agent identity block.
#[derive(Debug, Clone)]
pub struct SoulDef {
    pub body: Block,
}

/// `spell Name(params) -> T { body }` — multi-step workflow (like `kernel`).
#[derive(Debug, Clone)]
pub struct SpellDef {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: TypeExpr,
    pub body: Block,
}

/// `[pub] skill Name(params) [-> T] { body }` — annotated capability function.
#[derive(Debug, Clone)]
pub struct SkillDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub is_pub: bool,
}

/// `memory Name: T = expr;` — named persistent state slot.
#[derive(Debug, Clone)]
pub struct MemoryDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub init: Expr,
}

/// `oracle Name(params) -> T;` — external knowledge source stub.
#[derive(Debug, Clone)]
pub struct OracleDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: TypeExpr,
}

/// `constraint Name { body }` — named invariant guard.
#[derive(Debug, Clone)]
pub struct ConstraintDef {
    pub name: String,
    pub body: Block,
}

/// `lore Name = "text";` — named static knowledge string.
#[derive(Debug, Clone)]
pub struct LoreDecl {
    pub name: String,
    pub value: String,
}

/// `use "path";` — module import.
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: String,
}

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Named(String),
    Semantic(Vec<String>),
    Primitive(PrimType),
}

#[derive(Debug, Clone)]
pub enum PrimType {
    Str,
    Bool,
    U32,
    F32,
}

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Option<TypeExpr>, Expr),
    Return(Expr),
    Expr(Expr),
    Branch(BranchStmt),
    /// An interruptible block — a Safe Interaction Point the VM can checkpoint.
    Interruptible(Block),
    /// Injects a literal string into the active context (inside `soul` / `skill` bodies).
    Instruction(String),
}

#[derive(Debug, Clone)]
pub struct BranchStmt {
    pub var: String,
    pub cases: Vec<BranchCase>,
    pub default: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct BranchCase {
    pub label: String,
    pub confidence: f64,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Call(String, Vec<Expr>),
    Ident(String),
    StringLit(String),
    IntLit(u64),
    FloatLit(f64),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum BinOp {
    NotEq,
    Gt,
    Lt,
}
