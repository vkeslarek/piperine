//! SC-04/SC-05 — PSS single shooting: a sine-driven RC converges to the
//! analytic steady-state phasor amplitude, the returned orbit is periodic
//! to the shooting tolerance, a digital divider reports `k`·period, and a
//! non-periodic circuit fails loud instead of returning a fake orbit.
//! (Options validation — bad period, negative pre-roll — is `failure_rules.rs`'s
//! §18 rows.)

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::circuits::NodeMap;
use piperine_device_fixtures::mixed;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, PssAnalysisOptions};

/// Converged orbit: `|x(T)−x(0)| < shoot_tol` (residual reported in stats)
/// and the recorded period's output amplitude matches the analytic phasor
/// within 1 %.
#[test]
fn sine_rc_converges_to_the_analytic_phasor() {
    let (mut circuit, info) = PssCases::build("Top");
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
    let (mut circuit, _info) = PssCases::build("RampTop");
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
    let (mut circuit, _info) = PssCases::build("DivTop");
    let err = circuit
        .pss(PssAnalysisOptions::new(1.0e-3), Context::default())
        .expect("pss")
        .solve()
        .expect_err("divider state closes at 2·T, not T");
    let msg = err.to_string();
    assert!(msg.contains("appears to be 2·T"), "k·T diagnostic: {msg}");
    assert!(msg.contains("2.000000e-3"), "suggests period = 2·T: {msg}");
}

/// The shooting reads the arrival of a shot through `TransientAnalysisResult::final_step`: a result with no
/// step is the transient's error. A shot always records its first step, so the shooting itself cannot be driven
/// to it; the claim is the reader's.
#[test]
fn a_result_with_no_step_has_no_final_step_error() {
    let empty = piperine_solver::prelude::TransientAnalysisResult::new(Vec::new());
    match empty.final_step() {
        Err(piperine_solver::abi::Error::Simple { domain: piperine_solver::abi::SolverDomain::Transient, detail }) => {
            assert!(detail.contains("no step"), "{detail}")
        }
        other => panic!("no step is the transient's error: {other:?}"),
    }
}

/// The helpers of the pss tests.
pub struct PssCases;

impl PssCases {
    /// The circuits of the former Verilog-A fixture, from hand-written devices.
    ///
    /// - `Top`: sine-driven RC, f = 1 kHz, tau = RC = 1 ms, so `wRC` is about 6.283;
    ///   |H| = 1/sqrt(1+(wRC)^2) is about 0.157167 and the steady-state out
    ///   amplitude is 5*|H|.
    /// - `RampTop`: the same RC driven by a ramp, which is not periodic at any T.
    /// - `DivTop`: a digital toggle clocked by an analog square wave at period T
    ///   (rising edges at T/4, 5T/4, ...): its output period is 2*T, the classic
    ///   divider-by-2 (same A2D pattern as example 21). The clock's edges sit at
    ///   T/4 and 3T/4 so every comparison-relevant net is stable in a whole
    ///   neighborhood of the k*T boundaries. The comparator's 0.5 V threshold is
    ///   a reference source on its minus input.
    fn build(top: &str) -> (CircuitInstance, NodeMap) {
        let mut b = CircuitBuilder::new(top);
        let gnd = b.ground();
        let mut map = NodeMap::new();
        match top {
            "Top" | "RampTop" => {
                let top_node = b.node("top");
                let out = b.node("out");
                let ports = [Port::Node(top_node.clone()), Port::Node(gnd.clone())];
                if top == "Top" {
                    let params = [("amp", ParamValue::Real(5.0)), ("freq", ParamValue::Real(1.0e3))];
                    b.device(circuit::SineSource::module(), "v1", &params, &ports);
                } else {
                    b.device(circuit::RampSource::module(), "v1", &[], &ports);
                }
                CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top_node, &out);
                CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-6, &out, &gnd);
                map.insert("out", out);
            }
            "DivTop" => {
                let clk_a = b.node("clk_a");
                let vth = b.node("vth");
                let ports = [Port::Node(clk_a.clone()), Port::Node(gnd.clone())];
                b.device(mixed::ClockSource::module(), "ck", &[("period", ParamValue::Real(1.0e-3))], &ports);
                CircuitBuilding::add_voltage_source(&mut b, "vth", 0.5, &vth, &gnd);
                let ports = [Port::Node(clk_a.clone()), Port::Node(vth), Port::Net("clk".into())];
                b.device(mixed::Comparator::module(), "cmp", &[], &ports);
                b.device(mixed::ToggleFlipFlop::module(), "tg", &[], &[Port::Net("clk".into()), Port::Net("q".into())]);
                map.insert("clk_a", clk_a);
            }
            other => panic!("no circuit `{other}`"),
        }
        (b.build().expect("build"), map)
    }
}
