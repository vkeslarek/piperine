use crate::analysis::dc::DcAnalysis;
use crate::component::{Component, Context};
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use crate::math::unit::Admittance;

pub struct AcAnalysisContext {
    pub omega: f64,
}

pub trait AcAnalysis: Component + DcAnalysis {
    fn load_ac(
        &self,
        circuit_states: &CircuitStates,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Admittance>>;
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
