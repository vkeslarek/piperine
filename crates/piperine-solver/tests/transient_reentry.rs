//! SC-04 enabler — transient full-state re-entry: an RC charge run 0→T,
//! captured at T, re-entered T→2T equals a single 0→2T run within the
//! solver's tolerance class. This is the PSS shooting seam
//! (`TransientSolver::with_initial_state`).

// Each target includes the library by path: it uses a different subset of it.
#[allow(dead_code)]
#[path = "helpers/devices.rs"]
mod devices;

use devices::{NodeMap, C, R, Vsrc};
use piperine_solver::abi::InitialValue;
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions};

/// `v(out) = 0` initial condition (discharged capacitor) so the runs show
/// the charge transient instead of the DC steady state.
fn discharged_ic(
    info: &NodeMap,
    _circuit: &CircuitInstance,
) -> Vec<InitialValue<piperine_solver::abi::AnalogReference, f64>> {
    let reference = info.get("out").clone();
    vec![InitialValue { reference, value: 0.0 }]
}

/// The RC charge circuit: 5 V source, 1 kOhm, 1 uF to ground.
fn build_rc() -> (CircuitInstance, NodeMap) {
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    b.element(Box::new(Vsrc::new("v1", 5.0, top.clone(), gnd.clone())));
    b.element(Box::new(R::new("r1", 1.0e3, top, out.clone())));
    b.element(Box::new(C::new("c1", 1.0e-6, out.clone(), gnd)));
    let circuit = b.build().expect("build");
    let mut map = NodeMap::new();
    map.insert("out", out);
    (circuit, map)
}

/// τ = RC = 1 ms; T = 1 ms. Continuous 0→2T vs captured-at-T re-entry T→2T:
/// endpoints agree within the solver tolerance class (1e-9 + 1e-3·|v|), and
/// both sit near the analytic 5·(1−e^(−t/τ)).
#[test]
fn reentry_from_captured_state_matches_continuous_run() {
    let t_half = 1.0e-3;
    let read_out = |step: &piperine_solver::prelude::TransientStep, info: &NodeMap| {
        step.get_node(&info.id("out")).expect("v(out)")
    };

    // Reference: one continuous 0→2T run.
    let (mut circuit_a, info_a) = build_rc();
    let opts_a = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0);
    let ic_a = discharged_ic(&info_a, &circuit_a);
    let mut solver_a = circuit_a.transient(opts_a, Context::default()).expect("tran A");
    solver_a.apply_initial_conditions(ic_a);
    let full = solver_a.solve().expect("solve A");
    let v_full_end = read_out(full.last().expect("A has steps"), &info_a);

    // Leg 1: 0→T, capture the final step.
    let (mut circuit_b, info_b) = build_rc();
    let ic_b = discharged_ic(&info_b, &circuit_b);
    let mut solver_b1 = circuit_b
        .transient(TransientAnalysisOptions::new(t_half, t_half / 100.0), Context::default())
        .expect("tran B1");
    solver_b1.apply_initial_conditions(ic_b);
    let leg1 = solver_b1.solve().expect("solve B1");
    let captured = leg1.last().expect("B1 has steps").clone();
    let v_captured = read_out(&captured, &info_b);
    let analytic_half = 5.0 * (1.0 - (-1.0_f64).exp());
    assert!(
        (v_captured - analytic_half).abs() < 5.0e-3,
        "capture point sanity: v(T)={v_captured} vs analytic {analytic_half}"
    );

    // Leg 2: re-enter from the captured state, T→2T on the same circuit.
    let opts_b2 = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0).with_start(t_half);
    let mut solver_b2 = circuit_b.transient(opts_b2, Context::default()).expect("tran B2");
    solver_b2.with_initial_state(&captured);
    let leg2 = solver_b2.solve().expect("solve B2");
    let first_b2 = leg2.iter().next().expect("B2 has steps");
    assert!(
        (read_out(first_b2, &info_b) - v_captured).abs() < 1e-12,
        "re-entry starts exactly at the captured state"
    );
    let v_reentry_end = read_out(leg2.last().expect("B2 has steps"), &info_b);

    let tol = 1e-9 + 1e-3 * v_full_end.abs();
    assert!(
        (v_reentry_end - v_full_end).abs() <= tol,
        "re-entered end v={v_reentry_end} vs continuous v={v_full_end} (tol {tol})"
    );
    let analytic_end = 5.0 * (1.0 - (-2.0_f64).exp());
    assert!(
        (v_reentry_end - analytic_end).abs() < 1.0e-2,
        "analytic sanity: v(2T)={v_reentry_end} vs {analytic_end}"
    );
}
