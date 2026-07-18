//! DC sensitivity analysis (`.sens`) — options and result types. The driver
//! lives in [`crate::solver::sens`].

use std::collections::HashMap;

use crate::core::net::Net;

/// What to differentiate and with respect to what. `outputs` are solved
/// analog nets (node voltages / branch currents); `params` are
/// `(element label, parameter name)` pairs addressed exactly like
/// [`CircuitInstance::set_element_param`](crate::core::circuit::CircuitInstance::set_element_param).
#[derive(Debug, Clone)]
pub struct SensAnalysisOptions {
    pub outputs: Vec<Net>,
    pub params: Vec<(String, String)>,
    /// Relative finite-difference step (absolute fallback when the
    /// parameter value is 0). Default `1e-6`.
    pub dp_rel: f64,
}

impl SensAnalysisOptions {
    pub fn new(outputs: Vec<Net>, params: Vec<(String, String)>) -> Self {
        Self { outputs, params, dp_rel: 1e-6 }
    }
}

/// `∂(output)/∂(param)` at the DC operating point, keyed by
/// `(output label, "element.param")`.
#[derive(Debug, Clone)]
pub struct SensResult {
    pub d: HashMap<(String, String), f64>,
}

impl SensResult {
    /// The sensitivity of `output` w.r.t. `label.param`, if computed.
    pub fn get(&self, output: &str, label: &str, param: &str) -> Option<f64> {
        self.d.get(&(output.to_string(), format!("{label}.{param}"))).copied()
    }
}
