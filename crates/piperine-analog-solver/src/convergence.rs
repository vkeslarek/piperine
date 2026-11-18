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
//! inside drivers (the mixed-signal DC settle cap). One knob for hosts; one place
//! to look for the solver's hidden constants.

use ndarray::Array1;
use ndarray::{ArrayView1, ArrayViewMut1};
use crate::netlist::Netlist;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::config::{Schedules, StepperGains, TraceFlags};
use crate::tolerances::{Policy, Tolerances};
use crate::error::Result;

/// Numerical caps honored across drivers. Replaces the literals that used to
/// live inline in DC.
#[derive(Debug, Clone, Copy)]
pub struct PlanLimits {
    /// Maximum number of (Newton + digital settle) alternations in DC before
    /// reporting that the mixed-signal loop did not stabilize.
    pub max_mixed_signal_iter: usize,
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            max_mixed_signal_iter: 20,
        }
    }
}

/// The two bounds a transient stepper clamps `dt` into: what it reads of the
/// analysis options.
#[derive(Debug, Clone, Copy)]
pub struct StepLimits {
    pub dt_min: f64,
    pub dt_max: f64,
}

// ── NewtonStrategy ─────────────────────────────────────────────────────────

/// Newton iteration policy: damping, convergence test, iteration cap.
/// The [`ConvergencePlan`] owns one; `NewtonRaphsonSolver` consults it
/// instead of calling `NonLinearSystem::apply_limit`/`converged`/
/// `residual_converged` directly (MD-05, MD-13 rule 2).
pub trait NewtonStrategy: Send + Sync {
    /// Damp the Newton update in-place before the convergence test. `context` says what the
    /// iteration is: ngspice's node damping reads it, [`NoDamping`] ignores it.
    fn damp_update(
        &self,
        prev: ArrayView1<f64>,
        update: ArrayViewMut1<f64>,
        context: &DampContext,
    );

    /// Converged if: update test passes AND residual test passes.
    /// Device limiting (`limiting_report()`) is NOT checked here — the driver
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

/// What a Newton iteration is, as [`NewtonStrategy::damp_update`] sees it.
#[derive(Debug, Clone, Copy)]
pub struct DampContext<'a> {
    /// The solve is an operating point (ngspice `MODEDCOP`/`MODETRANOP`), not a transient step.
    pub static_solve: bool,
    /// Zero-based Newton iteration within the solve.
    pub iteration: usize,
    /// A device reports an active limit in this iteration (ngspice `CKTnoncon`).
    pub limiting: bool,
    /// The unknown indices that are node voltages (branch currents are not damped).
    pub nodes: &'a [usize],
}

/// Default Newton strategy: no damping; the update-and-residual convergence test of
/// `Tolerances` (ngspice's `NIconvTest`). Junction limiting is per device (`LIMIT` inputs).
pub struct NoDamping;

impl NewtonStrategy for NoDamping {
    fn damp_update(&self, _prev: ArrayView1<f64>, _update: ArrayViewMut1<f64>, _context: &DampContext) {}

    fn is_converged(
        &self,
        state: &CircularArrayBuffer2<f64>,
        guess: &ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &Netlist,
        tolerances: &Tolerances,
    ) -> bool {
        // Voltage-step test AND current-residual test (ngspice NIconvTest) —
        // both owned by `Tolerances`, the single home of the convergence math.
        tolerances.has_converged(state.view(0), guess, netlist)
            && tolerances.residual_test(netlist, residual, scale)
    }

    fn max_iter(&self, policy: &Policy) -> usize {
        policy.max_iter
    }
}

/// ngspice's node damping (`niiter.c:297-323`, off by default there): in a static solve, after
/// the first iteration, while a device limits, when the largest node-voltage step exceeds 10 V,
/// every node's step is scaled by `max(10/maxΔv, 0.1)`. Opt in with
/// [`ConvergencePlan::with_newton`]; the convergence test is [`NoDamping`]'s.
pub struct NodeDamping;

impl NodeDamping {
    /// The largest node-voltage step ngspice leaves undamped, in volts.
    const MAX_STEP: f64 = 10.0;
    /// The smallest factor ngspice scales a step by.
    const MIN_FACTOR: f64 = 0.1;
}

impl NewtonStrategy for NodeDamping {
    fn damp_update(&self, prev: ArrayView1<f64>, mut update: ArrayViewMut1<f64>, context: &DampContext) {
        if !context.static_solve || context.iteration == 0 || !context.limiting {
            return;
        }
        let max_step = context.nodes.iter().fold(0.0_f64, |m, &i| m.max((update[i] - prev[i]).abs()));
        if max_step <= Self::MAX_STEP {
            return;
        }
        let factor = (Self::MAX_STEP / max_step).max(Self::MIN_FACTOR);
        for &i in context.nodes {
            update[i] = prev[i] + factor * (update[i] - prev[i]);
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
        NoDamping.is_converged(state, guess, residual, scale, netlist, tolerances)
    }

    fn max_iter(&self, policy: &Policy) -> usize {
        policy.max_iter
    }
}

// ── StepperStrategy ────────────────────────────────────────────────────────

/// What a [`StepperStrategy`] remembers between steps, as numbers: the state a run re-entered from
/// a captured step resumes with. The strategy owns the encoding; an empty memory is a strategy
/// with nothing to carry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepperMemory {
    pub values: Vec<f64>,
}

/// Transient timestep policy. The driver computes the global integration
/// error after each accepted step and hands it here; the strategy proposes
/// the next `dt` (and a reduced `dt` after a rejection). Stateful strategies
/// (the PI controller) carry their own memory across steps.
pub trait StepperStrategy: Send + Sync {
    /// Propose the next timestep after an accepted step. `lte` is the
    /// normalized global error from `TrBdf2::state_lte` (~1.0 = at tolerance);
    /// `dt_actual` is the dt this step used.
    fn propose_dt(
        &mut self,
        lte: f64,
        dt_actual: f64,
        limits: &StepLimits,
    ) -> f64;

    /// Reduced timestep after a failed (rejected) step.
    fn reject_dt(
        &mut self,
        failed_dt: f64,
        limits: &StepLimits,
    ) -> f64;

    /// The memory a later [`resume`](Self::resume) restores. A strategy with none keeps the
    /// default: empty.
    fn memory(&self) -> StepperMemory {
        StepperMemory::default()
    }

    /// Restores a memory this strategy's [`memory`](Self::memory) captured. The default is a no-op.
    fn resume(&mut self, _memory: &StepperMemory) {}
}

/// Proportional-Integral timestep controller — the default TR-BDF2 stepper.
/// Replaces the reactive LTE stepper: instead of shrinking `dt` sharply when
/// an element's local error spikes, it smooths `dt` from the **global** Milne
/// LTE and the previous error, so `dt` does not thrash on stiff transients.
///
/// The law is the standard adaptive form
/// `dt_{n+1} = dt_n · (target/lte)^p`, where the exponent `p` combines a
/// proportional gain `kp` with an integral correction on the error history:
/// `p = kp + ki·(lte − lte_prev)/lte`. `target = 1` (the Milne estimate is
/// already normalized by tolerance). The result is clamped to a safe per-step
/// range and `[dt_min, dt_max]`. A rejection resets the error memory so the
/// retry is not biased (TRB-09). All gains live in [`StepperGains`].
#[derive(Default)]
pub struct PiController {
    pub gains: StepperGains,
    prev_error: Option<f64>,
}

impl PiController {
    /// Build a controller with explicit PI gains (ngspice-lineage defaults are
    /// `kp = 0.7`, `ki = 0.4`); the remaining stepper gains keep their
    /// [`StepperGains`] defaults.
    pub fn new(kp: f64, ki: f64) -> Self {
        Self {
            gains: StepperGains { kp, ki, ..StepperGains::default() },
            prev_error: None,
        }
    }
}

impl StepperStrategy for PiController {
    fn propose_dt(
        &mut self,
        lte: f64,
        dt_actual: f64,
        limits: &StepLimits,
    ) -> f64 {
        // No error signal (non-reactive step, or history too short): grow dt
        // toward dt_max without biasing the PI memory.
        if !lte.is_finite() || lte <= 0.0 {
            self.prev_error = None;
            return (dt_actual * self.gains.grow_factor).clamp(limits.dt_min, limits.dt_max);
        }
        // Exponent: proportional gain + integral correction on error history.
        //
        // Both the relative-change bound and the `max(0.0)` floor are load
        // bearing, and for the same reason: the law `factor = lte^(-p)` is
        // only monotone in the error while `p >= 0`. `de` is normalized by the
        // *current* `lte`, so a step whose error IMPROVES sharply (lte falling
        // 20x is routine right after a switching edge) yields `de ≈ -19` and
        // `p = 0.7 + 0.4·(-19) = -6.9`. With the usual `lte < 1` of a
        // well-resolved step, `lte^(6.9)` is astronomically small and clamps to
        // the 0.2 shrink floor — the controller cuts `dt` hardest exactly when
        // accuracy is best. Paired with the 1.5 growth cap that asymmetry is a
        // ratchet: `dt` walks down to `dt_min` over a few hundred steps with
        // zero rejections (the accept gate is happy — the error is tiny), and
        // the run stops terminating in any useful time.
        //
        // Bounding `de` to +/-1 keeps the integral term a correction rather
        // than a sign flip, and flooring `p` at 0 makes "better error" never
        // mean "smaller step". With the default gains `p` lands in [0.3, 1.1].
        let p = match self.prev_error {
            Some(prev) => {
                let de = ((lte - prev) / lte.max(1e-30)).clamp(-1.0, 1.0);
                (self.gains.kp + self.gains.ki * de).max(0.0)
            }
            None => self.gains.kp, // first accepted step: proportional only
        };
        self.prev_error = Some(lte);
        // (target/lte)^p: error above tolerance (lte > 1) shrinks dt.
        let (lo, hi) = self.gains.factor_clamp;
        let factor = lte.powf(-p).clamp(lo, hi);
        (dt_actual * factor).clamp(limits.dt_min, limits.dt_max)
    }

    fn reject_dt(
        &mut self,
        failed_dt: f64,
        limits: &StepLimits,
    ) -> f64 {
        // Aggressive backtracking: a failed step (Newton non-convergence or
        // LTE-reject) means the local dynamics are too fast for this dt, so
        // cut hard and let the PI regrow on the accepts that follow. The ÷8
        // default reaches a workable dt in one retry instead of the timid ÷2
        // cascade.
        self.prev_error = None;
        (failed_dt / self.gains.reject_divisor).max(limits.dt_min)
    }

    /// The previous error, or nothing before the first accepted step.
    fn memory(&self) -> StepperMemory {
        StepperMemory { values: self.prev_error.into_iter().collect() }
    }

    fn resume(&mut self, memory: &StepperMemory) {
        self.prev_error = memory.values.first().copied();
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

    /// Snapshot every element's non-accept-gated state before an attempt
    /// (ABI-07): plain Newton or a homotopy strategy can dirty device-internal
    /// state (the limiter) before failing; the next attempt must start from the
    /// pre-failure checkpoint, not the dirty rejected-attempt state. Default
    /// no-op — a driver whose devices have no `SUPPORTS_ROLLBACK` pays nothing.
    fn checkpoint_devices(&mut self) {}

    /// Restore elements to the snapshot from the last
    /// [`checkpoint_devices`](Self::checkpoint_devices) before retrying with
    /// the next homotopy parameter (ABI-07). Default no-op.
    fn restore_devices(&mut self) {}
}

/// One homotopy: reshapes a hard operating-point problem into an easy one and
/// tracks the solution back to full strength. Stateless — all mutable state is
/// behind the `HomotopyDriver`; the numeric schedule it ramps is owned by
/// the plan and handed in as [`Schedules`].
pub trait HomotopyStrategy: Send + Sync {
    /// Short name for diagnostics/tracing.
    fn name(&self) -> &'static str;

    /// Attempt to converge to the true operating point, or fail so the plan
    /// falls through to the next strategy. `trace` carries the diagnostic
    /// toggles (seeded from `Policy::trace` by the driver).
    fn converge(
        &self,
        driver: &mut dyn HomotopyDriver,
        schedules: &Schedules,
        trace: TraceFlags,
    ) -> Result<Array1<f64>>;
}

/// The DC convergence plan: plain Newton, then each homotopy in order until one
/// converges. Replaces the hand-inlined homotopy cascade in the DC driver, and
/// is the seam where an analysis or host selects a different escalation. Owns
/// the [`Schedules`] every strategy reads its numeric ramp from, and the
/// [`TraceFlags`] the driver seeds from its [`Policy`].
///
/// Completes the strategy composition triad (MD-28): the plan owns the
/// [`NewtonStrategy`], the [`HomotopyStrategy`] escalation, and the
/// [`StepperStrategy`] the transient driver delegates `propose_dt`/`reject_dt`
/// to. Every analysis gets a uniform strategy surface — no inline rejection
/// logic in the driver.
pub struct ConvergencePlan {
    newton: Box<dyn NewtonStrategy>,
    strategies: Vec<Box<dyn HomotopyStrategy>>,
    stepper: Box<dyn StepperStrategy>,
    limits: PlanLimits,
    schedules: Schedules,
    trace: TraceFlags,
}

impl Default for ConvergencePlan {
    /// SPICE's standard escalation: `GminStepping` first (cheap, robust), then
    /// `SourceStepping` (finds the correct solution branch where gmin stepping
    /// can settle on the wrong one — BJT/MOS amplifiers). The default stepper
    /// is the [`PiController`] (TR-BDF2 adaptive timestep, ngspice-lineage
    /// gains).
    fn default() -> Self {
        Self {
            newton: Box::new(NoDamping),
            strategies: vec![Box::new(GminStepping), Box::new(SourceStepping)],
            stepper: Box::new(PiController::default()),
            limits: PlanLimits::default(),
            schedules: Schedules::default(),
            trace: TraceFlags::default(),
        }
    }
}

impl ConvergencePlan {
    /// Build a plan from an explicit strategy list (escalation order preserved).
    pub fn new(strategies: Vec<Box<dyn HomotopyStrategy>>) -> Self {
        Self {
            newton: Box::new(NoDamping),
            strategies,
            stepper: Box::new(PiController::default()),
            limits: PlanLimits::default(),
            schedules: Schedules::default(),
            trace: TraceFlags::default(),
        }
    }

    /// Override the Newton strategy.
    pub fn with_newton(mut self, newton: Box<dyn NewtonStrategy>) -> Self {
        self.newton = newton;
        self
    }

    /// Override the transient timestep strategy (ABI-42). The transient driver
    /// delegates `propose_dt`/`reject_dt` to this strategy through the plan,
    /// instead of owning one inline.
    pub fn with_stepper(mut self, stepper: Box<dyn StepperStrategy>) -> Self {
        self.stepper = stepper;
        self
    }

    /// Override the numerical limits honored across drivers.
    pub fn with_limits(mut self, limits: PlanLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Override the homotopy schedules the strategies ramp by.
    pub fn with_schedules(mut self, schedules: Schedules) -> Self {
        self.schedules = schedules;
        self
    }

    /// Override the diagnostic trace toggles handed to the strategies
    /// (drivers seed them from [`Policy::trace`](crate::tolerances::Policy)).
    pub fn with_trace(mut self, trace: TraceFlags) -> Self {
        self.trace = trace;
        self
    }

    /// The Newton iteration policy.
    pub fn newton(&self) -> &dyn NewtonStrategy {
        self.newton.as_ref()
    }

    /// The transient timestep strategy (ABI-42). Transient drivers delegate
    /// `propose_dt`/`reject_dt` here; the plan is the single owner.
    pub fn stepper(&self) -> &dyn StepperStrategy {
        self.stepper.as_ref()
    }

    /// Mutable access to the stepper — the transient driver advances the
    /// strategy's internal state across accepted/rejected steps.
    pub fn stepper_mut(&mut self) -> &mut dyn StepperStrategy {
        self.stepper.as_mut()
    }

    /// Numerical caps every driver should honor.
    pub fn limits(&self) -> PlanLimits {
        self.limits
    }

    /// The homotopy schedules owned by this plan.
    pub fn schedules(&self) -> &Schedules {
        &self.schedules
    }

    /// Run the plan: plain Newton, then each homotopy in order. Returns the
    /// first converged solution (and which strategy found it), else the most
    /// recent failure. Each attempt is bracketed by a device-state checkpoint
    /// (ABI-07): a failed attempt restores device-internal state (limiter)
    /// before the next strategy retries from the last accepted point.
    pub fn solve(&self, driver: &mut dyn HomotopyDriver) -> Result<PlanOutcome> {
        driver.checkpoint_devices();
        let mut last = match driver.newton() {
            Ok(solution) => return Ok(PlanOutcome { solution, strategy: None }),
            Err(err) => err,
        };
        driver.restore_devices();
        for strategy in &self.strategies {
            driver.checkpoint_devices();
            match strategy.converge(driver, &self.schedules, self.trace) {
                Ok(solution) => {
                    return Ok(PlanOutcome { solution, strategy: Some(strategy.name()) });
                }
                Err(err) => last = err,
            }
            // Strategy fell through — restore device state before the next
            // attempt so its limiter starts clean (ABI-07).
            driver.restore_devices();
        }
        Err(last)
    }
}

/// What the plan converged and how — the DC driver copies the strategy name
/// into the owner's `SolverStats`.
pub struct PlanOutcome {
    pub solution: Array1<f64>,
    /// The homotopy that converged; `None` when plain Newton sufficed.
    pub strategy: Option<&'static str>,
}

/// SPICE gmin stepping: converge an easy, diagonally-dominant version of the
/// circuit (large node-to-ground conductance), then ramp that conductance to 0,
/// warm-starting each step. The standard homotopy for stiff coupled-junction
/// operating points that plain Newton oscillates on.
pub struct GminStepping;

impl HomotopyStrategy for GminStepping {
    fn name(&self) -> &'static str {
        "gmin-stepping"
    }

    fn converge(
        &self,
        driver: &mut dyn HomotopyDriver,
        schedules: &Schedules,
        trace: TraceFlags,
    ) -> Result<Array1<f64>> {
        let schedule = &schedules.gmin;
        // Ramp until the extra conductance is negligible next to the real gmin.
        let floor = driver.gmin_floor() * schedule.floor_margin;
        // Start very easy (100 mS to ground) and drop a decade per step, with
        // adaptive back-off: a step that won't converge raises the conductance
        // again (smaller decrements) instead of giving up. Bounded so a truly
        // non-convergent circuit still terminates.
        let mut g = schedule.start_g;
        let mut factor = schedule.decade_factor;
        let mut converged_any = false;
        for _ in 0..schedule.max_steps {
            driver.set_gmin_extra(g);
            let result = driver.newton();
            if trace.gmin {
                eprintln!("GMIN step g={g:.3e} -> {}", if result.is_ok() { "ok" } else { "fail" });
            }
            match result {
                Ok(_) => {
                    converged_any = true;
                    if g <= floor {
                        break;
                    }
                    factor = (factor * schedule.relax_growth).min(schedule.relax_cap); // relax faster once it's easy
                    g *= factor;
                }
                Err(err) => {
                    if !converged_any {
                        // Couldn't even converge the easiest problem — give up.
                        driver.set_gmin_extra(0.0);
                        return Err(err);
                    }
                    factor = (factor * schedule.backoff_growth).min(schedule.backoff_cap); // back off: raise g, shrink step
                    g /= factor;
                }
            }
        }
        // Final solve with the extra conductance removed — the true operating
        // point, warm-started from the last stepped solution.
        driver.set_gmin_extra(0.0);
        if trace.gmin {
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
    fn name(&self) -> &'static str {
        "source-stepping"
    }

    fn converge(
        &self,
        driver: &mut dyn HomotopyDriver,
        schedules: &Schedules,
        trace: TraceFlags,
    ) -> Result<Array1<f64>> {
        let schedule = &schedules.source;
        // A real shunt conductance (1 µS) conditions the exponential turn-on
        // knee (the BJT/MOS threshold), held through the source ramp then itself
        // ramped to 0 (a nested gmin step) so the final answer is exact.
        let knee_gmin = schedule.knee_gmin;
        let mut scale = 0.0_f64;
        let mut step = schedule.start_step;
        let mut last_ok = 0.0_f64;
        driver.set_src_scale(0.0);
        driver.set_gmin_extra(knee_gmin);
        // Solve the fully-off circuit first (trivial).
        let mut sol = driver.newton();
        for _ in 0..schedule.max_steps {
            if sol.is_ok() {
                last_ok = scale;
                if scale >= 1.0 {
                    break;
                }
                step = (step * schedule.step_growth).min(schedule.step_cap);
                scale = (last_ok + step).min(1.0);
            } else {
                // Back off toward the last converged scale.
                step *= schedule.backoff_factor;
                if step < schedule.min_step {
                    driver.set_src_scale(1.0);
                    driver.set_gmin_extra(0.0);
                    return sol; // give up with the failure
                }
                scale = last_ok + step;
            }
            driver.set_src_scale(scale);
            if trace.source {
                eprintln!("SRC step scale={scale:.4} step={step:.4}");
            }
            sol = driver.newton();
        }
        // Full source strength reached with the knee shunt still in. Ramp the
        // shunt out (a nested gmin step, warm-started) so the final answer is
        // exact.
        driver.set_src_scale(1.0);
        let mut g = knee_gmin;
        while g > driver.gmin_floor() * schedule.floor_margin {
            g *= schedule.knee_decay;
            driver.set_gmin_extra(g);
            if driver.newton().is_err() {
                break;
            }
        }
        driver.set_gmin_extra(0.0);
        driver.newton()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{Error, SolverDomain};

    /// A controller resumed from another's memory proposes what that one proposes next, bit for bit,
    /// after one accept (`prev_error` set) and after several.
    #[test]
    fn the_pi_controller_resumes_its_memory() {
        let limits = StepLimits { dt_min: 1e-15, dt_max: 1e-3 };
        let errors = [0.3, 2.5, 0.7, 1.4, 0.05, 0.9];
        let mut fresh_resumed = PiController::default();
        fresh_resumed.resume(&PiController::default().memory());
        assert_eq!(fresh_resumed.memory(), StepperMemory::default(), "no accept yet: nothing to resume");
        let mut original = PiController::default();
        for (i, lte) in errors.iter().enumerate() {
            original.propose_dt(*lte, 1e-6 * (i as f64 + 1.0), &limits);
            let mut resumed = PiController::default();
            resumed.resume(&original.memory());
            let (next_lte, dt) = (errors[(i + 1) % errors.len()] * 1.37, 2.3e-6);
            let (a, b) = (original.propose_dt(next_lte, dt, &limits), resumed.propose_dt(next_lte, dt, &limits));
            assert_eq!(a.to_bits(), b.to_bits(), "after {} accepts", i + 1);
        }
        // A controller that had accepted once differs from a fresh one: the memory is not a no-op.
        let mut warm = PiController::default();
        warm.propose_dt(0.3, 1e-6, &limits);
        let mut cold = PiController::default();
        assert_ne!(warm.propose_dt(2.0, 1e-6, &limits).to_bits(), cold.propose_dt(2.0, 1e-6, &limits).to_bits());
    }

    /// A static solve, past the first iteration, with a limit active and a 20 V node step.
    fn damp(strategy: &dyn NewtonStrategy, context: DampContext, prev: [f64; 3], new: [f64; 3]) -> [f64; 3] {
        let prev = Array1::from(prev.to_vec());
        let mut update = Array1::from(new.to_vec());
        strategy.damp_update(prev.view(), update.view_mut(), &context);
        [update[0], update[1], update[2]]
    }

    /// ngspice `niiter.c:297-323`: each of the four conditions off leaves the update; on, the
    /// node steps scale by `max(10/maxΔv, 0.1)` (maxΔv 20 V gives 0.5, 200 V the floor 0.1) and the
    /// branch current (index 2) is untouched.
    #[test]
    fn node_damping_follows_ngspice() {
        let nodes = [0usize, 1];
        let on = DampContext { static_solve: true, iteration: 1, limiting: true, nodes: &nodes };
        let (prev, new) = ([1.0, 2.0, 0.5], [21.0, 4.0, 3.0]);
        for (off, what) in [
            (DampContext { static_solve: false, ..on }, "a transient step"),
            (DampContext { iteration: 0, ..on }, "the first iteration"),
            (DampContext { limiting: false, ..on }, "no limit active"),
        ] {
            assert_eq!(damp(&NodeDamping, off, prev, new), new, "{what} leaves the update");
        }
        assert_eq!(damp(&NodeDamping, on, prev, [10.9, 4.0, 3.0]), [10.9, 4.0, 3.0], "maxΔv 9.9 V is not above 10 V");
        let at_20 = damp(&NodeDamping, on, prev, new);
        assert_eq!(at_20, [11.0, 3.0, 3.0], "maxΔv 20 V: factor 0.5 on the nodes, the branch untouched");
        let at_200 = damp(&NodeDamping, on, prev, [201.0, 4.0, 3.0]);
        assert!((at_200[0] - 21.0).abs() < 1e-12 && (at_200[1] - 2.2).abs() < 1e-12, "maxΔv 200 V: factor 0.1, got {at_200:?}");
        assert_eq!(at_200[2], 3.0);
    }

    /// The default strategy leaves every update alone, whatever the context.
    #[test]
    fn no_damping_leaves_the_update() {
        let nodes = [0usize, 1];
        let on = DampContext { static_solve: true, iteration: 5, limiting: true, nodes: &nodes };
        let new = [201.0, 4.0, 3.0];
        assert_eq!(damp(&NoDamping, on, [1.0, 2.0, 0.5], new), new);
        let plan = ConvergencePlan::default();
        let mut update = Array1::from(new.to_vec());
        plan.newton().damp_update(Array1::from(vec![1.0, 2.0, 0.5]).view(), update.view_mut(), &on);
        assert_eq!(update.to_vec(), new.to_vec(), "the default plan damps nothing");
    }

    /// Scripted homotopy driver: records every `newton()` call as the
    /// `(src_scale, gmin_extra)` it ran under — the warm-start chain — and
    /// fails when the scripted predicate says so.
    struct ScriptedDriver {
        src_scale: f64,
        gmin_extra: f64,
        calls: Vec<(f64, f64)>,
        fail: Box<dyn Fn(f64, usize) -> bool>,
    }

    impl ScriptedDriver {
        fn new(fail: impl Fn(f64, usize) -> bool + 'static) -> Self {
            Self { src_scale: 1.0, gmin_extra: 0.0, calls: Vec::new(), fail: Box::new(fail) }
        }
    }

    impl HomotopyDriver for ScriptedDriver {
        fn newton(&mut self) -> Result<Array1<f64>> {
            let n = self.calls.len();
            self.calls.push((self.src_scale, self.gmin_extra));
            if (self.fail)(self.src_scale, n) {
                Err(Error::simple(SolverDomain::Dc, "scripted non-convergence"))
            } else {
                Ok(Array1::zeros(1))
            }
        }
        fn set_gmin_extra(&mut self, g: f64) {
            self.gmin_extra = g;
        }
        fn set_src_scale(&mut self, s: f64) {
            self.src_scale = s;
        }
        fn gmin_floor(&self) -> f64 {
            1e-12
        }
    }

    /// cktop.c ramp semantics: the schedule starts fully off (scale 0),
    /// raises the scale monotonically to full strength, holds the knee shunt
    /// through the ramp, then ramps the shunt out — the last solve is the
    /// exact problem (scale 1, no extra conductance).
    #[test]
    fn source_stepping_ramp_schedule() {
        let mut driver = ScriptedDriver::new(|_, _| false);
        SourceStepping.converge(&mut driver, &Schedules::default(), TraceFlags::default()).expect("all-converging driver must succeed");

        let calls = &driver.calls;
        assert_eq!(calls[0], (0.0, 1e-6), "ramp starts fully off, knee shunt in");
        // Phase 1: the source ramp, under the constant knee shunt.
        let ramp: Vec<f64> = calls.iter().take_while(|(_, g)| *g == 1e-6).map(|(s, _)| *s).collect();
        assert!(ramp.windows(2).all(|w| w[0] <= w[1]), "ramp is monotone: {ramp:?}");
        assert_eq!(*ramp.last().unwrap(), 1.0, "ramp reaches full strength");
        // Phase 2: the knee shunt itself is ramped out at full source strength.
        let tail = &calls[ramp.len()..];
        assert!(
            tail.iter().all(|&(s, g)| s == 1.0 && g < 1e-6),
            "knee ramp-out runs at full source strength: {tail:?}"
        );
        assert_eq!(*calls.last().unwrap(), (1.0, 0.0), "final solve is the exact problem");
    }

    /// Back-off: a step that fails is retried closer to the last converged
    /// scale (warm start preserved), and the ramp still reaches 1.0 once the
    /// hard region is passable.
    #[test]
    fn source_stepping_backs_off_after_failure() {
        // A transient hard region: scales above 0.55 fail for the first 6
        // newton calls, then converge.
        let mut driver = ScriptedDriver::new(|scale, n| scale > 0.55 && n < 6);
        SourceStepping.converge(&mut driver, &Schedules::default(), TraceFlags::default()).expect("transient hard region must be passable");

        let calls = &driver.calls;
        let first_fail = calls
            .iter()
            .position(|&(s, _)| s > 0.55)
            .expect("the schedule must have attempted the hard region");
        assert!(
            calls[first_fail + 1].0 < calls[first_fail].0,
            "a failed step must back off toward the last converged scale: {calls:?}"
        );
        assert_eq!(*calls.last().unwrap(), (1.0, 0.0), "still reaches the exact problem");
    }

    /// A permanently impassable region exhausts the back-off (step < 1e-6)
    /// and gives up with the failure, leaving the driver restored to the
    /// exact problem (scale 1, no shunt).
    #[test]
    fn source_stepping_gives_up_when_backoff_exhausts() {
        let mut driver = ScriptedDriver::new(|scale, _| scale > 0.5);
        let result = SourceStepping.converge(&mut driver, &Schedules::default(), TraceFlags::default());
        assert!(result.is_err(), "impassable ramp must report non-convergence");
        assert_eq!(driver.src_scale, 1.0, "source scale restored");
        assert_eq!(driver.gmin_extra, 0.0, "shunt removed");
    }

    /// Warm-start reuse: every solve in the plan runs on the *same* driver
    /// state — successful scales form one non-decreasing chain (no restart
    /// from scratch after a converged step).
    #[test]
    fn source_stepping_warm_start_chain() {
        let mut driver = ScriptedDriver::new(|scale, n| scale > 0.55 && n < 6);
        SourceStepping.converge(&mut driver, &Schedules::default(), TraceFlags::default()).unwrap();
        let ramp_ok: Vec<f64> = driver
            .calls
            .iter()
            .enumerate()
            .filter(|&(n, &(s, _))| !(s > 0.55 && n < 6))
            .take_while(|&(_, &(_, g))| g > 0.0)
            .map(|(_, &(s, _))| s)
            .collect();
        assert!(
            ramp_ok.windows(2).all(|w| w[0] <= w[1]),
            "converged scales must form one warm-started chain: {ramp_ok:?}"
        );
    }
}
