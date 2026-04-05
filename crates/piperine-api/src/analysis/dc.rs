use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::{ElementRef, Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

/// Optional second source for a nested (dual) DC sweep.
#[derive(Debug, Clone)]
pub struct DcSweep2 {
    pub source: ElementRef,
    pub start: f64,
    pub stop: f64,
    pub step: f64,
}

#[derive(Debug, Clone)]
pub struct DcAnalysis {
    pub source: ElementRef,
    pub start: f64,
    pub stop: f64,
    pub step: f64,
    /// Optional nested sweep (second source).
    pub sweep2: Option<DcSweep2>,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl DcAnalysis {
    pub fn new(source: ElementRef, start: f64, stop: f64, step: f64) -> Self {
        Self {
            source,
            start,
            stop,
            step,
            sweep2: None,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// Add a nested (dual) sweep over a second source.
    pub fn with_sweep2(mut self, source: ElementRef, start: f64, stop: f64, step: f64) -> Self {
        self.sweep2 = Some(DcSweep2 { source, start, stop, step });
        self
    }
}

crate::impl_solver_options!(DcAnalysis);
crate::impl_analysis_common!(DcAnalysis);

impl SpiceAnalysis for DcAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        let mut cmd = format!(
            "dc {} {} {} {}",
            self.source.spice_name(),
            self.start,
            self.stop,
            self.step
        );
        if let Some(s2) = &self.sweep2 {
            cmd.push_str(&format!(
                " {} {} {} {}",
                s2.source.spice_name(),
                s2.start,
                s2.stop,
                s2.step
            ));
        }
        lines.push(cmd);
        emit_meas(&self.measurements, "dc", &mut lines);
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spice::ElementRef;

    fn src(symbol: &'static str, instance: &str) -> ElementRef {
        ElementRef::new(symbol, instance)
    }

    #[test]
    fn dc_single_unchanged() {
        let cmds = DcAnalysis::new(src("V", "1"), 0.0, 5.0, 0.1)
            .to_spice_control_commands();
        assert_eq!(cmds, vec!["dc V1 0 5 0.1"]);
    }

    #[test]
    fn dc_dual_sweep() {
        let cmds = DcAnalysis::new(src("V", "DS"), 0.0, 10.0, 0.5)
            .with_sweep2(src("V", "GS"), 0.0, 5.0, 1.0)
            .to_spice_control_commands();
        assert_eq!(cmds, vec!["dc VDS 0 10 0.5 VGS 0 5 1"]);
    }
}
