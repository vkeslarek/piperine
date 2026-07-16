use crate::analog::{
    BranchIdentifier, AnalogReference, AnalogVariable, Netlist, NodeIdentifier,
};
use crate::core::net::Net;
use crate::digital::LogicValue;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::solver::Context;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

/// The read-only state an element sees while stamping the DC system: the analog
/// solution history **and** the digital net snapshot it may read (D2A — an
/// analog stamp that depends on digital logic reads it here, with no device-side
/// cache). Derefs to the analog history so existing history access is unchanged.
pub struct DcAnalysisState<'a> {
    history: &'a CircularArrayBuffer2<f64>,
    /// Every digital net's logic value for this solve, indexed by `DigitalNet`.
    pub digital: &'a [LogicValue],
    /// Source-stepping homotopy scale (SPICE): every forced source value is
    /// multiplied by this. `1.0` in normal operation; the DC solver ramps it
    /// `0 → 1` while tracking a hard operating point. Elements that drive forced
    /// sources read it here instead of a mutable `Context` field.
    pub src_scale: f64,
}

impl<'a> DcAnalysisState<'a> {
    pub fn new(
        history: &'a CircularArrayBuffer2<f64>,
        digital: &'a [LogicValue],
        src_scale: f64,
    ) -> Self {
        Self { history, digital, src_scale }
    }

    /// The analog solution history buffer.
    pub fn history(&self) -> &CircularArrayBuffer2<f64> {
        self.history
    }
}

impl Deref for DcAnalysisState<'_> {
    type Target = CircularArrayBuffer2<f64>;
    fn deref(&self) -> &Self::Target {
        self.history
    }
}

pub trait DcAnalysis {
    fn load_dc(
        &mut self,
        dc_circuit_state: &DcAnalysisState<'_>,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn initial_dc_values(&mut self, _context: &Context) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    values: HashMap<Arc<AnalogVariable>, f64>,
    pub stats: crate::result::SolverStats,
}

impl DcAnalysisResult {
    pub fn new(
        values: HashMap<Arc<AnalogVariable>, f64>,
    ) -> Self {
        Self {
            values,
            stats: crate::result::SolverStats::default(),
        }
    }

    /// Replace the default (zeroed) stats with populated values.
    pub fn set_stats(&mut self, stats: crate::result::SolverStats) {
        self.stats = stats;
    }
    pub fn get(&self, variable: impl Into<Arc<AnalogVariable>>) -> Option<f64> {
        self.values.get(&variable.into()).cloned()
    }

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<f64> {
        self.get(AnalogVariable::Node(node_identifier.clone()))
    }

    pub fn get_branch(&self, branch_identifier: impl Into<BranchIdentifier>) -> Option<f64> {
        self.get(AnalogVariable::Branch(branch_identifier.into()))
    }

    pub fn values(&self) -> &HashMap<Arc<AnalogVariable>, f64> {
        &self.values
    }

    pub fn as_iv(&self, netlist: &Netlist) -> Vec<InitialValue<AnalogReference, f64>> {
        let mut initial_values = Vec::with_capacity(self.values.len());
        for (var, value) in &self.values {
            if let Some(reference) = netlist.reference_for(var).cloned() {
                initial_values.push(InitialValue {
                    reference,
                    value: *value,
                });
            }
        }

        initial_values
    }

    /// Read the solved value by [`Net`] — the unified naming layer used by
    /// hosts, diagnostics, and result mappers. Returns `None` for any net
    /// the result does not cover (pseudo nets like ground, or unmapped
    /// digital nets — those live on a separate path).
    pub fn get_net(&self, net: &Net) -> Option<f64> {
        let var = net.analog_variable()?;
        self.values.get(var).copied()
    }
}
