use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::{
    BranchIdentifier, CircuitReference, CircuitVariable, NodeIdentifier,
};
use crate::devices::soa::SoaViolations;
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

pub trait AcAnalysis: DcAnalysis {
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
    soa_violations: SoaViolations,
}

impl AcAnalysisResult {
    pub fn new(values: Vec<AcAnalysisStep>, soa_violations: SoaViolations) -> Self {
        Self {
            values,
            soa_violations,
        }
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

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<&Complex<f64>> {
        self.get(&CircuitVariable::Node(node_identifier.clone()))
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
