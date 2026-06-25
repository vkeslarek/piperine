use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::{
    BranchIdentifier, AnalogReference, AnalogVariable, NodeIdentifier,
};
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
    ) -> Vec<Stamp<AnalogReference, Complex<f64>>>;
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

impl AcSweepAnalysisOptions {
    /// Generates frequency points for the sweep.
    ///
    /// # Returns
    ///
    /// A vector of frequencies distributed between `start_frequency` and `stop_frequency`.
    /// If `logarithmic` is true, uses logarithmic spacing; otherwise uses linear spacing.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let options = AcSweepAnalysisOptions {
    ///     start_frequency: 1.0,
    ///     stop_frequency: 1000.0,
    ///     steps: 3,
    ///     logarithmic: true,
    /// };
    /// let freqs = options.generate_frequencies();
    /// // freqs ≈ [1.0, 31.62, 1000.0] (logarithmic spacing)
    /// ```
    pub fn generate_frequencies(&self) -> Vec<f64> {
        if self.steps <= 1 {
            return vec![self.start_frequency];
        }

        (0..self.steps)
            .map(|i| {
                let ratio = i as f64 / (self.steps - 1) as f64;
                if self.logarithmic {
                    // Logarithmic spacing: f = f_start * (f_stop / f_start)^ratio
                    self.start_frequency * (self.stop_frequency / self.start_frequency).powf(ratio)
                } else {
                    // Linear spacing: f = f_start + (f_stop - f_start) * ratio
                    self.start_frequency + (self.stop_frequency - self.start_frequency) * ratio
                }
            })
            .collect()
    }
}

pub struct AcAnalysisResult {
    values: Vec<AcAnalysisStep>,
}

impl AcAnalysisResult {
    pub fn new(values: Vec<AcAnalysisStep>) -> Self {
        Self {
            values,
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
    pub frequency: Hertz,
    values: HashMap<Arc<AnalogVariable>, Complex<f64>>,
}

impl AcAnalysisStep {
    pub fn new(frequency: Hertz, values: HashMap<Arc<AnalogVariable>, Complex<f64>>) -> Self {
        Self { frequency, values }
    }

    pub fn get(&self, circuit_var: &AnalogVariable) -> Option<&Complex<f64>> {
        self.values.get(circuit_var)
    }

    pub fn get_branch(
        &self,
        branch_identifier: impl Into<BranchIdentifier>,
    ) -> Option<&Complex<f64>> {
        self.get(&AnalogVariable::Branch(branch_identifier.into()))
    }

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<&Complex<f64>> {
        self.get(&AnalogVariable::Node(node_identifier.clone()))
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
