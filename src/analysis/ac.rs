use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::{
    BranchIdentifier, CircuitReference, CircuitVariable, NodeIdentifier,
};
use crate::devices::Component;
use crate::math::linear::Stamp;
use crate::math::unit::Hertz;
use crate::solver::Context;
use num_complex::Complex;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;

pub struct AcAnalysisContext {
    pub frequency: Hertz,
}

pub trait AcAnalysis: Component + DcAnalysis {
    fn update_ac(
        &mut self,
        _dc_analysis_result: &DcAnalysisResult,
        _ac_analysis_context: &AcAnalysisContext,
        _context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_ac(
        &self,
        dc_analysis_result: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>>;
}

pub struct AcFrequencyAnalysisOptions {
    pub frequency: f64,
}

#[derive(Clone, Debug)]
pub struct AcSweepAnalysisOptions {
    pub start_frequency: f64,
    pub stop_frequency: f64,
    pub steps: usize,
    pub logarithmic: bool,
}

pub struct AcAnalysisResult {
    values: Vec<AcAnalysisStep>,
}

impl AcAnalysisResult {
    pub fn new(num_frequencies: usize) -> Self {
        Self {
            values: Vec::with_capacity(num_frequencies),
        }
    }

    pub fn push(&mut self, step: AcAnalysisStep) {
        self.values.push(step)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn get(&self, index: usize) -> Option<&AcAnalysisStep> {
        assert!(index < self.values.len());

        self.values.get(index)
    }

    pub fn iter(&self) -> Iter<'_, AcAnalysisStep> {
        self.values.iter()
    }
}

pub struct AcAnalysisStep {
    frequency: Hertz,
    values: HashMap<Arc<CircuitVariable>, Complex<f64>>,
}

impl AcAnalysisStep {
    pub fn new(frequency: Hertz, values: HashMap<Arc<CircuitVariable>, Complex<f64>>) -> Self {
        Self { frequency, values }
    }

    pub fn get(&self, circuit_var: &CircuitVariable) -> Option<&Complex<f64>> {
        self.values.get(circuit_var)
    }

    pub fn get_branch(
        &self,
        branch_identifier: impl Into<BranchIdentifier>,
    ) -> Option<&Complex<f64>> {
        self.get(&CircuitVariable::Branch(branch_identifier.into()))
    }

    pub fn get_node(&self, node_identifier: impl Into<NodeIdentifier>) -> Option<&Complex<f64>> {
        self.get(&CircuitVariable::Node(node_identifier.into()))
    }
}

pub trait AcAnalysisSolver {
    fn solve_frequency_ac_analysis(
        &self,
        options: AcFrequencyAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<AcAnalysisResult>;

    fn solve_sweep_ac_analysis(
        &self,
        options: AcSweepAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<AcAnalysisResult>;
}
