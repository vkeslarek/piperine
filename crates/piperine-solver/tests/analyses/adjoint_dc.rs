//! The DC adjoint driver (dv-gradients T8, DVG-08/10): one transpose solve
//! `Jᵀλ = ∂f/∂x` at the converged operating point combines with each
//! parameter's `∂F/∂p` to yield `∂f/∂p = −λᵀ·∂F/∂p` for **every**
//! requested parameter from that one solve. The economic heart of the
//! adjoint: `.sens`'s two re-solves per parameter collapse to one extra
//! linear solve, regardless of count — asserted here, not claimed.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::circuits::NodeMap;
use piperine_solver::prelude::{AdjointOptions, CircuitInstance, Context};

// ── DVG-10: one transpose solve serves every requested parameter ──────────
//
// The economic heart of the adjoint. A 20-parameter request costs exactly
// ONE transpose solve — not one per parameter. The count is asserted on
// the result, not inspected by eye or claimed in a comment.
#[test]
fn one_transpose_solve_serves_every_requested_parameter() {
    let (mut circuit, info) = AdjointDcCircuit::build("Top");
    let mid = CircuitBuilding::net_for(&circuit, &info, "mid");

    // 20 parameter requests over the divider's three differentiable params.
    // The driver processes each (label, param) pair independently through
    // the SAME λ — the count must stay at one regardless.
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
        .adjoint(AdjointOptions::new(mid.clone(), params), Context::default())
        .expect("adjoint")
        .solve()
        .expect("solve");

    assert_eq!(
        result.transpose_solves, 1,
        "20 parameters → exactly one transpose solve (got {})",
        result.transpose_solves,
    );
}

// ── DVG-08: ∂f/∂x is built for a scalar output from the converged DC ───────
//
// On the linear divider, `v_mid = V·r2/(r1+r2)`. The analytic gradients
// are `∂v_mid/∂r2 = V·r1/(r1+r2)² = 2.5e-3 V/Ω` and `∂v_mid/∂v1.dc = 0.5`.
// Verifying these asserts the adjoint assembled ∂f/∂x correctly and the
// transpose solve produced a meaningful λ — a wrong indexing or sign on
// either half surfaces as a wrong value here.
#[test]
fn df_dx_is_built_for_a_scalar_output_from_the_converged_dc_solution() {
    let (mut circuit, info) = AdjointDcCircuit::build("Top");
    let mid = CircuitBuilding::net_for(&circuit, &info, "mid");

    let params = vec![("r2".into(), "r".into()), ("v1".into(), "dc".into())];
    let result = circuit
        .adjoint(AdjointOptions::new(mid.clone(), params), Context::default())
        .expect("adjoint")
        .solve()
        .expect("solve");

    let analytic_d_r2 = 10.0 * 1.0e3 / (2.0e3_f64).powi(2); // 2.5e-3
    let got_d_r2 = result.get(mid.label(), "r2", "r").expect("∂v_mid/∂r2.r");
    assert!(
        ((got_d_r2 - analytic_d_r2) / analytic_d_r2).abs() < 1.0e-9,
        "∂v(mid)/∂r2.r matches analytic: {got_d_r2} vs {analytic_d_r2}"
    );

    let got_d_v = result.get(mid.label(), "v1", "dc").expect("∂v_mid/∂v1.dc");
    assert!(
        ((got_d_v - 0.5) / 0.5).abs() < 1.0e-9,
        "∂v(mid)/∂v1.dc = 0.5 (analytic): got {got_d_v}"
    );
}

// ── DVG-08: the driver is a DC analysis variant (no codegen dep) ────────────
//
// The nonlinear diode fixture also solves through the adjoint: it
// converges on a DC operating point and the transpose solve produces a
// finite gradient. This proves the path does not assume linearity — the
// Jacobian is the converged Newton Jacobian, the same one `.sens`
// finite-differences around. (Value agreement with `.sens` is T9's job;
// here we just confirm the nonlinear path runs and produces a finite
// answer with the right physical sign — raising `r1` lowers the diode
// voltage.)
#[test]
fn adjoint_runs_on_a_nonlinear_circuit() {
    let (mut circuit, info) = AdjointDcCircuit::build("DiodeTop");
    let mid = CircuitBuilding::net_for(&circuit, &info, "mid");

    let result = circuit
        .adjoint(
            AdjointOptions::new(mid.clone(), vec![("r1".into(), "r".into())]),
            Context::default(),
        )
        .expect("adjoint")
        .solve()
        .expect("nonlinear adjoint solve");

    let d_r1 = result.get(mid.label(), "r1", "r").expect("∂v(mid)/∂r1.r");
    assert!(d_r1.is_finite(), "nonlinear gradient is finite: {d_r1}");
    assert!(d_r1 < 0.0, "raising r1 must lower the diode node: got {d_r1}");
    assert_eq!(result.transpose_solves, 1, "nonlinear does not change the solve count");
}

/// The circuit the adjoint operating-point tests solve.
struct AdjointDcCircuit;

impl AdjointDcCircuit {
    /// The build helper `build`.
    fn build(top: &str) -> (CircuitInstance, NodeMap) {
        CircuitBuilding::divider_fixture(top)
    }
}
