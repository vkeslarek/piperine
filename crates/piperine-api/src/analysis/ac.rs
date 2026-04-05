use crate::node::Node;
use crate::options::{SolverOptions, Variation};
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

/// AC-analysis-specific options.
#[derive(Debug, Clone, Default)]
pub struct AcOptions {
    /// Skip the DC operating point calculation before AC.
    pub noopac: Option<bool>,
}

impl AcOptions {
    pub(crate) fn to_options_string(&self) -> String {
        if Some(true) == self.noopac {
            "noopac".to_string()
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone)]
pub struct AcAnalysis {
    pub variation: Variation,
    pub npoints: u32,
    pub fstart: f64,
    pub fstop: f64,
    pub solver: SolverOptions,
    pub ac_options: AcOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl AcAnalysis {
    pub fn new(variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            variation,
            npoints,
            fstart,
            fstop,
            solver: SolverOptions::default(),
            ac_options: AcOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// Skip DC operating point before AC sweep.
    pub fn noopac(mut self) -> Self {
        self.ac_options.noopac = Some(true);
        self
    }
}

crate::impl_solver_options!(AcAnalysis);
crate::impl_analysis_common!(AcAnalysis);

impl SpiceAnalysis for AcAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let extra = self.ac_options.to_options_string();
        emit_common(&self.solver, &extra, &self.nodesets, &mut lines);
        lines.push(format!(
            "ac {} {} {} {}",
            self.variation.to_spice(),
            self.npoints,
            self.fstart,
            self.fstop
        ));
        emit_meas(&self.measurements, "ac", &mut lines);
        lines
    }
}
