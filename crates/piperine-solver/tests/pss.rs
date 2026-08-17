//! SC-04/SC-05 — PSS single shooting: a sine-driven RC converges to the
//! analytic steady-state phasor amplitude, the returned orbit is periodic
//! to the shooting tolerance, a digital divider reports `k`·period, and a
//! non-periodic circuit fails loud instead of returning a fake orbit.
//! (Options validation — bad period, negative pre-roll — is `failure_rules.rs`'s
//! §18 rows.)

use std::path::PathBuf;

use piperine_codegen::CircuitCompiler;
use piperine_lang::SourceMap;
use piperine_solver::prelude::{Context, PssAnalysisOptions};

/// Sine-driven RC: f = 1 kHz, τ = RC = 1 ms → ωRC ≈ 6.283.
/// |H| = 1/√(1+(ωRC)²) ≈ 0.157167; steady-state out amplitude = 5·|H|.
const FIXTURE: &str = r#"
    `include "disciplines.vams"

    module R (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog I(p, n) <+ V(p, n) / r;
    endmodule

    module C (inout electrical p, inout electrical n);
        parameter real c = 1.0e-6;
        analog I(p, n) <+ c * ddt(V(p, n));
    endmodule

    module Vsine (inout electrical p, inout electrical n);
        parameter real amp = 5.0;
        parameter real freq = 1.0e3;
        analog V(p, n) <+ amp * sin(2.0 * 3.14159265358979 * freq * $abstime);
    endmodule

    module Vramp (inout electrical p, inout electrical n);
        analog V(p, n) <+ 1.0e3 * $abstime;
    endmodule

    module Top ();
        electrical gnd, top, out;
        Vsine v1 (.p(top), .n(gnd));
        R r1 (.p(top), .n(out));
        C c1 (.p(out), .n(gnd));
    endmodule

    module RampTop ();
        electrical gnd, top, out;
        Vramp v1 (.p(top), .n(gnd));
        R r1 (.p(top), .n(out));
        C c1 (.p(out), .n(gnd));
    endmodule

    // Digital toggle clocked by an analog square wave at period T (rising
    // edges at T/4, 5T/4, …): its output period is 2·T — the classic
    // divider-by-2 (same A2D pattern as example 21). The clock's edges sit
    // at T/4 and 3T/4 so every comparison-relevant net is stable in a whole
    // neighborhood of the k·T boundaries (a stimulus wrapping exactly at the
    // boundary would be phase-jittered by floating-point rounding).
    module ClockSrc (inout electrical clk_a, inout electrical gnd);
        parameter real period = 1.0e-3;
        real ph;
        analog begin
            ph = $abstime - period * floor($abstime / period);
            V(clk_a, gnd) <+ (ph > period * 0.25 && ph < period * 0.75) ? 1.0 : 0.0;
        end
    endmodule

    module Comparator (input electrical a, input electrical n, output logic y);
        always @(*) begin
            y <= V(a, n) > 0.5;
        end
    endmodule

    // Divide-by-2: one clocked block counts posedges into the module-level
    // `integer` register and republishes its parity on `q`, so `q` flips on
    // every rising clock edge and its period is 2·T.
    module ToggleDff (input logic clk, output logic q);
        integer st;
        always begin
            q <= (st % 2) > 0;
            @(posedge clk) st = st + 1;
        end
    endmodule

    module DivTop ();
        electrical gnd, clk_a;
        logic clk, q;
        ClockSrc #(.period(1.0e-3)) ck (.clk_a(clk_a), .gnd(gnd));
        Comparator cmp (.a(clk_a), .n(gnd), .y(clk));
        ToggleDff tg (.clk(clk), .q(q));
    endmodule
"#;

fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn build(top: &str) -> (piperine_solver::prelude::CircuitInstance, piperine_codegen::device::CircuitBuildInfo) {
    let design = piperine_lang::parse_and_elaborate(FIXTURE, &headers_source_map())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lower");
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
        result.stats.residual < 1.0e-6,
        "periodicity residual under shoot_tol: {:.3e}",
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

// The two options-validation cases this target used to carry
// (`period <= 0`, `tstab < 0`) are proven by `failure_rules.rs`'s
// `section_18_a_non_positive_period_is_an_analysis_error` and
// `section_18_a_negative_pre_roll_is_an_analysis_error`, which assert the
// typed `SolverDomain::Pss` as well as the message fragment — deleted here
// per MD-28.3 (delete the weaker of two tests asserting one behavior).

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

/// SC-05 — mixed-signal digital periodicity: the analog RC closes its orbit
/// at T but the toggle's state only closes at 2·T (divider-by-2), so PSS at
/// T fails loud with the "period appears to be k·T" diagnostic naming k = 2
/// and the suggested re-run period.
#[test]
fn digital_divider_reports_k_times_period() {
    let (mut circuit, _info) = build("DivTop");
    let err = circuit
        .pss(PssAnalysisOptions::new(1.0e-3), Context::default())
        .expect("pss")
        .solve()
        .expect_err("divider state closes at 2·T, not T");
    let msg = err.to_string();
    assert!(msg.contains("appears to be 2·T"), "k·T diagnostic: {msg}");
    assert!(msg.contains("2.000000e-3"), "suggests period = 2·T: {msg}");
}
