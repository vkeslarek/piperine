//! SC-01/SC-02 — `.sens` DC sensitivity: analytic divider references,
//! finite-difference independence on a nonlinear (diode) circuit, and the
//! loud-error paths (unknown element/param, non-analog output).

// Each target includes the library by path: it uses a different subset of it.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{divider_fixture, net_for, NodeMap};
use piperine_solver::prelude::{CircuitInstance, Context, Net, SensAnalysisOptions};

fn build(top: &str) -> (CircuitInstance, NodeMap) {
    divider_fixture(top)
}

/// Divider analytics: `v_mid = V·r2/(r1+r2)` →
/// `∂v_mid/∂r2 = V·r1/(r1+r2)² = 2.5e-3 V/Ω` and `∂v_mid/∂v1.dc = 0.5`.
#[test]
fn divider_sensitivities_match_analytic_values() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");
    let opts = SensAnalysisOptions::new(
        vec![mid.clone()],
        vec![("r2".into(), "r".into()), ("v1".into(), "dc".into())],
    );
    let result = circuit.sens(opts, Context::default()).expect("sens").solve().expect("solve");

    let d_r2 = result.get(mid.label(), "r2", "r").expect("d(mid)/d(r2.r)");
    let analytic = 10.0 * 1.0e3 / (2.0e3_f64).powi(2); // 2.5e-3
    assert!(
        ((d_r2 - analytic) / analytic).abs() < 1.0e-6,
        "d v(mid)/d r2.r = {d_r2} vs analytic {analytic}"
    );

    let d_v = result.get(mid.label(), "v1", "dc").expect("d(mid)/d(v1.dc)");
    assert!(((d_v - 0.5) / 0.5).abs() < 1.0e-6, "d v(mid)/d v1.dc = {d_v} vs 0.5");
}

/// Nonlinear (diode) circuit: the sensitivity is step-size independent —
/// `dp_rel = 1e-6` and `dp_rel = 1e-4` agree within 1e-3 relative — and
/// has the physical sign (raising `r1` drops the diode voltage).
#[test]
fn diode_sensitivity_is_step_independent_and_signed() {
    let (mut circuit, info) = build("DiodeTop");
    let mid = net_for(&circuit, &info, "mid");

    let mut opts = SensAnalysisOptions::new(vec![mid.clone()], vec![("r1".into(), "r".into())]);
    let fine =
        circuit.sens(opts.clone(), Context::default()).expect("sens").solve().expect("solve");
    opts.dp_rel = 1.0e-4;
    let coarse = circuit.sens(opts, Context::default()).expect("sens").solve().expect("solve");

    let a = fine.get(mid.label(), "r1", "r").expect("fine");
    let b = coarse.get(mid.label(), "r1", "r").expect("coarse");
    assert!(a < 0.0, "raising r1 must lower the diode node: {a}");
    assert!(((a - b) / a).abs() < 1.0e-3, "step independence: {a} vs {b}");
}

/// Loud errors: unknown element, unknown parameter (naming the available
/// ones), a Rebuild-class (presence-flipping) parameter, and a
/// digital/pseudo output net.
#[test]
fn sens_error_paths_are_loud() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");

    let err = circuit
        .sens(
            SensAnalysisOptions::new(vec![mid.clone()], vec![("nope".into(), "r".into())]),
            Context::default(),
        )
        .expect("sens")
        .solve()
        .expect_err("unknown element");
    assert!(err.to_string().contains("nope"), "names the element: {err}");

    let err = circuit
        .sens(
            SensAnalysisOptions::new(vec![mid], vec![("r2".into(), "bogus".into())]),
            Context::default(),
        )
        .expect("sens")
        .solve()
        .expect_err("unknown param");
    assert!(err.to_string().contains("bogus"), "names the param: {err}");

    // Non-addressable output: a digital net is not a solved analog net — loud.
    let digital = Net::digital(0, "d0");
    let err = circuit
        .sens(
            SensAnalysisOptions::new(vec![digital], vec![("r2".into(), "r".into())]),
            Context::default(),
        )
        .expect("sens")
        .solve()
        .expect_err("digital output");
    assert!(
        err.to_string().contains("not a solved analog net"),
        "names the output class: {err}"
    );
}

/// SC-02 — a presence-queried optional param that was never given is
/// Rebuild-class (the given-mask is baked at build): a finite difference
/// across a rebuild boundary is not a sensitivity, so the analysis fails
/// loud naming the param.
#[test]
fn sens_rebuild_class_param_fails_loud() {
    let (mut circuit, info) = build("OptTop");
    let mid = net_for(&circuit, &info, "mid");

    let err = circuit
        .sens(
            SensAnalysisOptions::new(vec![mid], vec![("r1".into(), "w".into())]),
            Context::default(),
        )
        .expect("sens")
        .solve()
        .expect_err("Rebuild-class param");
    let msg = err.to_string();
    assert!(msg.contains("w"), "names the param: {msg}");
    assert!(msg.contains("Rebuild"), "names the invalidation class: {msg}");

    // The given-at-build scalar param stays Restamp-class and solves fine.
    let opts = SensAnalysisOptions::new(
        vec![net_for(&circuit, &info, "mid")],
        vec![("r2".into(), "r".into())],
    );
    circuit.sens(opts, Context::default()).expect("sens").solve().expect("restampable param solves");
}
