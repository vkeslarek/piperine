//! Abstract syntax tree for Verilog-A/AMS (OpenVAF subset).
//!
//! Split by area — [`expr`] (expressions), [`stmt`] (statements), [`item`]
//! (declarations) — all re-exported here so consumers can `use crate::ast::*`.

mod expr;
mod item;
mod stmt;

pub use expr::*;
pub use item::*;
pub use stmt::*;

/// A byte-offset range into the (preprocessed) source buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    DisciplineDecl(DisciplineDecl),
    NatureDecl(NatureDecl),
    ModuleDecl(ModuleDecl),
    ExternModule(ExternModuleDecl),
    TypedefEnum(TypedefEnum),
    TypedefStruct(TypedefStruct),
    ExternClass(ExternClassDecl),
    Paramset(ParamsetDecl),
}

#[derive(Debug, Clone)]
pub struct Name(pub String);

#[derive(Debug, Clone)]
pub struct NameRef(pub String);

#[derive(Debug, Clone)]
pub struct Path {
    pub qualifier: Option<Box<Path>>,
    pub segment: PathSegment,
}

#[derive(Debug, Clone)]
pub enum PathSegment {
    Ident(String),
    Root,
}

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Real,
    String,
    Custom(Name),
}

/// A declaration bit/array range `[msb:lsb]` (or `[size]`, where `msb == lsb`).
#[derive(Debug, Clone)]
pub struct BitRange {
    pub msb: Expr,
    pub lsb: Expr,
}

/// A declared identifier with an optional per-name range, e.g. `bus[3:0]` in
/// `electrical bus[3:0], scalar;`.
#[derive(Debug, Clone)]
pub struct Declarator {
    pub name: Name,
    pub range: Option<BitRange>,
}

#[derive(Debug, Clone)]
pub struct Attr {
    pub name: Name,
    pub val: Option<Expr>,
}
