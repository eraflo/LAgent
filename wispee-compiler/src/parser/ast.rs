// SPDX-License-Identifier: Apache-2.0
//! Abstract syntax tree node definitions for the Wispee language.

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
    // ── Phase 7: Composite types & constants ─────────────────────────────
    /// `struct Name { fields }` — named aggregate type.
    StructDef(StructDef),
    /// `enum Name { variants }` — tagged union type.
    EnumDef(EnumDef),
    /// `const NAME: Type = value;` — compile-time constant.
    ConstDef(ConstDef),
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct KernelDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub body: Block,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: String,
    pub def: TypeExpr,
    pub is_pub: bool,
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
    pub is_pub: bool,
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
    pub is_pub: bool,
}

/// `constraint Name { body }` — named invariant guard.
#[derive(Debug, Clone)]
pub struct ConstraintDef {
    pub name: String,
    pub body: Block,
    pub is_pub: bool,
}

/// `lore Name = "text";` — named static knowledge string.
#[derive(Debug, Clone)]
pub struct LoreDecl {
    pub name: String,
    pub value: String,
    pub is_pub: bool,
}

/// `use "path";` — module import.
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: String,
}

// ── Phase 7: Composite types & constants ──────────────────────────────────────

/// `struct Name { field: Type, ... }` — aggregate type.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: TypeExpr,
}

/// `enum Name { VariantA, VariantB, ... }` — tagged union.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<TypeExpr>,
}

/// `const NAME: Type = value;` — compile-time constant.
#[derive(Debug, Clone)]
pub struct ConstDef {
    pub name: String,
    pub ty: TypeExpr,
    pub value: Expr,
    pub is_pub: bool,
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
    /// `Vec<T>` — dynamic array type.
    Vec(Box<TypeExpr>),
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
    Let(String, Option<TypeExpr>, Option<Expr>, bool), // name, type, init_expr, is_mut
    Return(Expr),
    Expr(Expr),
    Branch(BranchStmt),
    /// An interruptible block — a Safe Interaction Point the VM can checkpoint.
    Interruptible(Block),
    /// Injects a literal string into the active context (inside `soul` / `skill` bodies).
    Instruction(String),
    /// `apply ConstraintName;` — inline a named constraint body at this call site.
    Apply(String),
    // ── Phase 7: Control flow ────────────────────────────────────────────
    /// `if condition { ... } else { ... }`
    If {
        condition: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    /// `loop { ... }`
    Loop(Block),
    /// `while condition { ... }`
    While {
        condition: Expr,
        body: Block,
    },
    /// `for item in collection { ... }`
    For {
        item: String,
        collection: Expr,
        body: Block,
    },
    /// `x = expr;` — reassignment of a mutable variable.
    Assign(String, Expr),
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
    BoolLit(bool),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    // ── Phase 7: Control flow expressions ────────────────────────────────
    /// `break` — exit the innermost loop
    Break,
    /// `continue` — skip to the next iteration of the innermost loop
    Continue,
    /// Tuple literal — `(a, b, c)`
    Tuple(Vec<Expr>),
    /// ── Phase 8: Collections ────────────────────────────────────────────
    /// Vector literal — `[a, b, c]`
    VecLit(Vec<Expr>),
    /// Index access — `expr[index]`
    Index(Box<Expr>, Box<Expr>),
    /// Field/tuple access — `expr.field` or `tuple.0`
    FieldAccess(Box<Expr>, String),
    /// Struct construction — `Name { field: expr, ... }`
    StructConstruct {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    /// Enum variant construction — `Variant(expr)` or `Variant`
    EnumVariant {
        variant: String,
        payload: Option<Box<Expr>>,
    },
}

#[derive(Debug, Clone)]
pub enum BinOp {
    // Comparison operators
    NotEq,
    Eq,
    Gt,
    Lt,
    // Logical operators
    And,
    Or,
    // Arithmetic operators
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}
