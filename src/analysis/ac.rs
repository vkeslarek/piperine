use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp2;
use crate::math::unit::Hertz;
use crate::solver::Context;
use num_complex::Complex;

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
    ) -> Vec<Stamp2<CircuitReference, Complex<f64>>>;
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

pub type AcAnalysisResult = CircularArrayBuffer2<Complex<f64>>;

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
