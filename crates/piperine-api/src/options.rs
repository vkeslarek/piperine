/// Solver options common to all analysis types.
#[derive(Debug, Clone, Default)]
pub struct SolverOptions {
    pub abstol: Option<f64>,
    pub reltol: Option<f64>,
    pub vntol: Option<f64>,
    pub gmin: Option<f64>,
    pub itl1: Option<u32>,
    pub itl2: Option<u32>,
    pub pivrel: Option<f64>,
    pub pivtol: Option<f64>,
    pub rshunt: Option<f64>,
    pub temp: Option<f64>,
    pub tnom: Option<f64>,
}

impl SolverOptions {
    /// Relaxed convergence settings.
    pub fn relaxed() -> Self {
        Self {
            reltol: Some(0.01),
            abstol: Some(1e-10),
            vntol: Some(1e-4),
            gmin: Some(1e-10),
            ..Default::default()
        }
    }

    /// Tight convergence settings.
    pub fn tight() -> Self {
        Self {
            reltol: Some(1e-6),
            abstol: Some(1e-14),
            vntol: Some(1e-8),
            ..Default::default()
        }
    }

    pub fn to_options_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = self.abstol { parts.push(format!("abstol={v}")); }
        if let Some(v) = self.reltol { parts.push(format!("reltol={v}")); }
        if let Some(v) = self.vntol { parts.push(format!("vntol={v}")); }
        if let Some(v) = self.gmin { parts.push(format!("gmin={v}")); }
        if let Some(v) = self.itl1 { parts.push(format!("itl1={v}")); }
        if let Some(v) = self.itl2 { parts.push(format!("itl2={v}")); }
        if let Some(v) = self.pivrel { parts.push(format!("pivrel={v}")); }
        if let Some(v) = self.pivtol { parts.push(format!("pivtol={v}")); }
        if let Some(v) = self.rshunt { parts.push(format!("rshunt={v}")); }
        if let Some(v) = self.temp { parts.push(format!("temp={v}")); }
        if let Some(v) = self.tnom { parts.push(format!("tnom={v}")); }
        parts.join(" ")
    }
}

/// Transient-analysis-specific options.
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
    pub fn to_options_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref m) = self.method { parts.push(format!("method={}", m.to_spice())); }
        if let Some(v) = self.maxord { parts.push(format!("maxord={v}")); }
        if let Some(v) = self.trtol { parts.push(format!("trtol={v}")); }
        if let Some(v) = self.chgtol { parts.push(format!("chgtol={v}")); }
        if let Some(v) = self.xmu { parts.push(format!("xmu={v}")); }
        if let Some(v) = self.itl3 { parts.push(format!("itl3={v}")); }
        if let Some(v) = self.itl4 { parts.push(format!("itl4={v}")); }
        if let Some(v) = self.itl5 { parts.push(format!("itl5={v}")); }
        if let Some(v) = self.ramptime { parts.push(format!("ramptime={v}")); }
        if Some(true) == self.autostop { parts.push("autostop".to_string()); }
        if Some(true) == self.interp { parts.push("interp".to_string()); }
        parts.join(" ")
    }
}

/// AC-analysis-specific options.
#[derive(Debug, Clone, Default)]
pub struct AcOptions {
    pub noopac: Option<bool>,
}

impl AcOptions {
    pub fn to_options_string(&self) -> String {
        if Some(true) == self.noopac {
            "noopac".to_string()
        } else {
            String::new()
        }
    }
}

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

/// AC frequency sweep variation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variation {
    Dec,
    Oct,
    Lin,
}

impl Variation {
    pub fn to_spice(&self) -> &'static str {
        match self {
            Variation::Dec => "dec",
            Variation::Oct => "oct",
            Variation::Lin => "lin",
        }
    }
}

/// Circuit-level options (physical properties, emitted as `.options` in netlist).
#[derive(Debug, Clone, Default)]
pub struct CircuitOptions {
    pub temp: Option<f64>,
    pub tnom: Option<f64>,
    pub scale: Option<f64>,
    pub savecurrents: bool,
}

impl CircuitOptions {
    pub fn to_options_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = self.temp { parts.push(format!("temp={v}")); }
        if let Some(v) = self.tnom { parts.push(format!("tnom={v}")); }
        if let Some(v) = self.scale { parts.push(format!("scale={v}")); }
        if self.savecurrents { parts.push("savecurrents".to_string()); }
        parts.join(" ")
    }
}

/// Macro to implement common solver option builder methods on analysis types.
#[macro_export]
macro_rules! impl_solver_options {
    ($ty:ty) => {
        impl $ty {
            pub fn reltol(mut self, v: f64) -> Self { self.solver.reltol = Some(v); self }
            pub fn abstol(mut self, v: f64) -> Self { self.solver.abstol = Some(v); self }
            pub fn vntol(mut self, v: f64) -> Self { self.solver.vntol = Some(v); self }
            pub fn gmin(mut self, v: f64) -> Self { self.solver.gmin = Some(v); self }
            pub fn itl1(mut self, v: u32) -> Self { self.solver.itl1 = Some(v); self }
            pub fn itl2(mut self, v: u32) -> Self { self.solver.itl2 = Some(v); self }
            pub fn solver_temp(mut self, v: f64) -> Self { self.solver.temp = Some(v); self }
            pub fn solver_tnom(mut self, v: f64) -> Self { self.solver.tnom = Some(v); self }
            pub fn with_solver(mut self, opts: $crate::options::SolverOptions) -> Self {
                self.solver = opts; self
            }
        }
    };
}

/// Macro to implement common analysis builder methods (save, nodeset).
#[macro_export]
macro_rules! impl_analysis_common {
    ($ty:ty) => {
        impl $ty {
            pub fn save(mut self, probe: $crate::spice::Probe) -> Self {
                self.saves.push(probe); self
            }
            pub fn nodeset(mut self, node: $crate::node::Node, voltage: f64) -> Self {
                self.nodesets.push((node, voltage)); self
            }
        }
    };
}
