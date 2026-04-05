use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

#[derive(Debug, Clone, Default)]
pub struct OpAnalysis {
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl OpAnalysis {
    pub fn new() -> Self {
        Self::default()
    }
}

crate::impl_solver_options!(OpAnalysis);
crate::impl_analysis_common!(OpAnalysis);

impl SpiceAnalysis for OpAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        lines.push("op".to_string());
        emit_meas(&self.measurements, "op", &mut lines);
        lines
    }
}
