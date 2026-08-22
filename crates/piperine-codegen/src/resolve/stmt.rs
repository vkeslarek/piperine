//! Statement-level types used by the analog event/flattening machinery.
//! They carry POM `Expr` directly.

use piperine_lang::ast::Expr as PomExpr;
use piperine_lang::ast::Stmt as PomStmt;

use super::symbols::StateId;

/// Whether a contribution is resistive or reactive. `Reactive` carries the
/// state slot whose presence classified it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContribKind {
    Resistive,
    Reactive(StateId),
}

/// Digital edge specification for clocked blocks (SPEC §9).
#[derive(Debug, Clone)]
pub enum DigitalEvent {
    Posedge(PomExpr),
    Negedge(PomExpr),
    Change(PomExpr),
    /// Fires once at simulation start.
    Initial,
    /// Fires once at simulation end.
    Final,
    Or(Vec<DigitalEvent>),
}

impl DigitalEvent {
    /// The atomic `(signal, edge)` terms of this event, flattening `Or`.
    pub fn terms(&self) -> Vec<(&PomExpr, EdgeKind)> {
        match self {
            DigitalEvent::Posedge(e) => vec![(e, EdgeKind::Rising)],
            DigitalEvent::Negedge(e) => vec![(e, EdgeKind::Falling)],
            DigitalEvent::Change(e) => vec![(e, EdgeKind::Any)],
            DigitalEvent::Initial | DigitalEvent::Final => Vec::new(),
            DigitalEvent::Or(events) => events.iter().flat_map(DigitalEvent::terms).collect(),
        }
    }

    /// Returns `true` if this event (or any sub-event in an `Or`) is `Initial`.
    pub fn is_initial(&self) -> bool {
        match self {
            DigitalEvent::Initial => true,
            DigitalEvent::Or(events) => events.iter().any(DigitalEvent::is_initial),
            _ => false,
        }
    }
}

/// The edge polarity of one atomic digital event term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Rising,
    Falling,
    Any,
}

/// Crossing direction for analog `cross` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossDir {
    Either,
    Rising,
    Falling,
}

/// The trigger of an analog event (SPEC §6.1).
#[derive(Debug, Clone)]
pub enum EventSource {
    /// `initial_step` / `initial_step("dc", "tran", ...)` — LRM §5.10.2.
    /// The analysis-name list (empty = every analysis, the bare-form
    /// default) filters which analyses fire it (LRMW2-08).
    InitialStep { analyses: Vec<String> },
    /// `final_step` / `final_step("dc", "tran", ...)` — LRM §5.10.2. Same
    /// analysis-name-list filtering as `InitialStep`.
    FinalStep { analyses: Vec<String> },
    /// `cross(expr[, dir[, time_tol[, expr_tol[, enable]]]])` — §5.10.3.1.
    /// `time_tol`/`expr_tol` (LRMW2-33) are `constant_expression`s
    /// (elaboration-time constant, `None` when omitted, evaluated once per
    /// instance the same way `Ddt`'s own `abstol` is); `enable` is an
    /// `analog_expression` but, like `AbsDelta`'s own `enable` below,
    /// treated as an elaboration-time constant here (documented scope
    /// limit) — `1.0` (always active) when omitted, the LRM's own default.
    Cross { expr: PomExpr, dir: CrossDir, time_tol: Option<PomExpr>, expr_tol: Option<PomExpr>, enable: PomExpr },
    /// `above(expr[, time_tol[, expr_tol[, enable]]])` — §5.10.3.2. Same
    /// tolerance/enable treatment as `Cross` above.
    Above { expr: PomExpr, time_tol: Option<PomExpr>, expr_tol: Option<PomExpr>, enable: PomExpr },
    Timer { period: PomExpr, phase: PomExpr },
    /// `absdelta(expr, delta, [time_tol, [expr_tol, [enable]]])` —
    /// §5.10.3.4, LRMC-C5-03. `delta`/`time_tol`/`expr_tol`/`enable` are
    /// evaluated once at elaboration (`pom_eval_const`), mirroring
    /// `Timer`'s existing `period`/`phase` treatment above — only `expr`
    /// is a genuine per-step trigger row.
    AbsDelta { expr: PomExpr, delta: PomExpr, time_tol: PomExpr, expr_tol: PomExpr, enable: PomExpr },
    /// `@(signal_name)` — a bare signal/named-event sensitivity (§5.6,
    /// §5.10.4): fires whenever `expr`'s value changes, in either
    /// direction (`piperine_ir::events::Trigger::AnyChange`) — LRMC-C5-04.
    Named { expr: PomExpr },
}

/// An analog event: a trigger plus a statement body.
#[derive(Debug, Clone)]
pub struct AnalogEvent {
    pub source: EventSource,
    pub body: Vec<PomStmt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Fatal,
}
