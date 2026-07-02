//! IR statements, events, and patterns (SPEC §6, §8, §9).

use super::expr::IrExpr;
use super::symbols::{NatureId, NodeId, VarId};

/// Whether a contribution is resistive or reactive. `Reactive` carries the
/// state slot whose presence classified it; the classification is checked at
/// validation, not assumed (SPEC §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContribKind {
    Resistive,
    Reactive(super::symbols::StateId),
}

/// Digital edge specification for clocked blocks (SPEC §9).
#[derive(Debug, Clone, PartialEq)]
pub enum DigitalEvent {
    Posedge(IrExpr),
    Negedge(IrExpr),
    Change(IrExpr),
    Or(Vec<DigitalEvent>),
}

impl DigitalEvent {
    /// The atomic `(signal, edge)` terms of this event, flattening `Or`.
    pub fn terms(&self) -> Vec<(&IrExpr, EdgeKind)> {
        match self {
            DigitalEvent::Posedge(e) => vec![(e, EdgeKind::Rising)],
            DigitalEvent::Negedge(e) => vec![(e, EdgeKind::Falling)],
            DigitalEvent::Change(e) => vec![(e, EdgeKind::Any)],
            DigitalEvent::Or(events) => events.iter().flat_map(DigitalEvent::terms).collect(),
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
#[derive(Debug, Clone, PartialEq)]
pub enum EventSource {
    InitialStep,
    FinalStep,
    Cross { expr: IrExpr, dir: CrossDir },
    Above { expr: IrExpr },
    Timer { period: IrExpr },
}

/// An analog event: a trigger plus a statement body.
#[derive(Debug, Clone)]
pub struct IrAnalogEvent {
    pub source: EventSource,
    pub body: Vec<IrStmt>,
}

/// An assignment target.
#[derive(Debug, Clone, PartialEq)]
pub enum Lval {
    Var(VarId),
    Net(NodeId),
    Index(Box<Lval>, IrExpr),
    Slice(Box<Lval>, IrExpr, IrExpr, bool),
}

/// A three-valued bit in a `Match` bit pattern: `0`, `1`, or don't-care.
/// Distinct from the `Quad` value X.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trit {
    Zero,
    One,
    DontCare,
}

/// A `Match` arm pattern (SPEC §8).
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Value(IrExpr),
    BitPattern(Vec<Trit>),
    Wildcard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Fatal,
}

/// The single statement set. Each body admits a subset, enforced by
/// validation (SPEC §11).
#[derive(Debug, Clone)]
pub enum IrStmt {
    // ── Analog only ──
    /// `<+` — accumulates on the branch.
    Contrib {
        nature: NatureId,
        plus: NodeId,
        minus: NodeId,
        expr: IrExpr,
        kind: ContribKind,
    },
    /// `<-` — single-driver ideal source / short.
    Force { nature: NatureId, plus: NodeId, minus: NodeId, expr: IrExpr },
    AnalogEvent(IrAnalogEvent),
    // ── Digital and analog-sequential ──
    /// Combinational or register assignment (context decides, SPEC §9); in an
    /// analog body, a sequential variable binding.
    Assign { lval: Lval, expr: IrExpr },
    /// A register-update block driven by a digital edge event.
    ClockedBlock { event: DigitalEvent, body: Vec<IrStmt> },
    // ── Shared control ──
    If { cond: IrExpr, then_: Vec<IrStmt>, else_: Vec<IrStmt> },
    Match {
        scrutinee: IrExpr,
        arms: Vec<(Pattern, Vec<IrStmt>)>,
        default: Vec<IrStmt>,
    },
    VarDecl { var: VarId, init: Option<IrExpr> },
    /// Function bodies only.
    Return(Option<IrExpr>),
    // ── Simulator control ──
    BoundStep(IrExpr),
    Finish,
    Discontinuity(u8),
    /// `{}` placeholders in `format` interpolate `args` in order.
    Diagnostic { severity: Severity, format: String, args: Vec<IrExpr> },
}

impl IrStmt {
    /// The nested statement bodies of this statement.
    pub fn bodies(&self) -> Vec<&[IrStmt]> {
        match self {
            IrStmt::AnalogEvent(ev) => vec![&ev.body],
            IrStmt::ClockedBlock { body, .. } => vec![body],
            IrStmt::If { then_, else_, .. } => vec![then_, else_],
            IrStmt::Match { arms, default, .. } => {
                let mut out: Vec<&[IrStmt]> = arms.iter().map(|(_, b)| b.as_slice()).collect();
                out.push(default);
                out
            }
            _ => Vec::new(),
        }
    }

    /// The expressions read directly by this statement (not recursing into
    /// nested bodies).
    pub fn exprs(&self) -> Vec<&IrExpr> {
        match self {
            IrStmt::Contrib { expr, .. } | IrStmt::Force { expr, .. } => vec![expr],
            IrStmt::AnalogEvent(ev) => match &ev.source {
                EventSource::Cross { expr, .. } | EventSource::Above { expr } => vec![expr],
                EventSource::Timer { period } => vec![period],
                EventSource::InitialStep | EventSource::FinalStep => Vec::new(),
            },
            IrStmt::Assign { expr, .. } => vec![expr],
            IrStmt::ClockedBlock { event, .. } => {
                event.terms().into_iter().map(|(e, _)| e).collect()
            }
            IrStmt::If { cond, .. } => vec![cond],
            IrStmt::Match { scrutinee, arms, .. } => {
                let mut out = vec![scrutinee];
                for (pattern, _) in arms {
                    if let Pattern::Value(e) = pattern {
                        out.push(e);
                    }
                }
                out
            }
            IrStmt::VarDecl { init, .. } => init.iter().collect(),
            IrStmt::Return(e) => e.iter().collect(),
            IrStmt::BoundStep(e) => vec![e],
            IrStmt::Diagnostic { args, .. } => args.iter().collect(),
            IrStmt::Finish | IrStmt::Discontinuity(_) => Vec::new(),
        }
    }
}
