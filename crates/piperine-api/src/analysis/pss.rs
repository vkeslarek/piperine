use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

/// Periodic Steady State (PSS) analysis.
///
/// **Warning:** PSS is experimental in ngspice and may not be available in all builds.
/// Only works on autonomous circuits (oscillators). Run a `.tran` first to determine
/// a good `tstab` value.
#[derive(Debug, Clone)]
pub struct PssAnalysis {
    /// Guessed fundamental frequency.
    pub gfreq: f64,
    /// Stabilization time before the shooting method starts (seconds).
    pub tstab: f64,
    /// Node or branch where oscillation is expected.
    pub oscnob: Node,
    /// Number of time steps in the predicted period (should be > 2 × harms).
    pub psspoints: u32,
    /// Number of harmonics to compute.
    pub harms: u32,
    /// Maximum shooting cycle iterations.
    pub sciter: u32,
    /// Global convergence error threshold. Lower = more accurate, slower.
    pub steadycoeff: f64,
    /// Skip OP calculation, use element IC= values.
    pub uic: bool,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl PssAnalysis {
    pub fn new(gfreq: f64, tstab: f64, oscnob: Node, psspoints: u32, harms: u32) -> Self {
        Self {
            gfreq,
            tstab,
            oscnob,
            psspoints,
            harms,
            sciter: 50,
            steadycoeff: 1e-3,
            uic: false,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    pub fn sciter(mut self, n: u32) -> Self {
        self.sciter = n;
        self
    }

    pub fn steadycoeff(mut self, v: f64) -> Self {
        self.steadycoeff = v;
        self
    }

    pub fn uic(mut self) -> Self {
        self.uic = true;
        self
    }
}

crate::impl_solver_options!(PssAnalysis);
crate::impl_analysis_common!(PssAnalysis);

impl SpiceAnalysis for PssAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        let mut cmd = format!(
            "pss {} {} {} {} {} {} {}",
            self.gfreq,
            self.tstab,
            self.oscnob,
            self.psspoints,
            self.harms,
            self.sciter,
            self.steadycoeff,
        );
        if self.uic {
            cmd.push_str(" UIC");
        }
        lines.push(cmd);
        emit_meas(&self.measurements, "pss", &mut lines);
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pss_basic_defaults() {
        let cmds = PssAnalysis::new(1e6, 1e-3, Node::from("osc"), 1024, 10)
            .to_spice_control_commands();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("50"), "sciter missing: {:?}", cmds[0]);
        assert!(cmds[0].contains("0.001"), "steadycoeff missing: {:?}", cmds[0]);
        assert!(!cmds[0].contains("UIC"));
    }

    #[test]
    fn pss_uic() {
        let cmds = PssAnalysis::new(1e6, 1e-3, Node::from("osc"), 1024, 10)
            .uic()
            .to_spice_control_commands();
        assert!(cmds[0].ends_with(" UIC"), "expected UIC: {:?}", cmds[0]);
    }

    #[test]
    fn pss_custom_sciter_steadycoeff() {
        let cmds = PssAnalysis::new(150.0, 0.2, Node::from("oscnode"), 1024, 11)
            .sciter(100)
            .steadycoeff(5e-3)
            .to_spice_control_commands();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("100"), "sciter: {:?}", cmds[0]);
        assert!(cmds[0].contains("0.005"), "steadycoeff: {:?}", cmds[0]);
    }
}
