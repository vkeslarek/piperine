use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::{Measurement, Probe, SpiceAnalysis};
use super::{emit_common, emit_meas};

/// Numerical integration method for transient analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationMethod {
    Trap,
    Gear,
}

impl IntegrationMethod {
    pub fn to_spice(&self) -> &'static str {
        match self {
            IntegrationMethod::Trap => "trap",
            IntegrationMethod::Gear => "gear",
        }
    }
}

/// Transient-analysis-specific solver options.
#[derive(Debug, Clone, Default)]
pub struct TranOptions {
    pub method: Option<IntegrationMethod>,
    pub maxord: Option<u32>,
    pub trtol: Option<f64>,
    pub chgtol: Option<f64>,
    pub xmu: Option<f64>,
    pub itl3: Option<u32>,
    pub itl4: Option<u32>,
    pub itl5: Option<u32>,
    pub ramptime: Option<f64>,
    pub autostop: Option<bool>,
    pub interp: Option<bool>,
}

impl TranOptions {
    pub(crate) fn to_options_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref m) = self.method {
            parts.push(format!("method={}", m.to_spice()));
        }
        if let Some(v) = self.maxord {
            parts.push(format!("maxord={v}"));
        }
        if let Some(v) = self.trtol {
            parts.push(format!("trtol={v}"));
        }
        if let Some(v) = self.chgtol {
            parts.push(format!("chgtol={v}"));
        }
        if let Some(v) = self.xmu {
            parts.push(format!("xmu={v}"));
        }
        if let Some(v) = self.itl3 {
            parts.push(format!("itl3={v}"));
        }
        if let Some(v) = self.itl4 {
            parts.push(format!("itl4={v}"));
        }
        if let Some(v) = self.itl5 {
            parts.push(format!("itl5={v}"));
        }
        if let Some(v) = self.ramptime {
            parts.push(format!("ramptime={v}"));
        }
        if Some(true) == self.autostop {
            parts.push("autostop".to_string());
        }
        if Some(true) == self.interp {
            parts.push("interp".to_string());
        }
        parts.join(" ")
    }
}

/// Fourier post-processing spec attached to a transient analysis.
///
/// Emits a `fourier` command after the `tran` run to compute DC component and
/// the first 9 harmonics of the specified output probes at the given fundamental frequency.
#[derive(Debug, Clone)]
pub struct FourierSpec {
    pub freq: f64,
    pub outputs: Vec<Probe>,
}

#[derive(Debug, Clone)]
pub struct TranAnalysis {
    pub tstep: f64,
    pub tstop: f64,
    pub tstart: Option<f64>,
    pub tmax: Option<f64>,
    pub uic: bool,
    pub solver: SolverOptions,
    pub tran_options: TranOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
    /// Optional Fourier post-processing directive.
    pub fourier: Option<FourierSpec>,
}

impl TranAnalysis {
    pub fn new(tstep: f64, tstop: f64) -> Self {
        Self {
            tstep,
            tstop,
            tstart: None,
            tmax: None,
            uic: false,
            solver: SolverOptions::default(),
            tran_options: TranOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
            fourier: None,
        }
    }

    pub fn tstart(mut self, v: f64) -> Self {
        self.tstart = Some(v);
        self
    }
    pub fn tmax(mut self, v: f64) -> Self {
        self.tmax = Some(v);
        self
    }
    pub fn uic(mut self) -> Self {
        self.uic = true;
        self
    }
    pub fn method(mut self, m: IntegrationMethod) -> Self {
        self.tran_options.method = Some(m);
        self
    }
    pub fn maxord(mut self, v: u32) -> Self {
        self.tran_options.maxord = Some(v);
        self
    }
    pub fn trtol(mut self, v: f64) -> Self {
        self.tran_options.trtol = Some(v);
        self
    }
    pub fn chgtol(mut self, v: f64) -> Self {
        self.tran_options.chgtol = Some(v);
        self
    }
    pub fn ramptime(mut self, v: f64) -> Self {
        self.tran_options.ramptime = Some(v);
        self
    }
    pub fn autostop(mut self) -> Self {
        self.tran_options.autostop = Some(true);
        self
    }
    pub fn interp(mut self) -> Self {
        self.tran_options.interp = Some(true);
        self
    }

    /// Attach Fourier post-processing at the given fundamental frequency.
    pub fn fourier(mut self, freq: f64, outputs: Vec<Probe>) -> Self {
        self.fourier = Some(FourierSpec { freq, outputs });
        self
    }
}

crate::impl_solver_options!(TranAnalysis);
crate::impl_analysis_common!(TranAnalysis);

impl SpiceAnalysis for TranAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let extra = self.tran_options.to_options_string();
        emit_common(&self.solver, &extra, &self.nodesets, &mut lines);

        let mut cmd = format!("tran {} {}", self.tstep, self.tstop);
        if let Some(ts) = self.tstart {
            cmd.push_str(&format!(" {ts}"));
        }
        if let Some(tm) = self.tmax {
            cmd.push_str(&format!(" {tm}"));
        }
        if self.uic {
            cmd.push_str(" UIC");
        }
        lines.push(cmd);
        emit_meas(&self.measurements, "tran", &mut lines);
        if let Some(ref f) = self.fourier {
            let outputs: Vec<String> = f.outputs.iter().map(|p| p.to_spice_save()).collect();
            lines.push(format!("fourier {} {}", f.freq, outputs.join(" ")));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;

    #[test]
    fn tran_no_fourier() {
        let cmds = TranAnalysis::new(1e-6, 1e-3).to_spice_control_commands();
        assert_eq!(cmds, vec!["tran 0.000001 0.001"]);
    }

    #[test]
    fn tran_fourier() {
        let out = Node::from("out");
        let cmds = TranAnalysis::new(1e-6, 1e-3)
            .fourier(1000.0, vec![Probe::voltage(out)])
            .to_spice_control_commands();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], "tran 0.000001 0.001");
        assert!(cmds[1].starts_with("fourier 1000 "), "unexpected: {:?}", cmds[1]);
    }
}
