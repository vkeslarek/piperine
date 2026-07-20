//! Statement-level types used by the analog event/flattening machinery.
//! These now carry POM `Expr` (not `IrExpr`) — the resolved-id form is gone.

use piperine_lang::parse::ast::Expr as PomExpr;
use piperine_lang::parse::ast::Stmt as PomStmt;

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
    InitialStep,
    FinalStep,
    Cross { expr: PomExpr, dir: CrossDir },
    Above { expr: PomExpr },
    Timer { period: PomExpr, phase: PomExpr },
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
