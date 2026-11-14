//! SC-04/SC-05 — PSS single shooting: a sine-driven RC converges to the
//! analytic steady-state phasor amplitude, the returned orbit is periodic
//! to the shooting tolerance, a digital divider reports `k`·period, and a
//! non-periodic circuit fails loud instead of returning a fake orbit.
//! (Options validation — bad period, negative pre-roll — is `failure_rules.rs`'s
//! §18 rows.)

// Each target includes the library by path: it uses a different subset of it.
#[allow(dead_code)]
#[path = "helpers/devices.rs"]
mod devices;

use devices::{ClockSrc, Comparator, NodeMap, ToggleDff, C, R, Vramp, Vsine};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, PssAnalysisOptions};

/// The circuits of the former Verilog-A fixture, from hand-written elements.
///
/// - `Top`: sine-driven RC, f = 1 kHz, tau = RC = 1 ms, so `wRC` is about 6.283;
///   |H| = 1/sqrt(1+(wRC)^2) is about 0.157167 and the steady-state out
///   amplitude is 5*|H|.
/// - `RampTop`: the same RC driven by a ramp, which is not periodic at any T.
/// - `DivTop`: a digital toggle clocked by an analog square wave at period T
///   (rising edges at T/4, 5T/4, ...): its output period is 2*T, the classic
///   divider-by-2 (same A2D pattern as example 21). The clock's edges sit at
///   T/4 and 3T/4 so every comparison-relevant net is stable in a whole
///   neighborhood of the k*T boundaries.
fn build(top: &str) -> (CircuitInstance, NodeMap) {
    let mut b = CircuitBuilder::new(top);
    let gnd = b.ground();
    let mut map = NodeMap::new();
    match top {
        "Top" | "RampTop" => {
            let top_node = b.node("top");
            let out = b.node("out");
            if top == "Top" {
                b.element(Box::new(Vsine::new("v1", 5.0, 1.0e3, top_node.clone(), gnd.clone())));
            } else {
                b.element(Box::new(Vramp::new("v1", top_node.clone(), gnd.clone())));
            }
            b.element(Box::new(R::new("r1", 1.0e3, top_node, out.clone())));
            b.element(Box::new(C::new("c1", 1.0e-6, out.clone(), gnd)));
            map.insert("out", out);
        }
        "DivTop" => {
            let clk_a = b.node("clk_a");
            let clk = b.digital_net(Some("clk"));
            let q = b.digital_net(Some("q"));
            b.element(Box::new(ClockSrc::new("ck", 1.0e-3, clk_a.clone(), gnd.clone())));
            b.element(Box::new(Comparator::new("cmp", clk_a.clone(), gnd, clk)));
            b.element(Box::new(ToggleDff::new("tg", clk, q)));
            map.insert("clk_a", clk_a);
        }
        other => panic!("no circuit `{other}`"),
    }
    (b.build().expect("build"), map)
}

/// Converged orbit: `|x(T)−x(0)| < shoot_tol` (residual reported in stats)
/// and the recorded period's output amplitude matches the analytic phasor
/// within 1 %.
#[test]
fn sine_rc_converges_to_the_analytic_phasor() {
    let (mut circuit, info) = build("Top");
    let out = info.id("out");

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
