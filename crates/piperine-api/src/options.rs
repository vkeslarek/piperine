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
        if let Some(v) = self.abstol {
            parts.push(format!("abstol={v}"));
        }
        if let Some(v) = self.reltol {
            parts.push(format!("reltol={v}"));
        }
        if let Some(v) = self.vntol {
            parts.push(format!("vntol={v}"));
        }
        if let Some(v) = self.gmin {
            parts.push(format!("gmin={v}"));
        }
        if let Some(v) = self.itl1 {
            parts.push(format!("itl1={v}"));
        }
        if let Some(v) = self.itl2 {
            parts.push(format!("itl2={v}"));
        }
        if let Some(v) = self.pivrel {
            parts.push(format!("pivrel={v}"));
        }
        if let Some(v) = self.pivtol {
            parts.push(format!("pivtol={v}"));
        }
        if let Some(v) = self.rshunt {
            parts.push(format!("rshunt={v}"));
        }
        if let Some(v) = self.temp {
            parts.push(format!("temp={v}"));
        }
        if let Some(v) = self.tnom {
            parts.push(format!("tnom={v}"));
        }
        parts.join(" ")
    }
}

/// AC frequency sweep variation type. Used by AC, Noise, Sensitivity, SP, and Distortion analyses.
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
        if let Some(v) = self.temp {
            parts.push(format!("temp={v}"));
        }
        if let Some(v) = self.tnom {
            parts.push(format!("tnom={v}"));
        }
        if let Some(v) = self.scale {
            parts.push(format!("scale={v}"));
        }
        if self.savecurrents {
            parts.push("savecurrents".to_string());
        }
        parts.join(" ")
    }
}

/// Macro to implement common solver option builder methods on analysis types.
#[macro_export]
macro_rules! impl_solver_options {
    ($ty:ty) => {
        impl $ty {
            pub fn reltol(mut self, v: f64) -> Self {
                self.solver.reltol = Some(v);
                self
            }
            pub fn abstol(mut self, v: f64) -> Self {
                self.solver.abstol = Some(v);
                self
            }
            pub fn vntol(mut self, v: f64) -> Self {
                self.solver.vntol = Some(v);
                self
            }
            pub fn gmin(mut self, v: f64) -> Self {
                self.solver.gmin = Some(v);
                self
            }
            pub fn itl1(mut self, v: u32) -> Self {
                self.solver.itl1 = Some(v);
                self
            }
            pub fn itl2(mut self, v: u32) -> Self {
                self.solver.itl2 = Some(v);
                self
            }
            pub fn solver_temp(mut self, v: f64) -> Self {
                self.solver.temp = Some(v);
                self
            }
            pub fn solver_tnom(mut self, v: f64) -> Self {
                self.solver.tnom = Some(v);
                self
            }
            pub fn with_solver(mut self, opts: $crate::options::SolverOptions) -> Self {
                self.solver = opts;
                self
            }
        }
    };
}

/// Macro to implement common analysis builder methods (meas, nodeset).
#[macro_export]
macro_rules! impl_analysis_common {
    ($ty:ty) => {
        impl $ty {
            pub fn meas(mut self, m: $crate::spice::Measurement) -> Self {
                self.measurements.push(m);
                self
            }
            pub fn nodeset(mut self, node: $crate::node::Node, voltage: f64) -> Self {
                self.nodesets.push((node, voltage));
                self
            }
        }
    };
}
