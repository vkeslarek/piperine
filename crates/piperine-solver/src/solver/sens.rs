//! `.sens` driver — DC sensitivities by central finite difference over the
//! restamp path: for each `(label, param)`, perturb the parameter
//! `±dp`, re-solve the operating point (same compiled circuit, symbolic LU
//! reused for the run), and difference the requested outputs.
//!
//! SPEC_DEVIATION: the design sketched a single-linear-solve direct method
//! (`A·dx = −∂R/∂p` on the converged Jacobian).
//! Reason: the assembled system is not exposed outside the Newton loop
//! today; central-difference re-solves are the robust baseline behind the
//! same API, and the exact/direct method remains the documented upgrade.

use crate::core::circuit::CircuitInstance;
use crate::core::introspect::{Invalidation, Value};
use crate::error::{Error, SolverDomain};
use crate::solver::dc::DcSolver;
use crate::solver::Policy;
use crate::analysis::sens::{SensAnalysisOptions, SensResult};
use crate::Context;

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
