use crate::node::Node;
use crate::options::{SolverOptions, Variation};
use crate::spice::{ElementRef, Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

#[derive(Debug, Clone)]
pub struct NoiseAnalysis {
    pub output: Node,
    pub src: ElementRef,
    pub variation: Variation,
    pub npoints: u32,
    pub fstart: f64,
    pub fstop: f64,
    pub pts_per_summary: Option<u32>,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl NoiseAnalysis {
    pub fn new(
        output: Node,
        src: ElementRef,
        variation: Variation,
        npoints: u32,
        fstart: f64,
        fstop: f64,
    ) -> Self {
        Self {
            output,
            src,
            variation,
            npoints,
            fstart,
            fstop,
            pts_per_summary: None,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    pub fn pts_per_summary(mut self, n: u32) -> Self {
        self.pts_per_summary = Some(n);
        self
    }
}

crate::impl_solver_options!(NoiseAnalysis);
crate::impl_analysis_common!(NoiseAnalysis);

impl SpiceAnalysis for NoiseAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        let mut cmd = format!(
            "noise V({}) {} {} {} {} {}",
            self.output,
            self.src.spice_name(),
            self.variation.to_spice(),
            self.npoints,
            self.fstart,
            self.fstop
        );
        if let Some(n) = self.pts_per_summary {
            cmd.push_str(&format!(" {n}"));
        }
        lines.push(cmd);
        emit_meas(&self.measurements, "noise", &mut lines);
        lines
    }
}
