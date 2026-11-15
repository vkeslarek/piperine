//! Cross-check the adjoint against `.sens` (dv-gradients T9, DVG-09/10):
//! the oracle test that makes DC-first (D6) worth it. The adjoint and
//! `.sens` compute the same physical quantity on a differentiable circuit
//! — `∂(output)/∂(param)` at the operating point — by totally different
//! routes (one transpose solve vs central finite differences). Their
//! agreement, within the finite difference's own error tolerance, is the
//! defensible differentiator the spec bets on.
//!
//! Fixtures cover three regimes:
//!
//! - **Linear divider** — a fully analytic reference (`∂v_mid/∂r2`,
//!   `∂v_mid/∂v1.dc`).
//! - **Nonlinear diode** — the regime where a wrong Jacobian or wrong
//!   ∂F/∂p produces a confidently wrong number; both engines must agree
//!   within the FD's step-error.
//! - **Solve-count re-verification** — the 20-parameter proof from T8,
//!   checked end-to-end through the same driver the value comparison ran
//!   through.
//!
//! Comparison is on VALUES, never on "both ran without error".

// Each target includes the library by path: it uses a different subset of it.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{divider_fixture, net_for, NodeMap};
use piperine_solver::prelude::{AdjointOptions, CircuitInstance, Context, SensAnalysisOptions};

fn build(top: &str) -> (CircuitInstance, NodeMap) {
    divider_fixture(top)
}

/// Relative tolerance scaled to the value's magnitude — both engines agree
/// to well within the FD's `O(dp²)` truncation plus its condition error.
/// The defaults are tight on linear circuits (where the adjoint is exact
/// and `.sens`'s error is just FD truncation) and looser on the diode
/// (where the FD step rides an exponential).
const LINEAR_REL: f64 = 1.0e-3;
const NONLINEAR_REL: f64 = 5.0e-2;

// ── DVG-09: linear divider — adjoint vs `.sens` on values ──────────────────
//
// Two parameters, both engines, one fixture. The adjoint's value is
// analytic on a linear circuit; `.sens`'s value carries central-FD
// truncation error of `O(dp²·f''')`. With `dp_rel = 1e-6` and a linear
// `f`, the truncation is zero in theory and ~`1e-12 · condition` in
// practice — `1e-3` relative agreement is a defensible tight bar.
#[test]
fn adjoint_matches_sens_on_the_linear_divider() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");
    let params = vec![("r2".to_string(), "r".to_string()), ("v1".to_string(), "dc".to_string())];

    let adj = circuit
        .adjoint(AdjointOptions::new(mid.clone(), params.clone()), Context::default())
        .expect("adjoint")
        .solve()
        .expect("adjoint solve");

    let fd = circuit
        .sens(SensAnalysisOptions::new(vec![mid.clone()], params.clone()), Context::default())
        .expect("sens")
        .solve()
        .expect("sens solve");

    for (label, param) in &params {
        let a = adj.get(mid.label(), label, param).unwrap_or_else(|| {
            panic!("adjoint missing ({label}.{param})")
        });
        let s = fd.get(mid.label(), label, param).unwrap_or_else(|| {
            panic!(".sens missing ({label}.{param})")
        });
        let denom = a.abs().max(s.abs());
        assert!(
            denom > 0.0 && ((a - s) / denom).abs() < LINEAR_REL,
            "({label}.{param}): adjoint {a} vs .sens {s} disagree beyond {LINEAR_REL}"
        );
    }
}

// ── DVG-09: RC fixture — both engines agree the DC gradient is zero ────────
//
// The capacitor is an open at DC; V(mid) = V·r1/(r1+r1) (here r1=r2). The
// gradient of V(mid) w.r.t. `c1.c` is zero — a legitimate zero, never
// "not differentiable". Both engines must return approximately zero
// (within FD noise). This is the spec's edge case in action.
#[test]
fn adjoint_and_sens_agree_on_zero_for_a_dc_invisible_parameter() {
    let (mut circuit, info) = build("RcTop");
    let mid = net_for(&circuit, &info, "mid");
    let params = vec![("c1".to_string(), "c".to_string())];

    let adj = circuit
        .adjoint(AdjointOptions::new(mid.clone(), params.clone()), Context::default())
        .expect("adjoint")
        .solve()
        .expect("adjoint solve");

    let fd = circuit
        .sens(SensAnalysisOptions::new(vec![mid.clone()], params.clone()), Context::default())
        .expect("sens")
        .solve()
        .expect("sens solve");

    let a = adj.get(mid.label(), "c1", "c").expect("adjoint entry");
    let s = fd.get(mid.label(), "c1", "c").expect(".sens entry");
    assert!(
        a.abs() < 1.0e-6,
        "adjoint ∂v(mid)/∂c1.c ≈ 0 (capacitor is open at DC): got {a}"
    );
    assert!(
        s.abs() < 1.0e-3,
        ".sens ∂v(mid)/∂c1.c ≈ 0 (capacitor is open at DC): got {s}"
    );
}

// ── DVG-09: nonlinear diode — the regime that catches a wrong gradient ────
//
// The diode's residual is exponential in V(mid); a wrong Jacobian or a
// sign error in ∂F/∂p produces a confidently wrong gradient that linear
// fixtures miss. Both engines must agree within the FD's own step error
// on this nonlinear circuit, and the sign must be physical (raising `r1`
// lowers V(mid)).
#[test]
fn adjoint_matches_sens_on_a_nonlinear_diode_circuit() {
    let (mut circuit, info) = build("DiodeTop");
    let mid = net_for(&circuit, &info, "mid");
    let params = vec![
        ("r1".to_string(), "r".to_string()),
        ("d1".to_string(), "is_sat".to_string()),
        ("v1".to_string(), "dc".to_string()),
    ];

    let adj = circuit
        .adjoint(AdjointOptions::new(mid.clone(), params.clone()), Context::default())
        .expect("adjoint")
        .solve()
        .expect("adjoint solve");

    let fd = circuit
        .sens(SensAnalysisOptions::new(vec![mid.clone()], params.clone()), Context::default())
        .expect("sens")
        .solve()
        .expect("sens solve");

    for (label, param) in &params {
        let a = adj.get(mid.label(), label, param).unwrap_or_else(|| {
            panic!("adjoint missing ({label}.{param})")
        });
        let s = fd.get(mid.label(), label, param).unwrap_or_else(|| {
            panic!(".sens missing ({label}.{param})")
        });
        // Both must have the same physical sign.
        assert!(
            a.signum() == s.signum() || a.abs() < 1.0e-9,
            "({label}.{param}): signs disagree — adjoint {a}, .sens {s}"
        );
        let denom = a.abs().max(s.abs());
        assert!(
            denom > 0.0 && ((a - s) / denom).abs() < NONLINEAR_REL,
            "({label}.{param}): adjoint {a} vs .sens {s} disagree beyond {NONLINEAR_REL}"
        );
    }

    // Spot-check the physical sign on r1: raising it must lower V(mid).
    let d_r1 = adj.get(mid.label(), "r1", "r").expect("∂v(mid)/∂r1.r");
    assert!(d_r1 < 0.0, "raising r1 must lower the diode node: {d_r1}");
}

// ── DVG-10: the solve-count assertion, end to end ──────────────────────────
//
// The whole point of the adjoint. A 20-parameter gradient costs exactly
// one transpose solve. Re-verified here through the same driver the value
// comparison ran through, so the economy and the correctness land in one
// suite.
#[test]
fn a_20_parameter_gradient_costs_exactly_one_extra_solve() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");

    let mut params = Vec::with_capacity(20);
    for i in 0..20 {
        let p = match i % 3 {
            0 => ("r1", "r"),
            1 => ("r2", "r"),
            _ => ("v1", "dc"),
        };
        params.push((p.0.to_string(), p.1.to_string()));
    }

    let result = circuit
        .adjoint(AdjointOptions::new(mid, params), Context::default())
        .expect("adjoint")
        .solve()
        .expect("solve");

    assert_eq!(
        result.transpose_solves, 1,
        "the solve-count proof from T8 holds end-to-end through this suite"
    );
}
