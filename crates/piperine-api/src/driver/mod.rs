//! The policy-parameterized optimization/centering driver (dv-gradients
//! T16, DVG-14): ONE driver type with three policies — optimize, center,
//! high-sigma — over the restamp loop, so the three policies never drift
//! into three separate engines that duplicate the same margin-reading,
//! restamp-writing plumbing (design §C4).
//!
//! # The `collect`-posture invariant (T16 AC2)
//!
//! [`Driver::run`] ALWAYS solves in [`CheckPosture::Collect`], regardless
//! of the `checks` field on the [`SolverConfig`] the caller passes in. An
//! optimizer that aborted on its own infeasible iterates (the `Strict`
//! behavior) could not search — climbing out of an infeasible starting
//! point, or a black-box engine's exploratory evaluations, routinely visit
//! infeasible points on the way to a feasible optimum. `Driver::run`
//! overrides `checks` unconditionally before it ever calls into a policy;
//! every other field on the passed-in config (tolerances, `require_scope`,
//! convergence tunables) travels through unchanged.
//!
//! # Policies
//!
//! - [`Policy::Optimize`] (T17/T18, DVG-14/15): feasibility phase (climb
//!   until every `require` margin ≥ 0), then objective phase (descend the
//!   objective while projecting onto the feasible set) — analytic gradient
//!   ([`crate::Session::gradient`]) where the objective is differentiable,
//!   a derivative-free engine composed in when it is not.
//! - [`Policy::Center`] (T19, DVG-19): maximize `min_i (m_i / σ_i)`.
//! - [`Policy::HighSigma`]: importance-sampling tail estimation (T20,
//!   DVG-20) — out of this delivery's scope. The variant exists so the
//!   driver's three-policy shape is complete (T16 AC1); running it fails
//!   loud naming the gap rather than silently doing nothing.

mod center;
mod optimize;

pub use center::{CenterResult, CenterSpec, SigmaScheme};
pub use optimize::{Engine, OptimizeResult, OptimizeSpec};

use crate::error::Error;
use crate::session::{Session, SolverConfig};
use piperine_solver::prelude::CheckPosture;

/// One of the driver's three policies (design §C4).
#[derive(Debug, Clone)]
pub enum Policy {
    /// Feasibility then objective, on gradients where available (T17/T18).
    Optimize(OptimizeSpec),
    /// Worst-case-distance centering (T19).
    Center(CenterSpec),
    /// Importance-sampling tail estimation (T20) — not implemented in
    /// this delivery; running it fails loud.
    HighSigma,
}

/// One [`Driver::run`] call's typed result — which policy ran decides
/// which variant comes back.
#[derive(Debug, Clone)]
pub enum DriverOutcome {
    Optimize(OptimizeResult),
    Center(CenterResult),
}

/// The one driver type (T16, DVG-14): built from a [`Policy`], shared by
/// both hosts (MD-22) because it lives in `piperine-api`.
#[derive(Debug, Clone)]
pub struct Driver {
    policy: Policy,
}

impl Driver {
    /// Build a driver for `policy`.
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }

    /// The posture this driver always solves in — [`CheckPosture::Collect`],
    /// by construction (T16 AC2), independent of any [`SolverConfig`] it is
    /// run with.
    pub fn posture(&self) -> CheckPosture {
        CheckPosture::Collect
    }

    /// Run the configured policy over `session`. Forces `collect` posture
    /// (T16 AC2) on a clone of `config` before dispatching — every other
    /// field of `config` (tolerances, `require_scope`, convergence
    /// tunables) reaches the policy unchanged.
    pub fn run(&self, session: &mut Session, config: &SolverConfig) -> Result<DriverOutcome, Error> {
        let mut collect_config = config.clone();
        collect_config.checks = CheckPosture::Collect;
        match &self.policy {
            Policy::Optimize(spec) => {
                optimize::run(session, spec, &collect_config).map(DriverOutcome::Optimize)
            }
            Policy::Center(spec) => {
                center::run(session, spec, &collect_config).map(DriverOutcome::Center)
            }
            Policy::HighSigma => Err(Error::Measurement(
                "the high-sigma policy is not implemented in this delivery (dv-gradients T20) — \
                 optimize and center are available"
                    .to_string(),
            )),
        }
    }
}

/// Read a metric's scalar value at the current operating point: a
/// declared margin name first (as [`crate::Session::gradient`] resolves
/// it), then a solved analog net name. Shared by every policy that needs
/// a raw (non-differentiated) objective/constraint readback — the
/// black-box engine (T18) and center's normalized-margin scoring (T19).
pub(crate) fn read_metric(op: &crate::results::OpResult, metric: &str) -> Result<f64, Error> {
    if let Ok(entry) = op.margins().get(metric) {
        return Ok(entry.scalar());
    }
    op.v(metric)
}

/// Resolve `label.param`'s AUTHORED (declared) default value by walking
/// the authored instance tree from `top` (MD-25 — never `flat_module`).
/// Used to seed a policy's starting point before any `Session::set` call
/// has moved it. Shared by every policy that needs a design-parameter
/// starting point.
pub(crate) fn nominal_param_value(
    design: &piperine_lang::pom::Design,
    top: &str,
    label: &str,
    param: &str,
) -> Result<f64, Error> {
    let top_module = design.module(top).ok_or_else(|| {
        Error::Measurement(format!("driver: top module `{top}` not found in the authored POM"))
    })?;
    find_param_default(design, top_module, "", label, param).ok_or_else(|| {
        Error::Measurement(format!(
            "driver: instance `{label}` param `{param}` not found on the authored instance tree"
        ))
    })
}

fn find_param_default(
    design: &piperine_lang::pom::Design,
    module: &piperine_lang::pom::Module,
    parent_path: &str,
    target_label: &str,
    param: &str,
) -> Option<f64> {
    use piperine_lang::value::Value;
    for instance in module.instances() {
        let path = if parent_path.is_empty() {
            instance.name().to_string()
        } else {
            format!("{parent_path}.{}", instance.name())
        };
        let Some(child) = design.module(instance.module_name()) else { continue };
        if path == target_label {
            return child.params().iter().find(|p| p.name() == param).and_then(|p| p.default()).and_then(Value::as_real);
        }
        if let Some(v) = find_param_default(design, child, &path, target_label, param) {
            return Some(v);
        }
    }
    None
}
