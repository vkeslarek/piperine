use crate::analog::netlist::{
    BranchIdentifier, AnalogReference, AnalogVariable, NodeIdentifier,
};
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
    /// Simulation stop time
    pub stop_time: Second,

    /// Fixed timestep (used when adaptive=false or as initial timestep when adaptive=true)
    pub dt: Second,

    /// Enable adaptive timestep control (default: false for backward compatibility)
    pub adaptive: bool,

    /// Minimum allowed timestep (default: 1e-15 seconds)
    pub dt_min: Second,

    /// Maximum allowed timestep (default: stop_time / 100)
    pub dt_max: Second,
}

impl TransientAnalysisOptions {
    /// Create a new TransientAnalysisOptions with fixed timestep (backward compatible)
    pub fn new(stop_time: Second, dt: Second) -> Self {
        Self {
            stop_time,
            dt,
            adaptive: false,
            dt_min: 1e-15.into(),
            dt_max: (stop_time / 100.0).into(),
        }
    }

    /// Create a new TransientAnalysisOptions with adaptive timestep control
    pub fn new_adaptive(stop_time: Second, dt_initial: Second) -> Self {
        Self {
            stop_time,
            dt: dt_initial,
            adaptive: true,
            dt_min: 1e-15.into(),
            dt_max: (stop_time / 100.0).into(),
        }
    }

    /// Set minimum timestep
    pub fn with_dt_min(mut self, dt_min: Second) -> Self {
        self.dt_min = dt_min;
        self
    }

    /// Set maximum timestep
    pub fn with_dt_max(mut self, dt_max: Second) -> Self {
        self.dt_max = dt_max;
        self
    }
}

#[derive(Clone)]
pub struct TransientAnalysisContext {
    pub time: Second,
    pub dt: Second,
}

pub trait TransientAnalysis {
    fn load_transient(
        &mut self,
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn load_transient_dynamic(
        &mut self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        vec![]
    }

    fn initial_transient_values(
        &mut self,
        _context: &Context,
    ) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
pub struct TransientAnalysisResult {
    values: Vec<TransientStep>,
}

impl TransientAnalysisResult {
    pub fn new(values: Vec<TransientStep>) -> Self {
        Self {
            values,
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
    values: HashMap<Arc<AnalogVariable>, f64>,
}

impl TransientStep {
    pub fn new(time: f64, values: HashMap<Arc<AnalogVariable>, f64>) -> Self {
        Self { time, values }
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

    pub fn time(&self) -> f64 {
        self.time
    }
}
