use crate::node::Node;
use crate::options::{SolverOptions, Variation};
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

#[derive(Debug, Clone)]
pub struct SParamAnalysis {
    pub variation: Variation,
    pub npoints: u32,
    pub fstart: f64,
    pub fstop: f64,
    /// If true, also performs SP noise analysis.
    pub donoise: bool,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl SParamAnalysis {
    pub fn new(variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            variation,
            npoints,
            fstart,
            fstop,
            donoise: false,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// Enable SP noise analysis alongside the S-parameter sweep.
    pub fn donoise(mut self) -> Self {
        self.donoise = true;
        self
    }
}

crate::impl_solver_options!(SParamAnalysis);
crate::impl_analysis_common!(SParamAnalysis);

impl SpiceAnalysis for SParamAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        let mut cmd = format!(
            "sp {} {} {} {}",
            self.variation.to_spice(),
            self.npoints,
            self.fstart,
            self.fstop,
        );
        if self.donoise {
            cmd.push_str(" 1");
        }
        lines.push(cmd);
        emit_meas(&self.measurements, "sp", &mut lines);
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sp_basic() {
        let cmds = SParamAnalysis::new(Variation::Dec, 10, 1e6, 1e9)
            .to_spice_control_commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0], "sp dec 10 1000000 1000000000");
    }

    #[test]
    fn sp_donoise() {
        let cmds = SParamAnalysis::new(Variation::Lin, 100, 1e3, 1e6)
            .donoise()
            .to_spice_control_commands();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].ends_with(" 1"), "expected donoise flag: {:?}", cmds[0]);
    }
}
