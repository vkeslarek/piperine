//! A [`Fixture`] is a hand-written IR module plus a stimulus program and a
//! set of named observation points — the unit the differential harness
//! (`harness.rs`) runs under two evaluators and compares (LOV-11).
//!
//! The reference interpreter (`piperine_ir::interp::Interpreter`) does not
//! solve KCL and does not drive itself: analog node values and digital
//! signal words are GIVEN to it as inputs (see `interp.rs`'s module doc).
//! A [`Step`] supplies exactly what the interpreter has no way to compute
//! on its own — this mirrors `Interpreter::set_node_values`/`set_signal`/
//! `seed_value` one for one.

use piperine_ir::core::{CellId, Module, NetId, ValueId};
use piperine_ir::interp::CellValue;

/// One step of external stimulus applied before an evaluator advances.
#[derive(Debug, Clone, Default)]
pub struct Step {
    /// Advance the clock to this instant before evaluating triggers.
    /// `None` re-evaluates at the current instant — used to arm an edge
    /// detector without moving time (mirrors `Interpreter::step` vs
    /// `step_to`).
    pub time: Option<f64>,
    /// Value slots seeded directly: a Timer's period/phase, an Edge's
    /// sampled bit, a constant operand with no writing op.
    pub seed_values: Vec<(ValueId, CellValue)>,
    /// Conservative node values (across, through) supplied as if a solver
    /// had already found them.
    pub node_values: Vec<(NetId, f64, f64)>,
    /// Digital signal words supplied on a `Signal` net.
    pub signal_values: Vec<(NetId, u128)>,
}

/// Where in the module an evaluator should read back a value, labelled for
/// disagreement reporting.
#[derive(Debug, Clone, Copy)]
pub enum ObservePoint {
    Cell(CellId),
    Value(ValueId),
}

/// One named observation point.
#[derive(Debug, Clone, Copy)]
pub struct Observation {
    pub label: &'static str,
    pub point: ObservePoint,
}

/// The per-fixture agreement tolerance. Declared ONCE per fixture, here —
/// never a second copy in a test body (Done-when: "tolerance is declared
/// per fixture in exactly one place").
#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    /// Absolute tolerance for `Real` comparisons. `Bits`/`Array` always
    /// compare exactly, regardless of this value.
    pub abs: f64,
}

impl Tolerance {
    /// Bit-for-bit / bit-for-bit agreement — the default for fixtures with
    /// no floating-point rounding in play.
    pub const EXACT: Tolerance = Tolerance { abs: 0.0 };
}

/// A hand-written IR fixture: the module, its stimulus program, its
/// observation points, and its declared tolerance.
pub struct Fixture {
    pub name: &'static str,
    pub module: Module,
    pub steps: Vec<Step>,
    pub observe: Vec<Observation>,
    pub tolerance: Tolerance,
}
