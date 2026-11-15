//! LRM-COVERS: 8.4.7
//! Rollback lifecycle contract tests (ABI-01..08) on the `Device` trait: the
//! solver owns every checkpoint (limit iterates and term histories), so a
//! rejected attempt leaves a device exactly where its last acceptance did.
//!
//! - T2: a transient that is forced through LTE rejections follows the
//!   analytic response: a rejected attempt left no trace in a history.
//! - T3: a DC homotopy fallthrough reaches the operating point of the solve
//!   that never fell through: the failed plain-Newton attempt left no trace
//!   in a limit iterate.
//!
//! The trait defaults of the removed `checkpoint_state`/`restore_state` hooks,
//! the `SUPPORTS_ROLLBACK` flag and `limiting_report` are gone with them (see
//! `removed.txt`).
//!
//! lrm-conformance-wave3 (Phase 6, T39): §8.4.7 point 2c ("Having
//! calculated the solution for a given time, the analog engine can
//! either accept or reject that solution; it cannot calculate a solution
//! for a future time until it has accepted the solution for the current
//! time") is exactly this file's own subject —
//! `transient_reject_drives_restore_accept_discards` (T2) proves the
//! reject-then-restore half directly; the solver's own transient loop
//! never advances past a rejected step's time, satisfying the second half
//! structurally.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{add_capacitor, add_diode, add_resistor, add_vsrc, node_id, vsine};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Policy, Solver, TransientAnalysisOptions};

/// Spec ABI-02/03/08: the sine-driven RC low-pass naturally overshoots its
/// LTE budget on roughly two dozen of its early steps, exercising the reject
/// path. Every accepted point lies on the analytic response of
/// `τ·dv/dt + v = sin(ωt)`, so no rejected attempt corrupted a history.
#[test]
fn transient_reject_drives_restore_accept_discards() {
    let (r, c, freq) = (1000.0_f64, 1e-7_f64, 1000.0_f64);
    let mut b = CircuitBuilder::new("rc-reject");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    b.device(
        vsine(),
        "vsin",
        &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(freq))],
        &[Port::Node(top.clone()), Port::Node(gnd.clone())],
    );
    add_resistor(&mut b, "r1", r, &top, &mid);
    add_capacitor(&mut b, "c1", c, &mid, &gnd);
    let circuit = b.build().expect("build");

    // The sine-driven RC low-pass naturally overshoots its LTE budget on
    // ~26 of its first steps under default tolerances, driving the reject
    // path without an artificial tolerance clamp (which would loop forever).
    let opts = TransientAnalysisOptions::new(1e-4, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    // ABI-02: at least one attempt was rejected and rolled back.
    assert!(res.stats.steps_rejected > 0, "expected rejections to drive the rollback");
    assert!(res.stats.steps_accepted > 0);

    // ABI-01/08: what was accepted is the response of a rolled-back state.
    let (tau, w) = (r * c, 2.0 * std::f64::consts::PI * freq);
    let wt = w * tau;
    let analytic = |t: f64| (((w * t).sin() - wt * (w * t).cos()) + wt * (-t / tau).exp()) / (1.0 + wt * wt);
    let mut worst = 0.0_f64;
    for step in res.iter() {
        let v = step.get_node(&node_id(&mid)).unwrap();
        worst = worst.max((v - analytic(step.time())).abs());
    }
    assert!(worst < 5e-3, "worst deviation from the analytic response {worst}");
}

// ── T3: DC homotopy retry ─────────────────────────────────────────────────
//
// Spec ABI-07: when a DC homotopy strategy falls through (failed attempt →
// next strategy), the retry starts from the pre-attempt state. A stiff diode
// whose plain-Newton attempt is starved of iterations (max_iter=4) falls
// through to gmin stepping, which converges.

fn diode_circuit() -> (piperine_solver::prelude::CircuitInstance, piperine_solver::abi::AnalogReference) {
    let mut b = CircuitBuilder::new("diode-homotopy");
    let gnd = b.ground();
    let src = b.node("src");
    let anode = b.node("anode");
    add_vsrc(&mut b, "v1", 1.0, &src, &gnd);
    add_resistor(&mut b, "r1", 1000.0, &src, &anode);
    add_diode(&mut b, "d1", 1e-14, &anode, &gnd);
    (b.build().expect("build"), anode)
}

/// Spec ABI-07: plain Newton fails on the starved stiff diode and falls
/// through to gmin stepping, which converges — to the operating point the
/// unstarved solve reaches, so the failed attempt left no limit iterate behind.
#[test]
fn dc_homotopy_fallthrough_drives_restore_between_strategies() {
    let (circuit, anode) = diode_circuit();
    // max_iter=4 starves plain Newton on the stiff diode so it falls through
    // to gmin stepping (which converges from the easy high-gmin start).
    let policy = Policy { max_iter: 4, ..Policy::default() };
    let mut solver = Solver::new(circuit).with_policy(policy).build();
    let res = solver.dc().unwrap().solve().unwrap();

    // Plain Newton fell through to gmin stepping (the homotopy took over).
    assert_eq!(
        res.stats.homotopy_strategy.as_deref(),
        Some("gmin-stepping"),
        "plain Newton must fall through so the homotopy restore fires"
    );
    let starved = res.get_node(&node_id(&anode)).unwrap();

    let (circuit, anode) = diode_circuit();
    let mut solver = Solver::new(circuit).build();
    let plain = solver.dc().unwrap().solve().unwrap().get_node(&node_id(&anode)).unwrap();
    assert!((starved - plain).abs() < 1e-4, "after the fallthrough {starved} V vs the plain solve {plain} V");
}
