//! Periodic steady state (PSS) — options, result types, and the
//! single-shooting driver (MD: user 2026-07-18, "inefficient but
//! sufficient for V1"): Newton on `g(x₀) = x(t₀+T) − x₀` where each
//! evaluation of `g` is an ordinary transient over one period re-entered
//! from `x₀` ([`TransientSolver::with_initial_state`]). Mixed signal runs
//! unchanged inside every shot (scheduler, breakpoints, bridges); Newton
//! sees only the continuous unknowns. The first Jacobian is finite
//! difference (n extra shots), then Broyden rank-1 updates. Digital
//! periodicity is a post-convergence verification — a mismatch fails loud,
//! and when the digital state closes only after `k ≤ 4` periods the error
//! names "circuit period appears to be k·T" (divider case).

use std::collections::HashMap;
use std::sync::Arc;

use crate::analyses::transient::{TransientAnalysisOptions, TransientSolver};
use crate::core::circuit::CircuitInstance;
use crate::error::{Error, SolverDomain};
use crate::result::TransientAnalysisResult;
use crate::result::TransientStep;
use crate::analyses::Policy;
use crate::Context;
use crate::analog::netlist::AnalogVariable;

// ── request/state ────────────────────────────────────────────────────────

/// Single-shooting PSS configuration. The user supplies the drive period
/// `T` (driven circuits only — autonomous period detection is out of
/// scope); `tstab` is an optional pre-roll transient that moves the
/// starting state near the orbit before Newton begins.
#[derive(Debug, Clone)]
pub struct PssAnalysisOptions {
    /// The period `T` the steady state must satisfy (`x(t0+T) = x(t0)`).
    pub period: f64,
    /// Pre-roll length: integrate `[0, tstab]` once and shoot from there.
    pub tstab: f64,
    /// Shooting-Newton iteration cap.
    pub max_shoot_iter: usize,
    /// Convergence bound on `max_i |x_i(T) − x_i(0)|`. Default `1e-6`,
    /// bounded below by the adaptive integrator per-period reproducibility
    /// (~1e-7) — tighter values just spin at the noise floor.
    pub shoot_tol: f64,
    /// Initial integrator dt for each shot; `None` → `period / 100`.
    pub dt: Option<f64>,
}

impl PssAnalysisOptions {
    pub fn new(period: f64) -> Self {
        Self { period, tstab: 0.0, max_shoot_iter: 40, shoot_tol: 1.0e-6, dt: None }
    }

    pub fn with_tstab(mut self, tstab: f64) -> Self {
        self.tstab = tstab;
        self
    }
}

/// Shooting diagnostics: how many Newton iterations the orbit took, the
/// final periodicity residual `max_i |x_i(T) − x_i(0)|`, and the estimated
/// natural settling time.
#[derive(Debug, Clone, Copy)]
pub struct PssStats {
    pub shoot_iterations: usize,
    pub residual: f64,
    /// How long a plain transient would need for its free response to decay
    /// below `reltol` — `T·ln(reltol)/ln(ρ)` where `ρ` is the dominant
    /// monodromy eigenvalue magnitude (power iteration on the shooting
    /// Jacobian). `None` when no Jacobian was computed (the start state was
    /// already on the orbit) or when `ρ ≥ 1` (no decaying free response).
    pub estimated_settle_time: Option<f64>,
}

/// A converged periodic orbit (Debug elided: the trace is bulky): one period of transient samples
/// (`t ∈ [t0, t0+T]`) plus the shooting diagnostics.
pub struct PssResult {
    pub trace: TransientAnalysisResult,
    pub stats: PssStats,
}

impl std::fmt::Debug for PssResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PssResult").field("stats", &self.stats).finish_non_exhaustive()
    }
}

// ── driver ───────────────────────────────────────────────────────────────

pub struct PssSolver<'a> {
    circuit: &'a mut CircuitInstance,
    options: PssAnalysisOptions,
    context: Context,
    /// Convergence tunables applied to every inner transient (MD-04).
    pub policy: Policy,
}

impl<'a> PssSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: PssAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        if !(options.period > 0.0) {
            return Err(Error::simple(
                SolverDomain::Pss,
                format!("period must be positive (got {})", options.period),
            ));
        }
        if options.tstab < 0.0 {
            return Err(Error::simple(
                SolverDomain::Pss,
                format!("tstab must be non-negative (got {})", options.tstab),
            ));
        }
        Ok(Self { circuit, options, context, policy: Policy::default() })
    }

    /// The index-ordered continuous unknowns (nodes + branch currents).
    fn variables(&self) -> Vec<(usize, Arc<AnalogVariable>)> {
        let mut vars: Vec<(usize, Arc<AnalogVariable>)> = self
            .circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter_map(|r| {
                use crate::math::linear::AsIndex;
                r.as_index().map(|i| (i, r.variable().clone()))
            })
            .collect();
        vars.sort_by_key(|(i, _)| *i);
        vars
    }

    fn step_to_vec(step: &TransientStep, vars: &[(usize, Arc<AnalogVariable>)]) -> Vec<f64> {
        vars.iter().map(|(_, v)| step.get(v.clone()).unwrap_or(0.0)).collect()
    }

    fn vec_to_step(
        x: &[f64],
        vars: &[(usize, Arc<AnalogVariable>)],
        time: f64,
        digital_from: &TransientStep,
    ) -> TransientStep {
        let mut values: HashMap<Arc<AnalogVariable>, f64> = HashMap::new();
        for ((_, var), &v) in vars.iter().zip(x) {
            values.insert(var.clone(), v);
        }
        let digital: Vec<crate::digital::LogicValue> =
            (0..usize::MAX).map_while(|i| digital_from.digital(i)).collect();
        TransientStep::new(time, values).with_digital(digital)
    }

    /// One shot: integrate `[t0, t0+span]` from `state`, returning the full
    /// recorded trace (last step = the arrival state).
    fn shoot(
        &mut self,
        state: &TransientStep,
        t0: f64,
        span: f64,
    ) -> crate::result::Result<crate::result::TransientAnalysisResult> {
        let dt = self.options.dt.unwrap_or(self.options.period / 100.0);
        let opts = TransientAnalysisOptions::new(t0 + span, dt).with_start(t0);
        let mut solver = TransientSolver::new(self.circuit, opts, self.context.clone())?;
        solver.policy = self.policy.clone();
        solver.with_initial_state(state);
        solver.solve()
    }

    pub fn solve(mut self) -> crate::result::Result<PssResult> {
        let period = self.options.period;
        let t0 = self.options.tstab;
        let vars = self.variables();
        let n = vars.len();

        // Starting state: DC operating point, optionally rolled to `tstab`.
        let mut x0_step: TransientStep = {
            let dt = self.options.dt.unwrap_or(period / 100.0);
            if t0 > 0.0 {
                let opts = TransientAnalysisOptions::new(t0, dt);
                let mut solver =
                    TransientSolver::new(self.circuit, opts, self.context.clone())?;
                solver.policy = self.policy.clone();
                solver.solve()?.last().expect("pre-roll has steps").clone()
            } else {
                // A zero-length "shot" is not defined; take the DC point via
                // a minimal transient record at t=0: integrate one dt and
                // keep the *initial* snapshot (index 0).
                let opts = TransientAnalysisOptions::new(dt, dt);
                let mut solver =
                    TransientSolver::new(self.circuit, opts, self.context.clone())?;
                solver.policy = self.policy.clone();
                let r = solver.solve()?;
                r.iter().next().expect("transient records t=0").clone()
            }
        };

        let mut x0 = Self::step_to_vec(&x0_step, &vars);
        let mut jacobian: Option<Vec<Vec<f64>>> = None; // (M − I), row-major
        let mut last_dx: Vec<f64> = vec![0.0; n];
        let mut last_g: Vec<f64> = Vec::new();
        let mut residual = f64::INFINITY;

        for iter in 0..self.options.max_shoot_iter.max(1) {
            let arrival = self.shoot(&x0_step, t0, period)?;
            let x_t = Self::step_to_vec(arrival.last().expect("shot has steps"), &vars);
            let g: Vec<f64> = x_t.iter().zip(&x0).map(|(a, b)| a - b).collect();
            residual = g.iter().fold(0.0_f64, |m, v| m.max(v.abs()));

            if residual < self.options.shoot_tol {
                // Converged — but a fixed point of the period map is only a
                // steady state when the drive itself is T-periodic: a linear
                // circuit under a non-periodic drive still has an (affine)
                // fixed point. Verify the orbit *repeats*: one extra shot
                // over the second period must land where the first did,
                // within the integration-tolerance class (not shoot_tol —
                // per-period LTE drift is larger than the Newton residual).
                let arrival_step = arrival.last().unwrap().clone();
                let second = self.shoot(&arrival_step, t0 + period, period)?;
                let x_2t = Self::step_to_vec(second.last().unwrap(), &vars);
                for (i, (a, b)) in x_t.iter().zip(&x_2t).enumerate() {
                    let tol = 1.0e-9 + 1.0e-3 * a.abs().max(b.abs());
                    if (a - b).abs() > tol {
                        return Err(Error::simple(
                            SolverDomain::Pss,
                            format!(
                                "shooting found a fixed point, but the orbit does not repeat: \
                                 |x(2T) − x(T)| = {:.3e} on `{}` — the drive is not periodic \
                                 at T={period:.3e}",
                                (a - b).abs(),
                                format!("{:?}", vars[i].1)
                            ),
                        ));
                    }
                }
                self.verify_digital_periodicity(&x0_step, &arrival_step, t0, period)?;
                let trace = self.shoot(&x0_step, t0, period)?;
                let estimated_settle_time = jacobian
                    .as_ref()
                    .and_then(|j| dominant_monodromy_magnitude(j))
                    .filter(|rho| *rho < 1.0 && *rho > 0.0)
                    .map(|rho| {
                        period * (self.context.tolerances.reltol.ln() / rho.ln()).max(0.0)
                    });
                return Ok(PssResult {
                    trace,
                    stats: PssStats { shoot_iterations: iter, residual, estimated_settle_time },
                });
            }

            // Jacobian: FD on the first iteration, Broyden updates after.
            let j = match jacobian.take() {
                None => {
                    let mut m = vec![vec![0.0_f64; n]; n];
                    for col in 0..n {
                        let eps = 1.0e-7 * x0[col].abs().max(1.0e-3);
                        let mut xp = x0.clone();
                        xp[col] += eps;
                        let xp_step = Self::vec_to_step(&xp, &vars, t0, &x0_step);
                        let arr = self.shoot(&xp_step, t0, period)?;
                        let xtp = Self::step_to_vec(arr.last().unwrap(), &vars);
                        for row in 0..n {
                            // d g_row / d x0_col = M − I (monodromy minus identity).
                            m[row][col] = (xtp[row] - x_t[row]) / eps
                                - if row == col { 1.0 } else { 0.0 };
                        }
                    }
                    m
                }
                Some(mut m) => {
                    // Broyden: J += ((Δg − J·Δx) Δxᵀ) / (Δxᵀ Δx)
                    let dg: Vec<f64> =
                        g.iter().zip(&last_g).map(|(a, b)| a - b).collect();
                    let dx = &last_dx;
                    let dxtdx: f64 = dx.iter().map(|v| v * v).sum();
                    if dxtdx > 0.0 {
                        let jdx: Vec<f64> = m
                            .iter()
                            .map(|row| row.iter().zip(dx).map(|(a, b)| a * b).sum::<f64>())
                            .collect();
                        for row in 0..n {
                            let coef = (dg[row] - jdx[row]) / dxtdx;
                            for col in 0..n {
                                m[row][col] += coef * dx[col];
                            }
                        }
                    }
                    m
                }
            };

            // Solve J·dx = −g (dense Gaussian elimination, partial pivot).
            let dx = solve_dense(&j, &g.iter().map(|v| -v).collect::<Vec<_>>()).ok_or_else(
                || {
                    Error::simple(
                        SolverDomain::Pss,
                        format!(
                            "shooting Jacobian is singular after {iter} iteration(s) \
                             (residual {residual:.3e})"
                        ),
                    )
                },
            )?;

            // Damped update: an undamped Newton step through an exponential
            // nonlinearity (diodes) can throw x₀ to hundreds of volts, and
            // every subsequent shot becomes a stiff nightmare. Clamp each
            // component to a physically-plausible move.
            for i in 0..n {
                let cap = 10.0 * x0[i].abs().max(1.0);
                x0[i] += dx[i].clamp(-cap, cap);
            }
            x0_step = Self::vec_to_step(&x0, &vars, t0, &x0_step);
            jacobian = Some(j);
            last_dx = dx;
            last_g = g;
        }

        Err(Error::simple(
            SolverDomain::Pss,
            format!(
                "shooting did not converge in {} iterations (final residual {residual:.3e}, \
                 tol {:.1e}) — the circuit may not be periodic at T={period:.3e}",
                self.options.max_shoot_iter, self.options.shoot_tol
            ),
        ))
    }

    /// After analog convergence: the digital state must also close over one
    /// period. When it closes only after `k ≤ 4` periods, say so (divider).
    fn verify_digital_periodicity(
        &mut self,
        start: &TransientStep,
        arrival: &TransientStep,
        t0: f64,
        period: f64,
    ) -> crate::result::Result<()> {
        let matches = |a: &TransientStep, b: &TransientStep| -> bool {
            (0..usize::MAX)
                .map_while(|i| match (a.digital(i), b.digital(i)) {
                    (Some(x), Some(y)) => Some(x == y),
                    (None, None) => None,
                    _ => Some(false),
                })
                .all(|eq| eq)
        };
        if matches(start, arrival) {
            return Ok(());
        }
        // Walk forward whole periods looking for the true digital period.
        let mut state = arrival.clone();
        for k in 2..=4_usize {
            let arr = self.shoot(&state, t0 + (k as f64 - 1.0) * period, period)?;
            let next = arr.last().unwrap().clone();
            if matches(start, &next) {
                return Err(Error::simple(
                    SolverDomain::Pss,
                    format!(
                        "digital state is not periodic at T — circuit period appears to be \
                         {k}·T (digital divider); re-run with period = {:.6e}",
                        k as f64 * period
                    ),
                ));
            }
            state = next;
        }
        Err(Error::simple(
            SolverDomain::Pss,
            "digital state is not periodic at T (and does not close within 4·T)".to_string(),
        ))
    }
}

/// Dominant eigenvalue magnitude of the monodromy `M = J + I` (the shooting
/// Jacobian is `M − I`) by power iteration — the per-period decay factor of
/// the slowest free-response mode.
fn dominant_monodromy_magnitude(j: &[Vec<f64>]) -> Option<f64> {
    let n = j.len();
    if n == 0 {
        return None;
    }
    let mut v = vec![1.0_f64; n];
    let mut rho = 0.0;
    for _ in 0..60 {
        let mut w = vec![0.0_f64; n];
        for row in 0..n {
            let mut acc = if row < v.len() { v[row] } else { 0.0 }; // + I·v
            for col in 0..n {
                acc += j[row][col] * v[col];
            }
            w[row] = acc;
        }
        let norm = w.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
        if norm == 0.0 {
            return Some(0.0);
        }
        for x in &mut w {
            *x /= norm;
        }
        rho = norm;
        v = w;
    }
    Some(rho)
}

/// Dense `A·x = b` by Gaussian elimination with partial pivoting; `None`
/// on a (numerically) singular matrix. Shooting systems are circuit-sized,
/// so dense is fine.
fn solve_dense(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let mut m: Vec<Vec<f64>> = a.iter().cloned().collect();
    let mut x: Vec<f64> = b.to_vec();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| {
            m[i][col].abs().total_cmp(&m[j][col].abs())
        })?;
        if m[pivot][col].abs() < 1.0e-300 {
            return None;
        }
        m.swap(col, pivot);
        x.swap(col, pivot);
        for row in (col + 1)..n {
            let f = m[row][col] / m[col][col];
            for k in col..n {
                m[row][k] -= f * m[col][k];
            }
            x[row] -= f * x[col];
        }
    }
    for col in (0..n).rev() {
        for row in 0..col {
            let f = m[row][col] / m[col][col];
            x[row] -= f * x[col];
        }
        x[col] /= m[col][col];
    }
    Some(x)
}

impl std::fmt::Debug for PssSolver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PssSolver").field("options", &self.options).finish_non_exhaustive()
    }
}
