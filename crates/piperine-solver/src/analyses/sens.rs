//! DC sensitivity analysis (`.sens`) — options, result types, and the
//! driver: DC sensitivities by central finite difference over the restamp
//! path. For each `(label, param)`, perturb the parameter `±dp`, re-solve
//! the operating point (same compiled circuit, symbolic LU reused for the
//! run), and difference the requested outputs.
//!
//! SPEC_DEVIATION: the design sketched a single-linear-solve direct method
//! (`A·dx = −∂R/∂p` on the converged Jacobian).
//! Reason: the assembled system is not exposed outside the Newton loop
//! today; central-difference re-solves are the robust baseline behind the
//! same API, and the exact/direct method remains the documented upgrade.

use std::collections::HashMap;

use crate::core::circuit::CircuitInstance;
use crate::core::introspect::{Invalidation, Value};
use crate::core::net::Net;
use crate::error::{Error, SolverDomain};
use crate::analyses::dc::DcSolver;
use crate::analyses::Policy;
use crate::Context;

// ── request/state ────────────────────────────────────────────────────────

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

// ── driver ───────────────────────────────────────────────────────────────

pub struct SensSolver<'a> {
    circuit: &'a mut CircuitInstance,
    options: SensAnalysisOptions,
    context: Context,
    /// Convergence tunables applied to every inner operating point (MD-04).
    pub policy: Policy,
}

impl<'a> SensSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: SensAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        Ok(Self { circuit, options, context, policy: Policy::default() })
    }

    /// Validate one `(label, param)` pair and return its current value.
    /// Loud on: unknown element, unknown/non-real parameter, and parameters
    /// whose write would invalidate the compiled structure
    /// ([`Invalidation::Rebuild`]) — a finite difference across a rebuild
    /// boundary is not a sensitivity.
    fn param_value(&self, label: &str, param: &str) -> crate::result::Result<f64> {
        let device =
            self.circuit.devices.iter().find(|d| d.name() == label).ok_or_else(|| {
                Error::simple(SolverDomain::Sens, format!("no element labeled `{label}`"))
            })?;
        let desc =
            device.list_params().into_iter().find(|d| d.name == param).ok_or_else(|| {
                let names: Vec<String> =
                    device.list_params().into_iter().map(|p| p.name).collect();
                Error::simple(
                    SolverDomain::Sens,
                    format!(
                        "`{label}` declares no parameter `{param}`; available: {}",
                        if names.is_empty() { "(none)".to_string() } else { names.join(", ") }
                    ),
                )
            })?;
        if desc.invalidation == Invalidation::Rebuild {
            return Err(Error::simple(
                SolverDomain::Sens,
                format!(
                    "`{label}.{param}` invalidates the compiled circuit (Rebuild) — \
                     sensitivities are only defined for restampable parameters"
                ),
            ));
        }
        match device.get_param(param) {
            Some(Value::Real(v)) => Ok(v),
            Some(other) => Err(Error::simple(
                SolverDomain::Sens,
                format!("`{label}.{param}` is not a real parameter (got {other:?})"),
            )),
            None => Err(Error::simple(
                SolverDomain::Sens,
                format!("`{label}.{param}` is not readable"),
            )),
        }
    }

    fn solve_op(&mut self) -> crate::result::Result<crate::result::DcAnalysisResult> {
        let mut dc = DcSolver::new(self.circuit, self.context.clone())?;
        dc.policy = self.policy.clone();
        dc.solve()
    }

    pub fn solve(mut self) -> crate::result::Result<SensResult> {
        // Validate everything up front — no partial writes on a bad request.
        let mut currents = Vec::with_capacity(self.options.params.len());
        for (label, param) in &self.options.params {
            currents.push(self.param_value(label, param)?);
        }
        for out in &self.options.outputs {
            if out.analog_variable().is_none() {
                return Err(Error::simple(
                    SolverDomain::Sens,
                    format!("output `{}` is not a solved analog net", out.label()),
                ));
            }
        }

        let mut result = SensResult { d: std::collections::HashMap::new() };
        let params = self.options.params.clone();
        let outputs = self.options.outputs.clone();
        for ((label, param), p0) in params.iter().zip(currents) {
            let dp = if p0 == 0.0 { self.options.dp_rel } else { self.options.dp_rel * p0.abs() };

            self.circuit.set_element_param(label, param, Value::Real(p0 + dp))?;
            let plus = self.solve_op()?;
            self.circuit.set_element_param(label, param, Value::Real(p0 - dp))?;
            let minus = self.solve_op()?;
            self.circuit.set_element_param(label, param, Value::Real(p0))?;

            for out in &outputs {
                let a = plus.get_net(out).ok_or_else(|| {
                    Error::simple(
                        SolverDomain::Sens,
                        format!("output `{}` is not in the solved result", out.label()),
                    )
                })?;
                let b = minus.get_net(out).unwrap_or(a);
                result
                    .d
                    .insert((out.label().to_string(), format!("{label}.{param}")), (a - b) / (2.0 * dp));
            }
        }
        Ok(result)
    }
}
