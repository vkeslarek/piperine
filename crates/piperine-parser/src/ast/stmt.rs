//! Statements and blocks.
//!
//! Mirrors `veriloga.ungram` from OpenVAF-Reloaded.

use super::*;

/// ungram: `Stmt = EmptyStmt | AssignStmt | ExprStmt | IfStmt | WhileStmt
///                | ForStmt | CaseStmt | EventStmt | BlockStmt`
#[derive(Debug, Clone)]
pub enum Stmt {
    Empty(EmptyStmt),
    Assign(AssignStmt),
    Expr(ExprStmt),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Case(CaseStmt),
    Event(EventStmt),
    Block(BlockStmt),
    Assert(AssertStmt),
    AssertRun(AssertStmt),
    AssertWarn(AssertStmt),
}

#[derive(Debug, Clone)]
pub struct EmptyStmt {
    pub attrs: Vec<Attr>,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub attrs: Vec<Attr>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub attrs: Vec<Attr>,
    pub assign: Assign,
}

/// ungram: `IfStmt = AttrList* 'if' '(' condition:Expr ')' then_branch:Stmt ('else' else_branch:Stmt)?`
#[derive(Debug, Clone)]
pub struct IfStmt {
    pub attrs: Vec<Attr>,
    pub condition: Expr,
    pub then_branch: Box<Stmt>,
    pub else_branch: Option<Box<Stmt>>,
}

/// ungram: `WhileStmt = AttrList* 'while' '(' condition:Expr ')' body:Stmt`
#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub attrs: Vec<Attr>,
    pub condition: Expr,
    pub body: Box<Stmt>,
}

/// ungram: `ForStmt = AttrList* 'for' '(' init:Stmt ';' condition:Expr ';' incr:Stmt ')' for_body:Stmt`
#[derive(Debug, Clone)]
pub struct ForStmt {
    pub attrs: Vec<Attr>,
    pub init: Box<Stmt>,
    pub condition: Expr,
    pub incr: Box<Stmt>,
    pub for_body: Box<Stmt>,
}

/// ungram: `CaseStmt = AttrList* 'case' '(' discriminant:Expr ')' Case* 'endcase'`
#[derive(Debug, Clone)]
pub struct CaseStmt {
    pub attrs: Vec<Attr>,
    pub discriminant: Expr,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone)]
pub enum CaseItem {
    Exprs(Vec<Expr>),
    Default,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub item: CaseItem,
    pub stmt: Box<Stmt>,
}

/// ungram: `EventStmt = AttrList* '@' '(' ('initial_step'|'final_step') ... ')' Stmt`
/// We accept any expression for the event to support Piperine extensions like
/// `@(cross(...))` and `@(above(...))`.
#[derive(Debug, Clone)]
pub struct EventStmt {
    pub attrs: Vec<Attr>,
    pub event: Expr,
    pub stmt: Box<Stmt>,
}

/// ungram: `BlockStmt = AttrList* 'begin' BlockScope? BlockItem* 'end'`
#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub attrs: Vec<Attr>,
    pub label: Option<Name>,
    pub items: Vec<BlockItem>,
}

/// ungram: `BlockItem = VarDecl | ParamDecl | Stmt`
#[derive(Debug, Clone)]
pub enum BlockItem {
    VarDecl(VarDecl),
    ParamDecl(ParamDecl),
    Stmt(Stmt),
}

#[derive(Debug, Clone)]
pub struct AssertStmt {
    pub attrs: Vec<Attr>,
    pub condition: Expr,
    pub message: Option<Expr>,
}
