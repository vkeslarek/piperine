use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysisContext;
use crate::component::Component;
use crate::math::linear::Stamp;
use crate::math::unit::Frequency;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;

pub struct AcAnalysisContext {
    pub frequency: Frequency,
}

pub trait AcAnalysis: Component + DcAnalysis {
    fn update_ac(
        &mut self,
        circuit_states: &CircuitState<Complex<f64>>,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> crate::error::Result<()> {
        Ok(())
    }

    fn load_ac(
        &self,
        circuit_states: &CircuitState<Complex<f64>>,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>>;
}

pub struct AcFrequencyAnalysisOptions {
    pub frequency: f64,
}

pub struct AcSweepAnalysisOptions {
    pub start_frequency: f64,
    pub stop_frequency: f64,
    pub steps: usize,
    pub logarithmic: bool,
}

pub struct AcAnalysisResult {
    pub frequency: f64,
    pub magnitude: f64,
    pub phase: f64,
}

pub trait AcAnalysisSolver {
    fn solve_frequency_ac_analysis(
        &self,
        options: &AcFrequencyAnalysisOptions,
    ) -> crate::error::Result<AcAnalysisResult>;

    fn solve_sweep_ac_analysis(
        &self,
        options: &AcSweepAnalysisOptions,
    ) -> crate::error::Result<Vec<AcAnalysisResult>>;
}
