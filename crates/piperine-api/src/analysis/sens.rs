use crate::node::Node;
use crate::options::{SolverOptions, Variation};
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

#[derive(Debug, Clone)]
pub struct SensAnalysis {
    pub output: Node,
    pub ac_variation: Option<(Variation, u32, f64, f64)>,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl SensAnalysis {
    /// DC sensitivity analysis.
    pub fn dc(output: Node) -> Self {
        Self {
            output,
            ac_variation: None,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// AC sensitivity analysis.
    pub fn ac(output: Node, variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            output,
            ac_variation: Some((variation, npoints, fstart, fstop)),
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(SensAnalysis);
crate::impl_analysis_common!(SensAnalysis);

impl SpiceAnalysis for SensAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        if let Some((var, np, fs, fe)) = &self.ac_variation {
            lines.push(format!(
                "sens V({}) ac {} {} {} {}",
                self.output,
                var.to_spice(),
                np,
                fs,
                fe
            ));
        } else {
            lines.push(format!("sens V({})", self.output));
        }
        emit_meas(&self.measurements, "sens", &mut lines);
        lines
    }
}
