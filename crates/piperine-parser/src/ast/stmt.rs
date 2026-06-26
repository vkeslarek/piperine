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
    Repeat(RepeatStmt),
    Forever(ForeverStmt),
    NonBlockingAssign(NonBlockingAssignStmt),
    Wait(WaitStmt),
    Fork(ForkStmt),
    Disable(DisableStmt),
    EventTrigger(EventTriggerStmt),
    ProceduralAssign(ProceduralAssignStmt),
    ProceduralDeassign(ProceduralDeassignStmt),
    IndirectContrib(IndirectContribution),
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

/// ungram: `CaseStmt = AttrList* ('case'|'casex'|'casez') '(' discriminant:Expr ')' Case* 'endcase'`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Case,   // exact match
    Casex,  // x/z bits are don't-cares
    Casez,  // z bits are don't-cares
}

#[derive(Debug, Clone)]
pub struct CaseStmt {
    pub attrs: Vec<Attr>,
    pub kind: CaseKind,
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

/// `repeat (count) body` — run `body` `count` times.
#[derive(Debug, Clone)]
pub struct RepeatStmt {
    pub attrs: Vec<Attr>,
    pub count: Expr,
    pub body: Box<Stmt>,
}

/// `forever body` — loop until `break`/`return`/`$finish`.
#[derive(Debug, Clone)]
pub struct ForeverStmt {
    pub attrs: Vec<Attr>,
    pub body: Box<Stmt>,
}

// ==========================================
// Phase 4 Extensions
// ==========================================

#[derive(Debug, Clone)]
pub struct NonBlockingAssignStmt {
    pub attrs: Vec<Attr>,
    pub lvalue: Expr,
    pub delay_or_event: Option<TimingControl>,
    pub rvalue: Expr,
}

#[derive(Debug, Clone)]
pub enum TimingControl {
    Delay(Expr),                      // #delay_value
    DelayParen(Expr),                 // #(mintypmax_expr)
    Event(EventControl),
}

#[derive(Debug, Clone)]
pub enum EventControl {
    Ident(Path),                      // @ident
    Expr(Vec<EventExpr>),             // @(event_expression)
    Star,                             // @* or @(*)
}

#[derive(Debug, Clone)]
pub enum EventExpr {
    Expr(Expr),
    Posedge(Expr),
    Negedge(Expr),
    Ident(Path),                      // hierarchical_event_identifier
    DriverUpdate(Expr),               // driver_update expression (AMS)
    AnalogEventFn(Expr),              // cross(...), above(...), timer(...)
    Or(Box<EventExpr>, Box<EventExpr>),
}

#[derive(Debug, Clone)]
pub struct WaitStmt {
    pub attrs: Vec<Attr>,
    pub condition: Expr,
    pub stmt: Box<Stmt>,
}

#[derive(Debug, Clone)]
pub struct ForkStmt {
    pub attrs: Vec<Attr>,
    pub label: Option<Name>,
    pub items: Vec<BlockItem>,  // BlockItem = declaration or statement
}

#[derive(Debug, Clone)]
pub struct DisableStmt {
    pub attrs: Vec<Attr>,
    pub target: Path,
}

#[derive(Debug, Clone)]
pub struct EventTriggerStmt {
    pub attrs: Vec<Attr>,
    pub event: Path,
}

#[derive(Debug, Clone)]
pub struct ProceduralAssignStmt {
    pub attrs: Vec<Attr>,
    pub is_force: bool,  // true if `force`, false if `assign`
    pub lvalue: Expr,
    pub rvalue: Expr,
}

#[derive(Debug, Clone)]
pub struct ProceduralDeassignStmt {
    pub attrs: Vec<Attr>,
    pub is_release: bool, // true if `release`, false if `deassign`
    pub lvalue: Expr,
}

#[derive(Debug, Clone)]
pub struct IndirectContribution {
    pub attrs: Vec<Attr>,
    pub lvalue: Expr,
    pub indirect_expr: Expr,
    pub rvalue: Expr,
}
