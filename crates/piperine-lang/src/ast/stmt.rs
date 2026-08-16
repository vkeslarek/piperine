//! Statement AST — LRM chapter 5 (analog) + the 1364 procedural statements
//! Verilog-AMS inherits for digital `always`/`initial` bodies, Annex A.6.

use crate::ast::{Attr, Expr};
use crate::error::PreprocessedSpan;

/// One entry in an event-control expression `@(...)` — LRM §5.6/Annex A.6.5,
/// plus the analog event functions (`cross`/`above`/`timer`/`initial_step`/
/// `final_step`) a `Call` in `Expr` already represents when used as a value;
/// this variant is for the *edge-qualified signal* form specifically.
#[derive(Debug, Clone, PartialEq)]
pub enum EventExpr {
    /// A bare expression becoming true/changing — the general form.
    Level(Expr),
    Posedge(Expr),
    Negedge(Expr),
    /// `@*`/`@(*)` — the implicit event-expression list.
    ImplicitAll,
}

/// A `case`/`casex`/`casez` arm — LRM Annex A.6.7.1.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseArm {
    pub span: PreprocessedSpan,
    /// `None` marks the `default` arm.
    pub labels: Option<Vec<Expr>>,
    pub body: Box<Stmt>,
}

/// A variant of the `case` statement family — §5.6/1364 `case_statement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Case,
    Casex,
    Casez,
    /// `analog_case` — the analog-context case form permitted inside an
    /// `analog` block (Annex C restricts ordinary `case` there).
    AnalogCase,
}

/// A block-scoped item declaration — LRM Annex A.2.8/A.6.2
/// `block_item_declaration`.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockItemDecl {
    Integer { attrs: Vec<Attr>, names: Vec<String> },
    Real { attrs: Vec<Attr>, names: Vec<String> },
    Time { attrs: Vec<Attr>, names: Vec<String> },
    Realtime { attrs: Vec<Attr>, names: Vec<String> },
    Reg { attrs: Vec<Attr>, discipline: Option<String>, names: Vec<String> },
}

/// A procedural or analog statement, LRM Annex A.6.4/A.6.7/A.6.8.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// The empty statement (`;`).
    Null { span: PreprocessedSpan },
    /// `lvalue = expr;` — blocking assignment, §5.5/1364.
    Assign { span: PreprocessedSpan, target: Expr, value: Expr },
    /// `lvalue <= expr;` — non-blocking assignment (digital only).
    NonBlockingAssign { span: PreprocessedSpan, target: Expr, value: Expr },
    /// `V(p,n) <+ expr;` / `I(br) <+ expr;` — a contribution statement,
    /// §5.4. Piperine's PHDL predecessor called this the behavioral-source
    /// primitive (MD-39); this is its Verilog-AMS spelling.
    Contribution { span: PreprocessedSpan, target: Expr, value: Expr },
    /// `begin [: name] block_item_decl* stmt* end` — §Annex A.6.3.
    Block { span: PreprocessedSpan, name: Option<String>, decls: Vec<BlockItemDecl>, stmts: Vec<Stmt> },
    If { span: PreprocessedSpan, cond: Expr, then_branch: Box<Stmt>, else_branch: Option<Box<Stmt>> },
    Case { span: PreprocessedSpan, kind: CaseKind, selector: Expr, arms: Vec<CaseArm> },
    For {
        span: PreprocessedSpan,
        init: Box<Stmt>,
        cond: Expr,
        step: Box<Stmt>,
        body: Box<Stmt>,
    },
    While { span: PreprocessedSpan, cond: Expr, body: Box<Stmt> },
    Repeat { span: PreprocessedSpan, count: Expr, body: Box<Stmt> },
    Forever { span: PreprocessedSpan, body: Box<Stmt> },
    /// `@(events) stmt` — an event-controlled statement, §5.6.
    EventControl { span: PreprocessedSpan, events: Vec<EventExpr>, body: Box<Stmt> },
    /// `#delay stmt` — a delay-controlled statement (digital contexts).
    DelayControl { span: PreprocessedSpan, delay: Expr, body: Box<Stmt> },
    /// `disable name;` — §Annex A.6.4.
    Disable { span: PreprocessedSpan, name: String },
    /// A task call or `$`-system-task call used as a statement.
    TaskCall { span: PreprocessedSpan, name: String, args: Vec<Expr> },
    /// `-> event_name;` — event trigger, §5.10.4/Annex A.6.5. Fires a
    /// named event, waking any `@(event_name)` sensitivity elsewhere.
    EventTrigger { span: PreprocessedSpan, name: String },
}

impl Stmt {
    pub fn span(&self) -> PreprocessedSpan {
        match self {
            Stmt::Null { span }
            | Stmt::Assign { span, .. }
            | Stmt::NonBlockingAssign { span, .. }
            | Stmt::Contribution { span, .. }
            | Stmt::Block { span, .. }
            | Stmt::If { span, .. }
            | Stmt::Case { span, .. }
            | Stmt::For { span, .. }
            | Stmt::While { span, .. }
            | Stmt::Repeat { span, .. }
            | Stmt::Forever { span, .. }
            | Stmt::EventControl { span, .. }
            | Stmt::DelayControl { span, .. }
            | Stmt::Disable { span, .. }
            | Stmt::TaskCall { span, .. }
            | Stmt::EventTrigger { span, .. } => *span,
        }
    }
}
