//! The DC convergence plan: a composable escalation of homotopy strategies, and
//! the shared numerical limits every driver honors.
//!
//! Plain Newton converges most circuits. Stiff coupled-junction operating
//! points (BJT/MOS) need a homotopy — reshape the problem into an easy one and
//! track the solution back. Rather than an inline `match … Err => match … Err`
//! cascade in the DC driver, the escalation is a [`ConvergencePlan`]: a list of
//! [`HomotopyStrategy`] the driver falls through in order. Each strategy is
//! stateless; every piece of mutable solve state lives behind the
//! [`HomotopyDriver`] the plan drives.
//!
//! [`PlanLimits`] is the home for numerical caps that used to live as literals
//! inside drivers (mixed-signal DC settle cap, digital delta-cycle cap,
//! scheduler time-equality epsilon). One knob for hosts; one place to look for
//! the solver's hidden constants.

use ndarray::Array1;
use ndarray::{ArrayView1, ArrayViewMut1};
use crate::analog::Netlist;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::{Policy, Tolerances};
use crate::result::Result;

/// Numerical caps honored across drivers. Replaces the literals that used to
/// live inline in DC and the digital scheduler.
#[derive(Debug, Clone, Copy)]
pub struct PlanLimits {
    /// Maximum number of (Newton + digital settle) alternations in DC before
    /// reporting that the mixed-signal loop did not stabilize.
    pub max_mixed_signal_iter: usize,
    /// Maximum delta cycles in a single digital evaluation time. Above this,
    /// the scheduler reports a combinational loop instead of warning.
    pub max_delta_cycles: usize,
    /// Absolute time equality tolerance when comparing two event times.
    pub digital_time_epsilon: f64,
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            max_mixed_signal_iter: 20,
            max_delta_cycles: 1000,
            digital_time_epsilon: 1e-12,
        }
    }
}

// ── NewtonStrategy ─────────────────────────────────────────────────────────

/// Newton iteration policy: damping, convergence test, iteration cap.
/// The [`ConvergencePlan`] owns one; [`NewtonRaphsonSolver`] consults it
/// instead of calling `NonLinearSystem::apply_limit`/`converged`/
/// `residual_converged` directly (MD-05, MD-13 rule 2).
pub trait NewtonStrategy: Send + Sync {
    /// Damp the Newton update in-place before the convergence test.
    /// `policy.dc_damp_tolerance` controls the threshold.
    fn damp_update(
        &self,
        prev: ArrayView1<f64>,
        update: ArrayViewMut1<f64>,
        policy: &Policy,
    );

    /// Converged if: update test passes AND residual test passes.
    /// Device limiting (`limiting_active()`) is NOT checked here — the driver
    /// gates on it separately after solve returns. This keeps the strategy
    /// borrowing only the netlist, not the device vector.
    fn is_converged(
        &self,
        state: &CircularArrayBuffer2<f64>,
        guess: &ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &Netlist,
        tolerances: &Tolerances,
    ) -> bool;

    /// Maximum Newton iterations.
    fn max_iter(&self, policy: &Policy) -> usize;
}

/// Default Newton strategy: midpoint damping + voltage-step + residual
/// convergence. Body is the exact logic from today's free fns
/// `check_convergence`, `residual_converged`, `apply_damping` in
/// `solver/mod.rs`, just moved into a trait impl.
pub struct DampedNewton;

impl NewtonStrategy for DampedNewton {
    fn damp_update(
        &self,
        prev: ArrayView1<f64>,
        mut update: ArrayViewMut1<f64>,
        policy: &Policy,
    ) {
        let last_guess = prev;
        let diff_norm_sq: f64 = update
            .iter()
            .zip(last_guess.iter())
            .fold(0.0, |acc, (curr, prev)| acc + (curr - prev).powi(2));
        let diff_norm = diff_norm_sq.sqrt();
        if diff_norm >= policy.dc_damp_tolerance {
            for (curr, prev) in update.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }
        }
    }

    fn is_converged(
        &self,
        state: &CircularArrayBuffer2<f64>,
        guess: &ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &Netlist,
        tolerances: &Tolerances,
    ) -> bool {
        // 1. Update convergence test
        if !tolerances.has_converged(state.view(0), guess, netlist) {
            return false;
        }
        // 2. Residual convergence test (ngspice NIconvTest)
        use crate::math::linear::AsIndex;
        for r in netlist.all_references() {
            let Some(i) = r.as_index() else { continue };
            if i >= residual.len() {
                continue;
            }
            let abs_limit = if r.variable().is_branch() {
                tolerances.abstol
            } else {
                tolerances.vntol
            };
            let tol = abs_limit + tolerances.reltol * scale[i];
            if residual[i].abs() > tol {
                return false;
            }
        }
        true
    }

    fn max_iter(&self, policy: &Policy) -> usize {
        policy.max_iter
    }
}

// ── StepperStrategy ────────────────────────────────────────────────────────

/// Transient timestep policy: propose the next `dt` after a successful step,
/// and return a reduced `dt` after a rejection.
pub trait StepperStrategy: Send + Sync {
    /// Propose the next timestep after an accepted step.
    /// `dt_actual` is the dt used for this step; `dt_proposed` is what was
    /// requested before event/stop-time clamping (for growth).
    fn propose_dt(
        &self,
        current_time: f64,
        dt_actual: f64,
        dt_proposed: f64,
        dt_prev: f64,
        circuit: &crate::core::circuit::CircuitInstance,
        solver_state: &CircularArrayBuffer2<f64>,
        tolerances: &Tolerances,
        tran_opts: &crate::analysis::transient::TransientAnalysisOptions,
    ) -> f64;

    /// Reduced timestep after a failed (rejected) step.
    fn reject_dt(
        &self,
        failed_dt: f64,
        tran_opts: &crate::analysis::transient::TransientAnalysisOptions,
    ) -> f64;
}

/// Default stepper: LTE-driven with 2× growth fallback and 0.5× on reject.
pub struct LteStepper;

impl StepperStrategy for LteStepper {
    fn propose_dt(
        &self,
        _current_time: f64,
        dt_actual: f64,
        dt_proposed: f64,
        dt_prev: f64,
        circuit: &crate::core::circuit::CircuitInstance,
        solver_state: &CircularArrayBuffer2<f64>,
        tolerances: &Tolerances,
        tran_opts: &crate::analysis::transient::TransientAnalysisOptions,
    ) -> f64 {
        use crate::analysis::transient::TransientAnalysisState;
        let method = tolerances.integration;
        let time_history = [dt_actual, dt_prev];
        let tran_state = TransientAnalysisState::new(solver_state, &[]);
        let mut lte_dt = tran_opts.dt_max;
        let mut any_lte = false;
        let ctx = crate::solver::Context { tolerances: *tolerances, ..Default::default() };
        for dev in &circuit.devices {
            if let Some(sug) = dev.suggest_transient_step(
                &tran_state,
                &time_history,
                method,
                &ctx,
            ) {
                if sug > 0.0 && sug < lte_dt {
                    lte_dt = sug;
                    any_lte = true;
                }
            }
        }
        if any_lte {
            lte_dt.clamp(tran_opts.dt_min, tran_opts.dt_max)
        } else {
            (dt_proposed * 2.0).clamp(tran_opts.dt_min, tran_opts.dt_max)
        }
    }

    fn reject_dt(
        &self,
        failed_dt: f64,
        tran_opts: &crate::analysis::transient::TransientAnalysisOptions,
    ) -> f64 {
        (failed_dt * 0.5).max(tran_opts.dt_min)
    }
}

/// What a [`HomotopyStrategy`] drives: the plain-Newton solve and the two SPICE
/// homotopy scales it ramps. The DC solver implements this; a strategy never
/// reaches into the solver's internals.
pub trait HomotopyDriver {
    /// Solve plain Newton from the current warm start, with whatever homotopy
    /// scales are set. `Ok` is the converged solution.
    fn newton(&mut self) -> Result<Array1<f64>>;

    /// Set the extra node-to-ground conductance (gmin stepping). `0.0` disables.
    fn set_gmin_extra(&mut self, g: f64);

    /// Set the forced-source scale (source stepping). `1.0` is full strength.
    fn set_src_scale(&mut self, s: f64);

    /// The smallest meaningful extra conductance — the real device gmin,
    /// floored — below which gmin stepping has effectively reached zero.
    fn gmin_floor(&self) -> f64;
}

/// One homotopy: reshapes a hard operating-point problem into an easy one and
/// tracks the solution back to full strength. Stateless — all mutable state is
/// behind the [`HomotopyDriver`].
pub trait HomotopyStrategy: Send + Sync {
    /// Short name for diagnostics/tracing.
    fn name(&self) -> &str;

    /// Attempt to converge to the true operating point, or fail so the plan
    /// falls through to the next strategy.
    fn converge(&self, driver: &mut dyn HomotopyDriver) -> Result<Array1<f64>>;
}

/// The DC convergence plan: plain Newton, then each homotopy in order until one
/// converges. Replaces the hand-inlined homotopy cascade in the DC driver, and
/// is the seam where an analysis or host selects a different escalation.
pub struct ConvergencePlan {
    newton: Box<dyn NewtonStrategy>,
    strategies: Vec<Box<dyn HomotopyStrategy>>,
    limits: PlanLimits,
}

impl Default for ConvergencePlan {
    /// SPICE's standard escalation: [`GminStepping`] first (cheap, robust), then
    /// [`SourceStepping`] (finds the correct solution branch where gmin stepping
    /// can settle on the wrong one — BJT/MOS amplifiers).
    fn default() -> Self {
        Self {
            newton: Box::new(DampedNewton),
            strategies: vec![Box::new(GminStepping), Box::new(SourceStepping)],
            limits: PlanLimits::default(),
        }
    }
}

impl ConvergencePlan {
    /// Build a plan from an explicit strategy list (escalation order preserved).
    pub fn new(strategies: Vec<Box<dyn HomotopyStrategy>>) -> Self {
        Self {
            newton: Box::new(DampedNewton),
            strategies,
            limits: PlanLimits::default(),
        }
    }

    /// Override the Newton strategy.
    pub fn with_newton(mut self, newton: Box<dyn NewtonStrategy>) -> Self {
        self.newton = newton;
        self
    }

    /// Override the numerical limits honored across drivers.
    pub fn with_limits(mut self, limits: PlanLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The Newton iteration policy.
    pub fn newton(&self) -> &dyn NewtonStrategy {
        self.newton.as_ref()
    }

    /// Numerical caps every driver should honor.
    pub fn limits(&self) -> PlanLimits {
        self.limits
    }

    /// Run the plan: plain Newton, then each homotopy in order. Returns the
    /// first converged solution, else the most recent failure.
    pub fn solve(&self, driver: &mut dyn HomotopyDriver) -> Result<Array1<f64>> {
        let mut last = match driver.newton() {
            Ok(solution) => return Ok(solution),
            Err(err) => err,
        };
        for strategy in &self.strategies {
            match strategy.converge(driver) {
                Ok(solution) => return Ok(solution),
                Err(err) => last = err,
            }
        }
        Err(last)
    }
}

/// SPICE gmin stepping: converge an easy, diagonally-dominant version of the
/// circuit (large node-to-ground conductance), then ramp that conductance to 0,
/// warm-starting each step. The standard homotopy for stiff coupled-junction
/// operating points that plain Newton oscillates on.
pub struct GminStepping;

impl HomotopyStrategy for GminStepping {
    fn name(&self) -> &str {
        "gmin-stepping"
    }

    fn converge(&self, driver: &mut dyn HomotopyDriver) -> Result<Array1<f64>> {
        let trace = std::env::var("PIPERINE_TRACE_GMIN").is_ok();
        // Ramp until the extra conductance is negligible next to the real gmin.
        let floor = driver.gmin_floor() * 10.0;
        // Start very easy (100 mS to ground) and drop a decade per step, with
        // adaptive back-off: a step that won't converge raises the conductance
        // again (smaller decrements) instead of giving up. Bounded so a truly
        // non-convergent circuit still terminates.
        let mut g = 0.1_f64;
        let mut factor = 0.1_f64;
        let mut converged_any = false;
        for _ in 0..200 {
            driver.set_gmin_extra(g);
            let result = driver.newton();
            if trace {
                eprintln!("GMIN step g={g:.3e} -> {}", if result.is_ok() { "ok" } else { "fail" });
            }
            match result {
                Ok(_) => {
                    converged_any = true;
                    if g <= floor {
                        break;
                    }
                    factor = (factor * 1.3).min(0.5); // relax faster once it's easy
                    g *= factor;
                }
                Err(err) => {
                    if !converged_any {
                        // Couldn't even converge the easiest problem — give up.
                        driver.set_gmin_extra(0.0);
                        return Err(err);
                    }
                    factor = (factor * 3.0).min(0.7); // back off: raise g, shrink step
                    g /= factor;
                }
            }
        }
        // Final solve with the extra conductance removed — the true operating
        // point, warm-started from the last stepped solution.
        driver.set_gmin_extra(0.0);
        if trace {
            eprintln!("GMIN final solve at gmin_extra=0");
        }
        driver.newton()
    }
}

/// SPICE source stepping: ramp the forced-source scale 0 → 1, warm-starting each
/// step. At scale 0 every source is off and the circuit converges trivially;
/// raising it tracks the solution continuously to the true operating point. A
/// small knee shunt conditions the exponential turn-on where source stepping
/// alone stalls, then is itself ramped out so the final answer is exact.
pub struct SourceStepping;

impl HomotopyStrategy for SourceStepping {
    fn name(&self) -> &str {
        "source-stepping"
    }

    fn converge(&self, driver: &mut dyn HomotopyDriver) -> Result<Array1<f64>> {
        let trace = std::env::var("PIPERINE_TRACE_SRC").is_ok();
        // A real shunt conductance (1 µS) conditions the exponential turn-on
        // knee (the BJT/MOS threshold), held through the source ramp then itself
        // ramped to 0 (a nested gmin step) so the final answer is exact.
        let knee_gmin = 1e-6_f64;
        let mut scale = 0.0_f64;
        let mut step = 0.1_f64;
        let mut last_ok = 0.0_f64;
        driver.set_src_scale(0.0);
        driver.set_gmin_extra(knee_gmin);
        // Solve the fully-off circuit first (trivial).
        let mut sol = driver.newton();
        for _ in 0..300 {
            if sol.is_ok() {
                last_ok = scale;
                if scale >= 1.0 {
                    break;
                }
                step = (step * 1.5).min(0.25);
                scale = (last_ok + step).min(1.0);
            } else {
                // Back off toward the last converged scale.
                step *= 0.5;
                if step < 1e-6 {
                    driver.set_src_scale(1.0);
                    driver.set_gmin_extra(0.0);
                    return sol; // give up with the failure
                }
                scale = last_ok + step;
            }
            driver.set_src_scale(scale);
            if trace {
                eprintln!("SRC step scale={scale:.4} step={step:.4}");
            }
            sol = driver.newton();
        }
        // Full source strength reached with the knee shunt still in. Ramp the
        // shunt out (a nested gmin step, warm-started) so the final answer is
        // exact.
        driver.set_src_scale(1.0);
        let mut g = knee_gmin;
        while g > driver.gmin_floor() * 10.0 {
            g *= 0.1;
            driver.set_gmin_extra(g);
            if driver.newton().is_err() {
                break;
            }
        }
        driver.set_gmin_extra(0.0);
        driver.newton()
    }
}
