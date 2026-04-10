// SPDX-License-Identifier: Apache-2.0
//! Abstract syntax tree node definitions for the L-Agent language.

/// Top-level items in a .la source file
#[derive(Debug, Clone)]
pub enum Item {
    FnDef(FnDef),
    KernelDef(KernelDef),
    TypeAlias(TypeAlias),
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
