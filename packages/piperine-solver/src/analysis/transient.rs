use crate::circuit::netlist::{
    BranchIdentifier, CircuitReference, CircuitVariable, NodeIdentifier,
};
use crate::devices::Component;
use crate::devices::soa::SoaViolations;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::unit::Second;
use crate::solver::Context;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;

pub type TransientAnalysisState = CircularArrayBuffer2<f64>;

#[derive(Clone)]
pub struct TransientAnalysisOptions {
    pub stop_time: Second,
    pub dt: Second,
}

#[derive(Clone)]
pub struct TransientAnalysisContext {
    pub time: Second,
    pub dt: Second,
}

pub trait TransientAnalysis: Component {
    fn update_transient(
        &mut self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn load_transient_dynamic(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }

    fn initial_transient_values(
        &self,
        _context: &Context,
    ) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
pub struct TransientAnalysisResult {
    values: Vec<TransientStep>,
    soa_violations: SoaViolations,
}

impl TransientAnalysisResult {
    pub fn new(values: Vec<TransientStep>, soa_violations: SoaViolations) -> Self {
        Self {
            values,
            soa_violations,
        }
    }

    pub fn push(&mut self, step: TransientStep) {
        self.values.push(step)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn get(&self, index: usize) -> Option<&TransientStep> {
        assert!(index < self.values.len());

        self.values.get(index)
    }

    pub fn last(&self) -> Option<&TransientStep> {
        self.values.last()
    }

    pub fn iter(&self) -> Iter<'_, TransientStep> {
        self.values.iter()
    }
}

#[derive(Debug, Clone)]
pub struct TransientStep {
    time: f64,
    values: HashMap<Arc<CircuitVariable>, f64>,
}

impl TransientStep {
    pub fn new(time: f64, values: HashMap<Arc<CircuitVariable>, f64>) -> Self {
        Self { time, values }
    }

    pub fn get(&self, variable: impl Into<Arc<CircuitVariable>>) -> Option<f64> {
        self.values.get(&variable.into()).cloned()
    }

    pub fn get_node(&self, node_identifier: impl Into<NodeIdentifier>) -> Option<f64> {
        self.get(CircuitVariable::Node(node_identifier.into()))
    }

    pub fn get_branch(&self, branch_identifier: impl Into<BranchIdentifier>) -> Option<f64> {
        self.get(CircuitVariable::Branch(branch_identifier.into()))
    }

    pub fn values(&self) -> &HashMap<Arc<CircuitVariable>, f64> {
        &self.values
    }

    pub fn time(&self) -> f64 {
        self.time
    }
}
