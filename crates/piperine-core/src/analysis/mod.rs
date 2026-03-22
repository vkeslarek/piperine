use crate::options::*;

/// Trait for types that produce SPICE control commands.
///
/// Control commands include: analysis commands (op, dc, ac, tran...),
/// .options for solver settings, .save, .meas, .nodeset.
pub trait ToControl {
    fn to_control_commands(&self) -> Vec<String>;
}

/// A .meas statement.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub expr: String,
}

// Helper: emit common control lines (options, nodesets, saves, measurements).
fn emit_common(
    solver: &SolverOptions,
    extra_options: &str,
    nodesets: &[(String, f64)],
    saves: &[String],
    measurements: &[String],
    _analysis_type: &str,
    lines: &mut Vec<String>,
) {
    // .options
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

    // .nodeset
    for (node, voltage) in nodesets {
        lines.push(format!(".nodeset V({node})={voltage}"));
    }

    // save
    if !saves.is_empty() {
        lines.push(format!("save {}", saves.join(" ")));
    }

    // .meas
    for m in measurements {
        lines.push(m.clone());
    }
}

// === Operating Point ===

#[derive(Debug, Clone, Default)]
pub struct OpAnalysis {
    pub solver: SolverOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl OpAnalysis {
    pub fn new() -> Self { Self::default() }
}

crate::impl_solver_options!(OpAnalysis);
crate::impl_analysis_common!(OpAnalysis);

impl ToControl for OpAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas dc {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, "", &self.nodesets, &self.saves, &meas, "dc", &mut lines);
        lines.push("op".to_string());
        lines
    }
}

// === DC Sweep ===

#[derive(Debug, Clone)]
pub struct DcAnalysis {
    pub source: String,
    pub start: f64,
    pub stop: f64,
    pub step: f64,
    pub solver: SolverOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl DcAnalysis {
    pub fn new(source: &str, start: f64, stop: f64, step: f64) -> Self {
        Self {
            source: source.to_string(), start, stop, step,
            solver: SolverOptions::default(),
            saves: Vec::new(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(DcAnalysis);
crate::impl_analysis_common!(DcAnalysis);

impl ToControl for DcAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas dc {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, "", &self.nodesets, &self.saves, &meas, "dc", &mut lines);
        lines.push(format!("dc {} {} {} {}", self.source, self.start, self.stop, self.step));
        lines
    }
}

// === AC Analysis ===

#[derive(Debug, Clone)]
pub struct AcAnalysis {
    pub variation: Variation,
    pub npoints: u32,
    pub fstart: f64,
    pub fstop: f64,
    pub solver: SolverOptions,
    pub ac_options: AcOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl AcAnalysis {
    pub fn new(variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            variation, npoints, fstart, fstop,
            solver: SolverOptions::default(),
            ac_options: AcOptions::default(),
            saves: Vec::new(),
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

impl ToControl for AcAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let extra = self.ac_options.to_options_string();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas ac {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, &extra, &self.nodesets, &self.saves, &meas, "ac", &mut lines);
        lines.push(format!("ac {} {} {} {}",
            self.variation.to_spice(), self.npoints, self.fstart, self.fstop));
        lines
    }
}

// === Transient Analysis ===

#[derive(Debug, Clone)]
pub struct TranAnalysis {
    pub tstep: f64,
    pub tstop: f64,
    pub tstart: Option<f64>,
    pub tmax: Option<f64>,
    pub uic: bool,
    pub solver: SolverOptions,
    pub tran_options: TranOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl TranAnalysis {
    pub fn new(tstep: f64, tstop: f64) -> Self {
        Self {
            tstep, tstop, tstart: None, tmax: None, uic: false,
            solver: SolverOptions::default(),
            tran_options: TranOptions::default(),
            saves: Vec::new(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    pub fn tstart(mut self, v: f64) -> Self { self.tstart = Some(v); self }
    pub fn tmax(mut self, v: f64) -> Self { self.tmax = Some(v); self }
    pub fn uic(mut self) -> Self { self.uic = true; self }
    pub fn method(mut self, m: IntegrationMethod) -> Self { self.tran_options.method = Some(m); self }
    pub fn maxord(mut self, v: u32) -> Self { self.tran_options.maxord = Some(v); self }
    pub fn trtol(mut self, v: f64) -> Self { self.tran_options.trtol = Some(v); self }
    pub fn chgtol(mut self, v: f64) -> Self { self.tran_options.chgtol = Some(v); self }
    pub fn ramptime(mut self, v: f64) -> Self { self.tran_options.ramptime = Some(v); self }
    pub fn autostop(mut self) -> Self { self.tran_options.autostop = Some(true); self }
    pub fn interp(mut self) -> Self { self.tran_options.interp = Some(true); self }

    pub fn with_tran_options(mut self, opts: TranOptions) -> Self {
        self.tran_options = opts; self
    }
}

crate::impl_solver_options!(TranAnalysis);
crate::impl_analysis_common!(TranAnalysis);

impl ToControl for TranAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let extra = self.tran_options.to_options_string();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas tran {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, &extra, &self.nodesets, &self.saves, &meas, "tran", &mut lines);

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
        lines
    }
}

// === Noise Analysis ===

#[derive(Debug, Clone)]
pub struct NoiseAnalysis {
    pub output: String,
    pub src: String,
    pub variation: Variation,
    pub npoints: u32,
    pub fstart: f64,
    pub fstop: f64,
    pub pts_per_summary: Option<u32>,
    pub solver: SolverOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl NoiseAnalysis {
    pub fn new(output: &str, src: &str, variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            output: output.to_string(), src: src.to_string(),
            variation, npoints, fstart, fstop,
            pts_per_summary: None,
            solver: SolverOptions::default(),
            saves: Vec::new(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    pub fn pts_per_summary(mut self, n: u32) -> Self { self.pts_per_summary = Some(n); self }
}

crate::impl_solver_options!(NoiseAnalysis);
crate::impl_analysis_common!(NoiseAnalysis);

impl ToControl for NoiseAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas noise {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, "", &self.nodesets, &self.saves, &meas, "noise", &mut lines);

        let mut cmd = format!("noise {} {} {} {} {} {}",
            self.output, self.src, self.variation.to_spice(),
            self.npoints, self.fstart, self.fstop);
        if let Some(n) = self.pts_per_summary {
            cmd.push_str(&format!(" {n}"));
        }
        lines.push(cmd);
        lines
    }
}

// === Transfer Function ===

#[derive(Debug, Clone)]
pub struct TfAnalysis {
    pub output: String,
    pub input_source: String,
    pub solver: SolverOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl TfAnalysis {
    pub fn new(output: &str, input_source: &str) -> Self {
        Self {
            output: output.to_string(),
            input_source: input_source.to_string(),
            solver: SolverOptions::default(),
            saves: Vec::new(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(TfAnalysis);
crate::impl_analysis_common!(TfAnalysis);

impl ToControl for TfAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas dc {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, "", &self.nodesets, &self.saves, &meas, "dc", &mut lines);
        lines.push(format!("tf {} {}", self.output, self.input_source));
        lines
    }
}

// === Sensitivity Analysis ===

#[derive(Debug, Clone)]
pub struct SensAnalysis {
    pub output: String,
    pub ac_variation: Option<(Variation, u32, f64, f64)>,
    pub solver: SolverOptions,
    pub saves: Vec<String>,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(String, f64)>,
}

impl SensAnalysis {
    /// DC sensitivity.
    pub fn dc(output: &str) -> Self {
        Self {
            output: output.to_string(),
            ac_variation: None,
            solver: SolverOptions::default(),
            saves: Vec::new(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }

    /// AC sensitivity.
    pub fn ac(output: &str, variation: Variation, npoints: u32, fstart: f64, fstop: f64) -> Self {
        Self {
            output: output.to_string(),
            ac_variation: Some((variation, npoints, fstart, fstop)),
            solver: SolverOptions::default(),
            saves: Vec::new(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(SensAnalysis);
crate::impl_analysis_common!(SensAnalysis);

impl ToControl for SensAnalysis {
    fn to_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let meas: Vec<String> = self.measurements.iter()
            .map(|m| format!(".meas dc {} {}", m.name, m.expr))
            .collect();
        emit_common(&self.solver, "", &self.nodesets, &self.saves, &meas, "dc", &mut lines);
        if let Some((var, np, fs, fe)) = &self.ac_variation {
            lines.push(format!("sens {} ac {} {} {} {}", self.output, var.to_spice(), np, fs, fe));
        } else {
            lines.push(format!("sens {}", self.output));
        }
        lines
    }
}
