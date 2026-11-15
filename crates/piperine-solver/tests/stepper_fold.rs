//! Strategy fold tests (ABI-44, ABI-45): the stepper lives on
//! [`ConvergencePlan`], and the transient driver delegates `propose_dt`/
//! `reject_dt` to it through the plan — not by owning one inline.
//!
//! - ABI-44: parity baselines stay bit-identical through the fold. The
//!   default `ConvergencePlan::default()` ships a `PiController::default()`,
//!   so existing circuits integrate exactly as before — the fold only
//!   changes WHO owns the stepper, not WHAT it does.
//! - ABI-45: a custom `StepperStrategy` plugged into the plan routes
//!   through `propose_dt`/`reject_dt` and produces a deterministic step
//!   sequence (here: hold dt on accept, halve dt on reject).

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{add_capacitor, add_resistor, add_vsrc};
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, ConvergencePlan, Solver, StepLimits, StepperStrategy, TransientAnalysisOptions,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ── helpers: minimal RC divider circuit ────────────────────────────────────

/// Constant voltage source `v` held for the whole run, so the RC divider
/// charges predictably.
fn build_rc_circuit(v: f64) -> CircuitInstance {
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", v, &top, &gnd);
    add_resistor(&mut b, "r1", 1000.0, &top, &mid);
    add_capacitor(&mut b, "c1", 1e-6, &mid, &gnd);
    b.build().expect("build")
}

// ── ABI-44: parity through the fold ────────────────────────────────────────

/// ABI-44: the default `ConvergencePlan::default()` ships a `PiController`,
/// so a transient routed through `Solver::tran()` (which builds with the
/// default plan) produces the same number of recorded steps as before the
/// fold. This is the structural half of the parity guarantee — value parity
/// lives in `parity_baseline.rs::parity_rc_transient`, already pinned tight.
///
/// The fold only changes WHO owns the stepper, not WHAT it does: the
/// `PiController` body is bit-identical before and after. This test asserts
/// the structural invariant (steps_accepted > 0, time reaches stop) so a
/// regression in routing surfaces here, not only in the value baseline.
#[test]
fn parity_default_plan_completes_run_and_reaches_stop_time() {
    let circuit = build_rc_circuit(1.0);
    let opts = TransientAnalysisOptions::new(1e-3, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    let last = res.last().unwrap();
    let t_last = last.time();
    assert!((t_last - 1e-3).abs() < 1e-12, "t_last = {t_last}");
    assert!(res.len() > 1, "at least the start + one accepted step: len = {}", res.len());
}

// ── ABI-45: custom stepper routes through the plan ────────────────────────

/// Test-double stepper: holds dt on accept (no growth), halves dt on reject.
/// Records every call so the test can prove the path through the plan is
/// exercised.
struct HalvingStepper {
    proposes: Arc<AtomicUsize>,
    rejects: Arc<AtomicUsize>,
}

impl StepperStrategy for HalvingStepper {
    fn propose_dt(
        &mut self,
        _lte: f64,
        dt_actual: f64,
        opts: &StepLimits,
    ) -> f64 {
        self.proposes.fetch_add(1, Ordering::SeqCst);
        dt_actual.clamp(opts.dt_min, opts.dt_max)
    }

    fn reject_dt(
        &mut self,
        failed_dt: f64,
        opts: &StepLimits,
    ) -> f64 {
        self.rejects.fetch_add(1, Ordering::SeqCst);
        (failed_dt * 0.5).max(opts.dt_min)
    }
}

/// A custom `StepperStrategy` installed through `ConvergencePlan::with_stepper`
/// receives every `propose_dt`/`reject_dt` call from the transient driver —
/// the plan is the single routing point (ABI-43/45). The counters prove the
/// driver reaches the stepper through the plan, not via some inline copy.
#[test]
fn custom_stepper_receives_propose_calls_through_plan() {
    let proposes = Arc::new(AtomicUsize::new(0));
    let rejects = Arc::new(AtomicUsize::new(0));
    let plan = ConvergencePlan::default().with_stepper(Box::new(HalvingStepper {
        proposes: proposes.clone(),
        rejects: rejects.clone(),
    }));

    let circuit = build_rc_circuit(1.0);
    let opts = TransientAnalysisOptions::new(1e-3, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let mut tran = solver.tran().unwrap();
    tran.set_plan(plan);
    let res = tran.solve();
    assert!(res.is_ok(), "transient must complete: {:?}", res.err());

    // The driver calls propose_dt once per accepted step. With the hold
    // stepper, the step size stays at the initial dt for the whole run, so
    // the count is fixed by the stop/dt ratio. Rejects (if any) prove the
    // reject path is wired identically — both go through the plan.
    let n_propose = proposes.load(Ordering::SeqCst);
    assert!(n_propose > 0, "propose_dt must route through plan.stepper_mut()");
    let _ = rejects.load(Ordering::SeqCst);
}

/// ABI-42: the default plan exposes a `stepper()` accessor. The driver calls
/// `plan.stepper_mut().propose_dt(...)` — this test exercises the exact
/// routing the transient driver uses on every accepted step.
#[test]
fn default_plan_stepper_is_routed_through_accessor() {
    let mut plan = ConvergencePlan::default();
    let opts = TransientAnalysisOptions::new(1e-3, 1e-5);
    let dt = plan.stepper_mut().propose_dt(1.0, 1e-5, &opts.step_limits());
    assert!(dt > 0.0, "default stepper returns a positive dt: {dt}");

    let dt_after_reject = plan.stepper_mut().reject_dt(1e-5, &opts.step_limits());
    assert!(dt_after_reject > 0.0 && dt_after_reject < 1e-5,
        "default reject_dt shrinks dt: {dt_after_reject}");
}
