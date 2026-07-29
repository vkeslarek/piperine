//! The DC adjoint driver (dv-gradients T8, DVG-08/10): one transpose solve
//! `Jᵀλ = ∂f/∂x` at the converged operating point combines with each
//! parameter's `∂F/∂p` to yield `∂f/∂p = −λᵀ·∂F/∂p` for **every**
//! requested parameter from that one solve. The economic heart of the
//! adjoint: `.sens`'s two re-solves per parameter collapse to one extra
//! linear solve, regardless of count — asserted here, not claimed.

use std::path::PathBuf;

use piperine_codegen::CircuitCompiler;
use piperine_lang::SourceMap;
use piperine_solver::prelude::{AdjointOptions, Context, Net};

/// A divider plus a diode fixture (reused from `tests/sens.rs`). Three
/// differentiable parameters across two circuits: linear (Top) and
/// nonlinear (DiodeTop).
const DIVIDER: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod R (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog R { I(p, n) <+ V(p, n) / r; }

    mod D (inout p : Electrical, inout n : Electrical) {
        param is_sat : Real = 1.0e-14;
    }
    analog D { I(p, n) <+ is_sat * (exp(V(p, n) / 0.02585) - 1.0); }

    mod Vsrc (inout p : Electrical, inout n : Electrical) {
        param dc : Real = 10.0;
    }
    analog Vsrc { V(p, n) <- dc; }

    mod Top () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire mid : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = mid) {};
        r2 : R(.p = mid, .n = gnd) {};
    }

    mod DiodeTop () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire mid : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) { .dc = 1.0 };
        r1 : R(.p = top, .n = mid) {};
        d1 : D(.p = mid, .n = gnd) {};
    }
"#;

fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn build(top: &str) -> (piperine_solver::prelude::CircuitInstance, piperine_codegen::device::CircuitBuildInfo) {
    let design = piperine_lang::parse_and_elaborate(DIVIDER, &headers_source_map())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped(top).expect("build");
    circuit.init_digital().expect("digital");
    circuit.rebuild_digital_topology();
    (circuit, info)
}

fn net_for(
    circuit: &piperine_solver::prelude::CircuitInstance,
    info: &piperine_codegen::device::CircuitBuildInfo,
    name: &str,
) -> Net {
    let node = info.nets.get(name).unwrap_or_else(|| panic!("net `{name}` not mapped")).clone();
    let var = piperine_solver::abi::AnalogVariable::Node(node);
    circuit
        .nets()
        .into_iter()
        .find(|n| n.analog_variable().map(|v| **v == var).unwrap_or(false))
        .unwrap_or_else(|| panic!("net `{name}` not solved"))
}

// ── DVG-10: one transpose solve serves every requested parameter ──────────
//
// The economic heart of the adjoint. A 20-parameter request costs exactly
// ONE transpose solve — not one per parameter. The count is asserted on
// the result, not inspected by eye or claimed in a comment.
#[test]
fn one_transpose_solve_serves_every_requested_parameter() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");

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
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");

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
    let (mut circuit, info) = build("DiodeTop");
    let mid = net_for(&circuit, &info, "mid");

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
