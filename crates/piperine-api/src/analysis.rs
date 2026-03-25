use crate::node::Node;
use crate::options::*;
use crate::spice::{ElementRef, SpiceAnalysis, Measurement};

// Helper: emit common control lines (options, nodesets).
fn emit_common(
    solver: &SolverOptions,
    extra_options: &str,
    nodesets: &[(Node, f64)],
    lines: &mut Vec<String>,
) {
    let solver_opts = solver.to_options_string();
    let all_opts = if extra_options.is_empty() {
        solver_opts
    } else if solver_opts.is_empty() {
        extra_options.to_string()
    } else {
        format!("{solver_opts} {extra_options}")
    };
    if !all_opts.is_empty() {
        lines.push(format!(".options {all_opts}"));
    }

    for (node, voltage) in nodesets {
        lines.push(format!(".nodeset V({})={}", node, voltage));
    }
}

// Helper: emit meas commands.
fn emit_meas(measurements: &[Measurement], analysis_type: &str, lines: &mut Vec<String>) {
    for m in measurements {
        lines.push(m.to_meas_cmd(analysis_type));
    }
}

// ===== Operating Point =====

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

// ===== DC Sweep =====

#[derive(Debug, Clone)]
pub struct DcAnalysis {
    pub source: ElementRef,
    pub start: f64,
    pub stop: f64,
    pub step: f64,
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
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(DcAnalysis);
crate::impl_analysis_common!(DcAnalysis);

impl SpiceAnalysis for DcAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        lines.push(format!(
            "dc {} {} {} {}",
            self.source.spice_name(),
            self.start,
            self.stop,
            self.step
        ));
        emit_meas(&self.measurements, "dc", &mut lines);
        lines
    }
}

// ===== AC Analysis =====

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

// ===== Transient Analysis =====

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
        lines
    }
}

// ===== Noise Analysis =====

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

// ===== Transfer Function =====

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

// ===== Sensitivity Analysis =====

#[derive(Debug, Clone)]
pub struct SensAnalysis {
    pub output: Node,
    pub ac_variation: Option<(Variation, u32, f64, f64)>,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl SensAnalysis {
    /// DC sensitivity.
    pub fn dc(output: Node) -> Self {
        Self {
            output,
            ac_variation: None,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// AC sensitivity.
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
