//! What one analysis run holds: the session it runs for, the circuit to run it on, where the
//! solver's observations go, and the host's changes due during it. Each family of analyses has
//! its own runner over this state (`operating_point`, `transient`, `sweep`, `frequency`,
//! `small_signal`, `periodic`, `sensitivity`).

use std::sync::Arc;

use piperine_ir::Ir;
use piperine_solver::prelude::CircuitInstance;

use super::Session;
use crate::held::Held;
use crate::host::HostAction;
use crate::record::Delivery;

/// The state every runner takes: one analysis of one session.
pub(super) struct AnalysisRun<'s> {
    /// The session whose request this run answers.
    pub(super) session: &'s mut Session,
    /// The circuit the analysis runs on.
    pub(super) circuit: CircuitInstance,
    /// Where the solver's observations go.
    pub(super) delivery: Box<Delivery>,
    /// The design, which a fault names its model by.
    pub(super) design: Arc<Ir>,
    /// The params `print` names that no net holds, with their values.
    pub(super) held: Vec<Held>,
    /// The host's drives and sets due after the start of a transient.
    pub(super) timed: Vec<HostAction>,
}
