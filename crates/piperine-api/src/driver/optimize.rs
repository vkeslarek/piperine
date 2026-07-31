//! The optimize policy (dv-gradients T17, DVG-14): a feasibility phase
//! (climb until every `require` margin ≥ 0) followed by an objective
//! phase (descend the objective while projecting onto the feasible set),
//! using [`crate::Session::gradient`] (T11) where the objective is
//! differentiable and composing in a derivative-free engine (T18) where
//! it is not.

use std::collections::{HashMap, HashSet};

use piperine_solver::prelude::MarginKind;

use crate::error::Error;
use crate::session::{Session, SolverConfig};

/// Which engine produced a step or the final result (T17/T18, DVG-15):
/// [`Engine::Gradient`] — the analytic adjoint ([`crate::Session::gradient`],
/// T11) — or [`Engine::BlackBox`] — a derivative-free fallback (T18) for a
/// part of the search the compiler cannot differentiate. A run that used
/// both (feasibility on gradients, a non-differentiable objective on
/// black-box) reports both — never silently picks one to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    /// The analytic DC adjoint (one transpose solve for every parameter).
    Gradient,
    /// A derivative-free engine (T18) — used when the objective or a
    /// constraint cannot be differentiated through the solver.
    BlackBox,
}

impl Engine {
    /// The stable, host-visible name (Python and Rust agree — MD-22).
    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Gradient => "gradient",
            Engine::BlackBox => "black-box",
        }
    }
}

impl std::fmt::Display for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The optimize policy's request (T17, DVG-14).
#[derive(Debug, Clone)]
pub struct OptimizeSpec {
    /// The objective's name — a declared margin/`var` name or a solved
    /// net name, resolved the same way
    /// [`crate::Session::gradient`] resolves a metric.
    pub objective: String,
    /// `true` to minimize the objective (the default), `false` to
    /// maximize it.
    pub minimize: bool,
    /// The design parameters to vary, `(label, param)` pairs.
    pub over: Vec<(String, String)>,
    /// The iteration cap for EACH phase (feasibility, then objective) —
    /// also the line search's outer bound, so the whole run always
    /// terminates regardless of a gradient discontinuity (D1).
    pub max_iter: usize,
    /// The relative step fraction for a parameter update
    /// (`Δp = step · |p|`, floored so a zero-valued parameter still
    /// moves) — halved by the objective phase's backtracking line search
    /// on a rejected trial.
    pub step: f64,
}

impl Default for OptimizeSpec {
    fn default() -> Self {
        Self { objective: String::new(), minimize: true, over: Vec::new(), max_iter: 50, step: 0.1 }
    }
}

/// The optimize policy's result (T17, DVG-14).
#[derive(Debug, Clone)]
pub struct OptimizeResult {
    /// The final design parameters, keyed `"label.param"`.
    pub params: HashMap<String, f64>,
    /// The objective's value at the final point.
    pub objective_value: f64,
    /// `true` when every `require` margin is ≥ 0 at the final point.
    pub feasible: bool,
    /// Total solver-facing iterations across both phases (feasibility
    /// steps + objective steps, not counting the internal evaluations a
    /// line search or the fallback engine spends per step).
    pub iterations: usize,
    /// Every engine that produced part of this result (DVG-15) — a run
    /// that composed gradient feasibility with a black-box objective
    /// names both.
    pub engines: HashSet<Engine>,
}

/// Run the optimize policy: [`super::Driver::run`]'s dispatch target for
/// [`super::Policy::Optimize`]. `config` always carries `collect` posture
/// (forced by [`super::Driver::run`]).
pub(super) fn run(
    session: &mut Session,
    spec: &OptimizeSpec,
    config: &SolverConfig,
) -> Result<OptimizeResult, Error> {
    let top = session.module().to_string();
    let design = session.design().clone();

    // An objective naming an unknown var/net fails loud at the host
    // boundary (T17 AC2) — before any restamp happens.
    {
        let op = session.op(config, None)?;
        super::read_metric(&op, &spec.objective).map_err(|_| {
            Error::Measurement(format!(
                "optimize: objective `{}` is not a declared margin/var name or a solved net",
                spec.objective
            ))
        })?;
    }

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

    let mut engines: HashSet<Engine> = HashSet::new();
    let mut iterations = 0usize;

    // ── feasibility phase (AC1): climb until every require margin >= 0 ──
    for _ in 0..spec.max_iter {
        let op = session.op(config, None)?;
        if op.requires_ok() {
            break;
        }
        let worst = op
            .margins()
            .entries()
            .iter()
            .filter(|e| e.kind == MarginKind::Require && !e.is_satisfied())
            .min_by(|a, b| a.scalar().partial_cmp(&b.scalar()).expect("finite margins"))
            .expect("requires_ok() == false implies a violated require exists");
        let name = format!("{}.{}", worst.instance_path, worst.label);
        drop(op);

        iterations += 1;
        engines.insert(Engine::Gradient);
        let grad = finite_diff_metric(session, &spec.over, &current, &name, config)?;
        let norm = grad.iter().map(|g| g * g).sum::<f64>().sqrt();
        if norm < 1e-12 {
            break;
        }
        for ((label, param), g) in spec.over.iter().zip(&grad) {
            let base = current[&(label.clone(), param.clone())];
            // Ascend: the finite-difference gradient of the margin's own
            // scalar value already carries the require's sign convention
            // (m >= 0 satisfied), so moving in +gradient direction always
            // increases the margin.
            let new_v = base + spec.step * base.abs().max(1e-6) * (g / norm);
            current.insert((label.clone(), param.clone()), new_v);
            session.set(label, param, new_v)?;
        }
    }

    let feasible_after_climb = session.op(config, None)?.requires_ok();

    // ── objective phase (AC1): descend while projecting onto feasible ──
    if feasible_after_climb {
        match session.gradient(&spec.objective, &spec.over, config) {
            Ok(_) => {
                engines.insert(Engine::Gradient);
                run_gradient_objective(session, spec, &mut current, config, &mut iterations)?;
            }
            Err(e) => {
                return Err(Error::Measurement(format!(
                    "optimize: objective `{}` is not analytically differentiable ({e}) — a \
                     derivative-free fallback is not composed in this delivery yet \
                     (dv-gradients T18)",
                    spec.objective
                )));
            }
        }
    }

    let final_op = session.op(config, None)?;
    let objective_value = super::read_metric(&final_op, &spec.objective)?;
    let feasible = final_op.requires_ok();
    drop(final_op);

    let params = current.into_iter().map(|((l, p), v)| (format!("{l}.{p}"), v)).collect();
    Ok(OptimizeResult { params, objective_value, feasible, iterations, engines })
}

/// Gradient descent (or ascent, per `spec.minimize`) on `spec.objective`,
/// with a bounded backtracking line search that rejects any trial that
/// leaves the feasible set. Terminates on convergence (a near-zero
/// gradient), on a fully-backtracked rejected step (a local optimum or a
/// gradient discontinuity the line search cannot climb past — D1), or on
/// `spec.max_iter` — never stalls or loops unboundedly.
fn run_gradient_objective(
    session: &mut Session,
    spec: &OptimizeSpec,
    current: &mut HashMap<(String, String), f64>,
    config: &SolverConfig,
    iterations: &mut usize,
) -> Result<(), Error> {
    for _ in 0..spec.max_iter {
        *iterations += 1;
        let g = session.gradient(&spec.objective, &spec.over, config)?;
        let dir: Vec<f64> = spec.over.iter().map(|(l, p)| g.get(l, p).unwrap_or(0.0)).collect();
        let norm = dir.iter().map(|d| d * d).sum::<f64>().sqrt();
        if norm < 1e-10 {
            break;
        }
        let sign = if spec.minimize { -1.0 } else { 1.0 };
        let obj0 = super::read_metric(&session.op(config, None)?, &spec.objective)?;

        let mut step = spec.step;
        let mut accepted = false;
        for _ in 0..20 {
            let mut trial = current.clone();
            for ((label, param), d) in spec.over.iter().zip(&dir) {
                let base = current[&(label.clone(), param.clone())];
                let nv = base + sign * step * base.abs().max(1e-6) * (d / norm);
                trial.insert((label.clone(), param.clone()), nv);
            }
            for (label, param) in &spec.over {
                session.set(label, param, trial[&(label.clone(), param.clone())])?;
            }
            let op_try = session.op(config, None)?;
            if !op_try.requires_ok() {
                step *= 0.5;
                continue;
            }
            let obj_try = super::read_metric(&op_try, &spec.objective)?;
            let improved =
                if spec.minimize { obj_try < obj0 - 1e-15 } else { obj_try > obj0 + 1e-15 };
            if improved {
                *current = trial;
                accepted = true;
                break;
            }
            step *= 0.5;
        }
        if !accepted {
            // Restore the last accepted point — the final rejected trial
            // left `session` holding its (worse or infeasible) values.
            for (label, param) in &spec.over {
                session.set(label, param, current[&(label.clone(), param.clone())])?;
            }
            break;
        }
    }
    Ok(())
}

/// Central finite-difference gradient of `metric`'s scalar value
/// ([`super::read_metric`]) w.r.t. `over`, at the point in `current` —
/// two `Session::op` solves per parameter, restoring `metric`'s owning
/// parameter to its base value afterward. Shared by the feasibility
/// phase (climbing a violated require) and the center policy (T19,
/// ascending the active normalized constraint).
pub(super) fn finite_diff_metric(
    session: &mut Session,
    over: &[(String, String)],
    current: &HashMap<(String, String), f64>,
    metric: &str,
    config: &SolverConfig,
) -> Result<Vec<f64>, Error> {
    let mut grad = Vec::with_capacity(over.len());
    for (label, param) in over {
        let base = current[&(label.clone(), param.clone())];
        let h = (base.abs() * 1e-4).max(1e-6);
        session.set(label, param, base + h)?;
        let plus = super::read_metric(&session.op(config, None)?, metric)?;
        session.set(label, param, base - h)?;
        let minus = super::read_metric(&session.op(config, None)?, metric)?;
        session.set(label, param, base)?;
        grad.push((plus - minus) / (2.0 * h));
    }
    Ok(grad)
}
