//! The center policy (dv-gradients T19, DVG-19): maximize
//! `min_i (m_i / σ_i)` — the worst-case distance to any constraint
//! boundary, in sigmas — over the design parameters.
//!
//! `σ_i` is estimated by an inner Monte Carlo run
//! ([`crate::SamplePlan::monte_carlo`], T15) and held FIXED within one
//! outer iteration, then re-sampled between outer iterations (design
//! §C4's "cheap scheme" — `σ_i` depends on the design point, so an outer
//! loop periodically refreshes it rather than paying for a fresh estimate
//! on every inner step). [`CenterResult::sigma_scheme`] names the scheme
//! so a reader never has to guess what the reported number means.

use std::collections::HashMap;

use crate::error::Error;
use crate::sampling::SamplePlan;
use crate::session::{Session, SolverConfig};

/// Which scheme produced the reported `σ_i` values (DVG-19 AC2) — the
/// number means different things under a different scheme, so the result
/// always states which one ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigmaScheme {
    /// `σ_i` held fixed within one outer iteration, re-sampled between
    /// outer iterations (design §C4) — the only scheme this delivery
    /// implements.
    FixedWithinOuterIteration,
}

/// The center policy's request (T19, DVG-19).
#[derive(Debug, Clone)]
pub struct CenterSpec {
    /// The declared `require` margins to center over (qualified or bare,
    /// as [`crate::MarginsResult::get`] resolves them).
    pub constraints: Vec<String>,
    /// The design parameters to vary.
    pub over: Vec<(String, String)>,
    /// Samples drawn per outer iteration to estimate `σ_i`.
    pub samples_per_outer: usize,
    /// The seed the inner Monte Carlo sampling is drawn from.
    pub seed: u64,
    /// Outer iterations (each re-samples `σ_i` and takes one step).
    pub outer_iters: usize,
    /// Inner gradient-ascent iterations per outer iteration, ascending
    /// `min_i (m_i / σ_i)` at the fixed `σ_i`.
    pub inner_iters: usize,
    /// The relative step fraction for a parameter update
    /// (`Δp = step · |p|`, floored so a zero-valued parameter still
    /// moves).
    pub step: f64,
}

impl Default for CenterSpec {
    fn default() -> Self {
        Self {
            constraints: Vec::new(),
            over: Vec::new(),
            samples_per_outer: 100,
            seed: 1,
            outer_iters: 5,
            inner_iters: 10,
            step: 0.05,
        }
    }
}

/// The center policy's result (T19, DVG-19).
#[derive(Debug, Clone)]
pub struct CenterResult {
    /// The centered design parameters, keyed `"label.param"`.
    pub params: HashMap<String, f64>,
    /// `min_i (m_i / σ_i)` at the centered point — the worst-case
    /// distance to any constraint boundary, in sigmas.
    pub worst_normalized_margin: f64,
    /// Which `σ_i` scheme produced the reported value (DVG-19 AC2).
    pub sigma_scheme: SigmaScheme,
    /// Outer iterations actually run.
    pub outer_iterations: usize,
}

/// Run the center policy: [`super::Driver::run`]'s dispatch target for
/// [`super::Policy::Center`]. `config` always carries `collect` posture
/// (forced by [`super::Driver::run`]).
pub(super) fn run(
    session: &mut Session,
    spec: &CenterSpec,
    config: &SolverConfig,
) -> Result<CenterResult, Error> {
    let top = session.module().to_string();
    let design = session.design().clone();

    let mut current: HashMap<(String, String), f64> = spec
        .over
        .iter()
        .map(|(label, param)| {
            super::nominal_param_value(&design, &top, label, param)
                .map(|v| ((label.clone(), param.clone()), v))
        })
        .collect::<Result<_, Error>>()?;
    for (label, param) in &spec.over {
        session.set(label, param, current[&(label.clone(), param.clone())])?;
    }

    let plan = SamplePlan::collect(&design, &top)?;
    let mut worst_normalized_margin = f64::NEG_INFINITY;
    let mut outer_run = 0;

    for outer in 0..spec.outer_iters.max(1) {
        outer_run = outer + 1;
        // Re-sample sigma at the top of every outer iteration (fixed
        // within it, refreshed between them — DVG-19 AC2).
        let mc = plan.monte_carlo(session, spec.samples_per_outer.max(2), spec.seed.wrapping_add(outer as u64), config)?;
        let sigmas: HashMap<String, f64> = spec
            .constraints
            .iter()
            .filter_map(|name| mc.sigma(name).map(|s| (name.clone(), s.max(1e-12))))
            .collect();

        for _ in 0..spec.inner_iters.max(1) {
            let op = session.op(config, None)?;
            let normalized: Vec<(String, f64)> = spec
                .constraints
                .iter()
                .filter_map(|name| {
                    let m = super::read_metric(&op, name).ok()?;
                    let sigma = *sigmas.get(name)?;
                    Some((name.clone(), m / sigma))
                })
                .collect();
            if normalized.is_empty() {
                break;
            }
            let (active_name, active_value) = normalized
                .iter()
                .min_by(|a, b| a.1.partial_cmp(&b.1).expect("finite normalized margins"))
                .cloned()
                .expect("non-empty");
            worst_normalized_margin = worst_normalized_margin.max(active_value);

            // Ascend the ACTIVE constraint's normalized margin — D1: the
            // `min` differentiates at the currently active member.
            let grad = super::optimize::finite_diff_metric(session, &spec.over, &current, &active_name, config)?;
            let sigma = *sigmas.get(&active_name).unwrap_or(&1.0);
            let norm = grad.iter().map(|g| g * g).sum::<f64>().sqrt();
            if norm < 1e-12 {
                break;
            }
            for ((label, param), g) in spec.over.iter().zip(&grad) {
                let base = current[&(label.clone(), param.clone())];
                let delta = spec.step * base.abs().max(1e-6) * (g / sigma / norm);
                let new_v = base + delta;
                current.insert((label.clone(), param.clone()), new_v);
                session.set(label, param, new_v)?;
            }
        }
    }

    let params = current
        .into_iter()
        .map(|((label, param), v)| (format!("{label}.{param}"), v))
        .collect();

    Ok(CenterResult {
        params,
        worst_normalized_margin,
        sigma_scheme: SigmaScheme::FixedWithinOuterIteration,
        outer_iterations: outer_run,
    })
}
