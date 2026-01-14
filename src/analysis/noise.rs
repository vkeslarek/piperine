use crate::analysis::ac::{AcAnalysis, AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult};
use crate::circuit::netlist::{CircuitReference, NodeIdentifier};
use crate::devices::Component;
use std::collections::HashMap;
use crate::math::unit::AmpereSquaredSecond;

pub struct Noise {
    pub terminals: (CircuitReference, CircuitReference),
    pub value: AmpereSquaredSecond,
}

pub trait NoiseSource: Component + AcAnalysis + DcAnalysis {
    fn noise_current_psd(
        &self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
    ) -> Vec<Noise>;
}
#[derive(Clone, Debug)]
pub struct NoiseAnalysisOptions {
    pub sweep_options: AcSweepAnalysisOptions,
    pub output_node: NodeIdentifier,
    pub reference_node: NodeIdentifier,
    pub input_source_name: Option<String>,
}

pub struct NoiseAnalysisResult {
    pub mapping: HashMap<CircuitReference, usize>,
    pub frequencies: Vec<f64>,
    pub out_noise_sq: Vec<f64>,
    pub integrated_noise: f64,
}
