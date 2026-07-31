//! Derivative-free fallback engine (dv-gradients T18, DVG-15): a
//! Nelder–Mead simplex search over the design parameters, used for the
//! objective phase when [`crate::Session::gradient`] cannot differentiate
//! the objective. Composed with the gradient path from T17: the
//! feasibility phase (always continuous `require` margins) still climbs
//! on the analytic gradient; only the objective phase falls back —
//! "gradient inside a discrete shell" — and the result names both
//! engines when both ran ([`super::optimize::Engine`], DVG-15).
//!
//! Pure Rust, no new dependency (T18's constraint against pulling in a
//! heavy optimization crate for this): a small textbook Nelder–Mead
//! implementation (reflect/expand/contract/shrink, the standard
//! coefficients), more than adequate for the low-dimensional
//! design-parameter spaces this policy searches. If a host ever wires in
//! scipy/BoTorch on the Python side, they are optional accelerators —
//! the Rust engine here is the source of truth and must reach the same
//! answers without them (DVG-15 AC4).

use std::collections::{HashMap, HashSet};

use piperine_solver::prelude::MarginKind;

use crate::error::Error;
use crate::session::{Session, SolverConfig};

use super::optimize::{Engine, OptimizeSpec};

/// Run the objective phase with the black-box engine (T18): a
/// Nelder–Mead search over `spec.over`, minimizing/maximizing
/// `spec.objective`'s RAW value ([`super::read_metric`]) with an
/// infeasibility penalty — a violated `require` margin adds a large
/// smooth cost, steering the simplex back toward the feasible set rather
/// than treating a non-convergent trial as a crash (it is simply scored
/// as a bad point). Updates `current`/`session` to the best point found
/// and marks [`Engine::BlackBox`] in `engines`.
pub(super) fn compose_objective(
    session: &mut Session,
    spec: &OptimizeSpec,
    current: &mut HashMap<(String, String), f64>,
    config: &SolverConfig,
    iterations: &mut usize,
    engines: &mut HashSet<Engine>,
) -> Result<(), Error> {
    engines.insert(Engine::BlackBox);
    let x0: Vec<f64> = spec.over.iter().map(|k| current[k]).collect();
    let sign = if spec.minimize { 1.0 } else { -1.0 };

    let mut eval_err: Option<Error> = None;
    let mut evals = 0usize;
    let objective = spec.objective.clone();
    let over = spec.over.clone();
    let mut f = |x: &[f64]| -> f64 {
        evals += 1;
        for ((label, param), &v) in over.iter().zip(x) {
            if let Err(e) = session.set(label, param, v) {
                eval_err.get_or_insert(e);
                return f64::MAX;
            }
        }
        match session.op(config, None) {
            Ok(op) => {
                let penalty: f64 = op
                    .margins()
                    .entries()
                    .iter()
                    .filter(|e| e.kind == MarginKind::Require && !e.is_satisfied())
                    .map(|e| -e.scalar())
                    .sum();
                match super::read_metric(&op, &objective) {
                    Ok(raw) => sign * raw + 1e6 * penalty,
                    Err(e) => {
                        eval_err.get_or_insert(e);
                        f64::MAX
                    }
                }
            }
            // A non-convergent trial is scored as a bad point, never a
            // crash — the search continues past it.
            Err(_) => f64::MAX,
        }
    };

    let best = nelder_mead(&mut f, x0, spec.max_iter.max(10), spec.step);
    if let Some(e) = eval_err {
        return Err(e);
    }
    *iterations += evals;
    for ((label, param), v) in spec.over.iter().zip(&best) {
        current.insert((label.clone(), param.clone()), *v);
        session.set(label, param, *v)?;
    }
    Ok(())
}

/// A textbook Nelder–Mead simplex search (reflect/expand/contract/shrink,
/// standard coefficients α=1, γ=2, ρ=0.5, σ=0.5). `f` is evaluated on
/// each simplex vertex; the search always terminates after `max_iter`
/// outer iterations — bounded, never stalls or loops unboundedly
/// regardless of how ragged `f` is (T18's tolerance for a
/// non-differentiable/discontinuous objective).
fn nelder_mead(f: &mut dyn FnMut(&[f64]) -> f64, x0: Vec<f64>, max_iter: usize, step: f64) -> Vec<f64> {
    let n = x0.len();
    if n == 0 {
        return x0;
    }
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
    let f0 = f(&x0);
    simplex.push((x0.clone(), f0));
    for i in 0..n {
        let mut xi = x0.clone();
        let h = if xi[i].abs() > 1e-8 { xi[i] * step } else { step };
        xi[i] += h;
        let fi = f(&xi);
        simplex.push((xi, fi));
    }

    for _ in 0..max_iter {
        simplex.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let worst = simplex[n].clone();
        let second_worst = simplex[n - 1].1;
        let best = simplex[0].1;

        let mut centroid = vec![0.0; n];
        for (x, _) in &simplex[..n] {
            for (c, xi) in centroid.iter_mut().zip(x) {
                *c += xi / n as f64;
            }
        }

        let xr: Vec<f64> = (0..n).map(|i| centroid[i] + (centroid[i] - worst.0[i])).collect();
        let fr = f(&xr);

        if fr < best {
            let xe: Vec<f64> = (0..n).map(|i| centroid[i] + 2.0 * (centroid[i] - worst.0[i])).collect();
            let fe = f(&xe);
            simplex[n] = if fe < fr { (xe, fe) } else { (xr, fr) };
        } else if fr < second_worst {
            simplex[n] = (xr, fr);
        } else {
            let xc: Vec<f64> = (0..n).map(|i| centroid[i] + 0.5 * (worst.0[i] - centroid[i])).collect();
            let fc = f(&xc);
            if fc < worst.1 {
                simplex[n] = (xc, fc);
            } else {
                let (bx, _) = simplex[0].clone();
                for point in simplex.iter_mut().skip(1) {
                    let nx: Vec<f64> = (0..n).map(|i| bx[i] + 0.5 * (point.0[i] - bx[i])).collect();
                    let nf = f(&nx);
                    *point = (nx, nf);
                }
            }
        }
    }
    simplex.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    simplex[0].0.clone()
}
