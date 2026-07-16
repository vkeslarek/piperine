use crate::analysis::ac::{AcAnalysis, AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysis;
use crate::prelude::DcAnalysisResult;
use crate::analog::{AnalogReference, NodeIdentifier};
use crate::math::unit::AmpereSquaredSecond;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoiseKind { Thermal, Shot, Flicker, Other }

pub struct Noise {
    pub terminals: (AnalogReference, AnalogReference),
    pub value: AmpereSquaredSecond,
    pub name: Option<String>,
    pub kind: NoiseKind,
}

impl Noise {
    pub fn new(terminals: (AnalogReference, AnalogReference), value: AmpereSquaredSecond) -> Self {
        Self { terminals, value, name: None, kind: NoiseKind::Other }
    }
    pub fn named(mut self, name: impl Into<String>, kind: NoiseKind) -> Self {
        self.name = Some(name.into()); self.kind = kind; self
    }
}

pub trait NoiseSource: AcAnalysis + DcAnalysis {
    fn noise_current_psd(
        &mut self,
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



/// Per-analysis config for noise. Carries the sweep, output/reference nodes,
/// and optional input source name.
#[derive(Debug, Clone)]
pub struct NoiseContext {
    pub options: NoiseAnalysisOptions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_new_defaults() {
        let n1 = AnalogReference::ground();
        let n2 = AnalogReference::ground();
        let noise = Noise::new((n1.clone(), n2.clone()), 1.0);
        assert_eq!(noise.name, None);
        assert_eq!(noise.kind, NoiseKind::Other);
    }
    
    #[test]
    fn noise_named() {
        let n1 = AnalogReference::ground();
        let n2 = AnalogReference::ground();
        let noise = Noise::new((n1.clone(), n2.clone()), 1.0).named("rn", NoiseKind::Thermal);
        assert_eq!(noise.name, Some("rn".to_string()));
        assert_eq!(noise.kind, NoiseKind::Thermal);
    }
}
