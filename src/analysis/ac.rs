use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::math::linear::Stamp;
use crate::math::unit::Hertz;
use crate::math::vector::IndexedVec1;
use crate::solver::Context;
use num_complex::Complex;

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

pub type AcAnalysisResult = IndexedVec1<CircuitReference, Complex<f64>>;

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
