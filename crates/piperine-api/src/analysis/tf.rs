use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::{ElementRef, Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

#[derive(Debug, Clone)]
pub struct TfAnalysis {
    pub output: Node,
    pub input_source: ElementRef,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl TfAnalysis {
    pub fn new(output: Node, input_source: ElementRef) -> Self {
        Self {
            output,
            input_source,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(TfAnalysis);
crate::impl_analysis_common!(TfAnalysis);

impl SpiceAnalysis for TfAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        lines.push(format!(
            "tf V({}) {}",
            self.output,
            self.input_source.spice_name()
        ));
        emit_meas(&self.measurements, "tf", &mut lines);
        lines
    }
}
