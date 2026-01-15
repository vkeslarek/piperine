use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::math::linear::Stamp;
use crate::math::unit::Hertz;
use crate::solver::Context;
use ndarray::Array2;
use num_complex::Complex;
use std::collections::HashMap;

pub struct AcAnalysisContext {
    pub frequency: Hertz,
}

pub trait AcAnalysis: Component + DcAnalysis {
    fn update_ac(
        &mut self,
        dc_analysis_result: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
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
    pub mapping: HashMap<CircuitReference, usize>,
    pub frequencies: Vec<f64>,
    pub data: Array2<Complex<f64>>, // [Frequency_Index, Node_Index]
}

impl AcAnalysisResult {
    pub fn get_phasor(
        &self,
        reference: &CircuitReference,
        freq_idx: usize,
    ) -> Option<Complex<f64>> {
        let col = *self.mapping.get(reference)?;
        Some(self.data[[freq_idx, col]])
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
