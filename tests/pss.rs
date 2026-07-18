//! SC-04/SC-05 — PSS single shooting: a sine-driven RC converges to the
//! analytic steady-state phasor amplitude, the returned orbit is periodic
//! to the shooting tolerance, and the loud paths (bad period,
//! non-convergent/non-periodic circuit) fail with `PSS` errors.

use std::path::PathBuf;

use piperine_codegen::CircuitCompiler;
use piperine_lang::SourceMap;
use piperine_solver::prelude::{Context, PssAnalysisOptions};

/// Sine-driven RC: f = 1 kHz, τ = RC = 1 ms → ωRC ≈ 6.283.
/// |H| = 1/√(1+(ωRC)²) ≈ 0.157167; steady-state out amplitude = 5·|H|.
const FIXTURE: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod R (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog R { I(p, n) <+ V(p, n) / r; }

    mod C (inout p : Electrical, inout n : Electrical) {
        param c : Real = 1.0e-6;
    }
    analog C { I(p, n) <+ c * ddt(V(p, n)); }

    mod Vsine (inout p : Electrical, inout n : Electrical) {
        param amp : Real = 5.0;
        param freq : Real = 1.0e3;
    }
    analog Vsine { V(p, n) <- amp * sin(2.0 * 3.14159265358979 * freq * $abstime); }

    mod Vramp (inout p : Electrical, inout n : Electrical) {
    }
    analog Vramp { V(p, n) <- 1.0e3 * $abstime; }

    mod Top () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire out : Electrical;
        v1 : Vsine(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = out) {};
        c1 : C(.p = out, .n = gnd) {};
    }

    mod RampTop () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire out : Electrical;
        v1 : Vramp(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = out) {};
        c1 : C(.p = out, .n = gnd) {};
    }
"#;

fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn build(top: &str) -> (piperine_solver::prelude::CircuitInstance, piperine_codegen::device::CircuitBuildInfo) {
    let design = piperine_lang::parse_and_elaborate(FIXTURE, &headers_source_map())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped(top).expect("build");
    circuit.init_digital().expect("digital");
    circuit.rebuild_digital_topology();
    (circuit, info)
}

/// Converged orbit: `|x(T)−x(0)| < shoot_tol` (residual reported in stats)
/// and the recorded period's output amplitude matches the analytic phasor
/// within 1 %.
#[test]
fn sine_rc_converges_to_the_analytic_phasor() {
    let (mut circuit, info) = build("Top");
    let out = info.nets.get("out").expect("net out").clone();

    let opts = PssAnalysisOptions::new(1.0e-3);
    let result = circuit.pss(opts, Context::default()).expect("pss").solve().expect("orbit");
    assert!(
        result.stats.residual < 1.0e-9,
        "periodicity residual: {:.3e}",
        result.stats.residual
    );

    let amplitude = result
        .trace
        .iter()
        .filter_map(|s| s.get_node(&out))
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    let w_rc = 2.0 * std::f64::consts::PI * 1.0e3 * 1.0e-3;
    let analytic = 5.0 / (1.0 + w_rc * w_rc).sqrt();
    assert!(
        ((amplitude - analytic) / analytic).abs() < 1.0e-2,
        "orbit amplitude {amplitude} vs analytic {analytic}"
    );
}

/// `period <= 0` → loud options error.
#[test]
fn non_positive_period_is_loud() {
    let (mut circuit, _info) = build("Top");
    let err = circuit.pss(PssAnalysisOptions::new(0.0), Context::default()).expect_err("bad period");
    assert!(err.to_string().contains("period"), "names the period: {err}");
}

/// A ramp-driven circuit is not periodic at any T: shooting must fail loud
/// (never return a fake orbit).
#[test]
fn non_periodic_circuit_fails_loud() {
    let (mut circuit, _info) = build("RampTop");
    let mut opts = PssAnalysisOptions::new(1.0e-3);
    opts.max_shoot_iter = 6;
    let err = circuit
        .pss(opts, Context::default())
        .expect("pss")
        .solve()
        .expect_err("ramp is not periodic");
    let msg = err.to_string();
    assert!(
        msg.contains("did not converge")
            || msg.contains("singular")
            || msg.contains("does not repeat"),
        "loud non-periodicity: {msg}"
    );
}
