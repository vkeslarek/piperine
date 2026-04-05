use crate::node::Node;
use crate::options::{SolverOptions, Variation};
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

#[derive(Debug, Clone)]
pub struct DistortionAnalysis {
    pub variation: Variation,
    pub npoints: u32,
    pub fstart: f64,
    pub fstop: f64,
    /// Ratio F2/F1 for intermodulation analysis. `None` = harmonic analysis only.
    pub f2overf1: Option<f64>,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl DistortionAnalysis {
    /// Harmonic distortion analysis (HD2, HD3) — no F2 frequency.
    pub fn new(variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            variation,
            npoints,
            fstart,
            fstop,
            f2overf1: None,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// Switch to intermodulation (spectral) mode with a second frequency F2 = ratio * fstart.
    ///
    /// `ratio` must be in (0, 1). Ideally irrational; in practice use a fraction A/B
    /// with large coprime integers (e.g., 0.49 ≈ 49/100).
    pub fn intermod(mut self, ratio: f64) -> Self {
        self.f2overf1 = Some(ratio);
        self
    }
}

crate::impl_solver_options!(DistortionAnalysis);
crate::impl_analysis_common!(DistortionAnalysis);

impl SpiceAnalysis for DistortionAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        let mut cmd = format!(
            "disto {} {} {} {}",
            self.variation.to_spice(),
            self.npoints,
            self.fstart,
            self.fstop,
        );
        if let Some(ratio) = self.f2overf1 {
            cmd.push_str(&format!(" {ratio}"));
        }
        lines.push(cmd);
        emit_meas(&self.measurements, "disto", &mut lines);
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disto_harmonic() {
        let cmds = DistortionAnalysis::new(Variation::Dec, 10, 1e3, 1e6)
            .to_spice_control_commands();
        assert_eq!(cmds, vec!["disto dec 10 1000 1000000"]);
    }

    #[test]
    fn disto_intermod() {
        let cmds = DistortionAnalysis::new(Variation::Dec, 10, 1e3, 1e6)
            .intermod(0.9)
            .to_spice_control_commands();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].ends_with(" 0.9"), "expected intermod ratio: {:?}", cmds[0]);
    }
}
