//! Value inputs, results that hold a node, feedback and the local `Integ`, run from MSL source on the
//! Rust backend (feature `analog-structure`, AS-08, AS-09, AS-10, AS-04 AC 2 to 4): each module is
//! written in `tests/structure/modules.msl`, elaborated by the front end, compiled on the transpiled
//! backend and run on the solver. Every expected value is written from the criterion it cites, with
//! its closed form beside the assertion, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier};
use piperine_solver::protocol::ObservationKind;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::build;

/// The project of `tests/structure/modules.msl`.
fn project() -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/modules.msl");
    Project::new(&path)
}

/// The node `<top>.<name>` of an elaborated design.
fn node(ir: &piperine_ir::Ir, top: &str, name: &str) -> NodeIdentifier {
    NodeIdentifier::Anonymous(ir.node_at(&format!("{top}.{name}")).expect("a node").id().index())
}

/// `v(o)` of the bench `top` at its operating point.
fn operating_point(top: &str) -> f64 {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    let out = node(&running.ir, top, "o");
    running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves").get_node(&out).expect("v(o)")
}

/// `|v(o)|` of the bench `top` at each frequency of a sweep of `steps` points from `start` to `stop`.
fn ac_magnitudes(top: &str, start: f64, stop: f64, steps: usize) -> Vec<(f64, f64)> {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    let out = node(&running.ir, top, "o");
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    sweep.iter().map(|step| (step.frequency, step.get_node(&out).expect("v(o)").norm())).collect()
}

/// `v(o)` of the bench `top` over a transient to `stop` with the step cap `step`, as a waveform.
fn transient(top: &str, stop: f64, step: f64) -> Waveform {
    let built = build(&project(), top);
    let out = node(&built.ir, top, "o");
    let (_, result) = built.run_with(stop, step);
    let steps: Vec<_> = result.iter().collect();
    Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(o)")).collect())
}

#[test]
fn a_value_input_of_a_module_without_a_result_drives_its_contribution() {
    // AS-08 AC 1: `Drive(out, 2.0 * v(a))` with `v(a)` = 1 V and the callee's `v(o) +> x`: the caller's
    // expression is read where the callee reads `x`, so `v(o)` = 2 * 1 V = 2.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gain", "v(o)", Coordinate::OperatingPoint), 2.0, operating_point("GainBench"));
}

#[test]
fn a_value_input_is_differentiated_in_ac() {
    // AS-08 AC 2 (§8.3.16): the callee's row differentiates what the input probes: with 1 V of AC at
    // `a`, `v(o)` = 2 * v(a), so |v(o)| = 2.0 at every frequency.
    for (frequency, magnitude) in ac_magnitudes("GainAcBench", 1e3, 1e6, 4) {
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("gain ac", "|v(o)|", Coordinate::Frequency(frequency)), 2.0, magnitude);
    }
}

#[test]
fn a_value_input_reads_the_callers_carrier_from_its_change() {
    // AS-08 AC 3: `Drive(o, lv.known_or(0.0))` with `lv` = 1.0 until 1 us and 3.0 after: the callee
    // reads the caller's carrier, so `v(o)` is 1.0 before the change and 3.0 from it.
    let waveform = transient("CarrierBench", 2e-6, 0.05e-6);
    for (time, wanted) in [(0.5e-6, 1.0), (0.9e-6, 1.0), (1.1e-6, 3.0), (1.5e-6, 3.0), (2.0e-6, 3.0)] {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("carrier input", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}

#[test]
fn a_value_input_read_in_a_reaction_is_the_callers_value_at_that_instant() {
    // AS-08 AC 5 (§8.3.13): `Sample(v(a))` records `x` at 1 ms; `v(a)` = 1 kV/s * t is 1.0 V there, so
    // the record holds 1.0, the caller's expression read at the reaction's instant.
    let seen = build(&project(), "SampleBench").run(2e-3, 0.1e-3);
    let records: Vec<(f64, Vec<u8>)> = seen.all().iter().filter_map(|o| match &o.kind {
        ObservationKind::Record { bytes, .. } => Some((o.time, bytes.clone())),
        _ => None,
    }).collect();
    assert_eq!(records.len(), 1, "one record: {records:?}");
    assert!((records[0].0 - 1e-3).abs() <= 1e-12, "at 1 ms, got {}", records[0].0);
    let x = f64::from_le_bytes(records[0].1[..8].try_into().expect("the field x"));
    assert!((x - 1.0).abs() <= 1e-6, "x = {x}");
}

#[test]
fn lag_follows_one_minus_exp() {
    // AS-09 AC 1: `v(o) +> Lag(v(inp))` with tau = 1 ms and `v(inp)` stepping from 0 to 1 V at 0:
    // `v(o)` = 1 - exp(-t / tau).
    let waveform = transient("LagBench", 5e-3, 10e-6);
    for time in [0.5e-3, 1e-3, 2e-3, 3e-3, 5e-3] {
        let wanted = 1.0 - (-time / 1e-3_f64).exp();
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("lag", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}

#[test]
fn lag_is_a_first_order_low_pass_in_ac() {
    // AS-09 AC 2: with a unit stimulus at `inp`, |v(o) / v(inp)| = 1 / sqrt(1 + (2 pi f tau)^2).
    for (frequency, magnitude) in ac_magnitudes("LagAcBench", 10.0, 1e5, 5) {
        let wanted = 1.0 / (1.0 + (2.0 * std::f64::consts::PI * frequency * 1e-3).powi(2)).sqrt();
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("lag ac", "|v(o)|", Coordinate::Frequency(frequency)), wanted, magnitude);
    }
}

#[test]
fn a_lag_under_an_untaken_branch_keeps_integrating() {
    // AS-09 AC 4 (§8.3.14): the placement sits under `if en.high()`, with `en` rising at 2 ms = 2 tau.
    // The instance exists and advances its state from 0 whatever the branch: just after 2 tau `v(o)`
    // = 1 - exp(-2), and 0 before it.
    let waveform = transient("UntakenBench", 3e-3, 10e-6);
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("untaken", "v(o)", Coordinate::Time(1e-3)), 0.0, waveform.at(1e-3).expect("a point"));
    let wanted = 1.0 - (-2.0_f64).exp();
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("untaken", "v(o)", Coordinate::Time(2.02e-3)), 1.0 - (-2.02_f64).exp(), waveform.at(2.02e-3).expect("a point"));
    assert!((wanted - 0.8647).abs() < 1e-3, "the closed form of the criterion");
}

#[test]
fn a_named_lag_is_one_instance_for_analog_and_its_event() {
    // AS-09 AC 5: `let l = Lag(v(inp));` read by `analog` (`v(o) +> l`) and by `on above(l - 0.5)`:
    // one instance, so the event fires when `v(o)` crosses 0.5, at tau * ln 2, within the default
    // `ttol` of 1 ps.
    let seen = build(&project(), "NamedBench").run(0.8e-3, 0.05e-6);
    let changes = support::transient(&seen, "NamedBench.crossed");
    assert_eq!(changes.len(), 1, "the event fires once: {changes:?}");
    let at = changes[0].0 as f64 * 1e-15;
    let wanted = 1e-3 * std::f64::consts::LN_2;
    assert!((at - wanted).abs() <= 1e-12, "fired at {at}, wanted {wanted} within 1 ps");
}

#[test]
fn feedback_through_integ_decays_exponentially() {
    // AS-10 AC 1: `let y = Integ(-y / 1m, 1.0, None);` is y' = -y / 1 ms with y(0) = ic = 1.0: `y` = 1.0
    // at the operating point and exp(-t / 1 ms) after it.
    let waveform = transient("FeedbackBench", 5e-3, 10e-6);
    for time in [0.0, 1e-3, 2e-3, 3e-3, 5e-3] {
        let wanted = (-time / 1e-3_f64).exp();
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("feedback", "y", Coordinate::Time(time)), wanted, actual);
    }
}

#[test]
fn integ_holds_ic_while_reset_then_integrates() {
    // AS-04 AC 2: `Integ(1.0, 0.5, now() < 1m)` holds 0.5 until 1 ms (`reset` true) and integrates x =
    // 1.0 from the held value after: 0.5 + (t - 1 ms) (x in V/s).
    let waveform = transient("ResetBench", 20e-3, 20e-6);
    for time in [0.5e-3, 0.9e-3, 5e-3, 10e-3, 20e-3] {
        let wanted = if time < 1e-3 { 0.5 } else { 0.5 + (time - 1e-3) };
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("reset", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}

#[test]
fn integ_is_one_over_j_omega_in_ac() {
    // AS-04 AC 3: swept in AC with `x` = `v(inp)` and a unit stimulus, the result is x / (j w):
    // |v(o)| = 1 / (2 pi f).
    for (frequency, magnitude) in ac_magnitudes("IntegAcBench", 10.0, 1e5, 5) {
        let wanted = 1.0 / (2.0 * std::f64::consts::PI * frequency);
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("integ ac", "|v(o)|", Coordinate::Frequency(frequency)), wanted, magnitude);
    }
}

#[test]
fn integ_holds_its_result_at_ic_at_the_operating_point() {
    // AS-04 AC 4: with `ic` given an equilibrium solve holds the result at `ic`: v(o) = 0.5.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("hold", "v(o)", Coordinate::OperatingPoint), 0.5, operating_point("HoldBench"));
}

#[test]
fn slew_bounds_the_rate_and_tracks_at_the_operating_point() {
    // AS-09 AC 6 (Q3, Part VII §22.2.10): `slew(v(inp), 1M, 1M)` follows a 0 to 1 V step at 1 us at
    // the bound of 1 V/us: 0 V before 1 us, `(t - 1 us) * 1e6` V on the ramp, 1 V from 2 us. The step
    // has a 0.1 us edge (10 V/us): the reaction `above(dx - r)` sees the rate run past the bound while
    // the edge rises, and the output leaves the input there. An ideal step is taken whole within one
    // solver step, before the reaction can run (a precision gap of the criterion's "step"). At an
    // operating point `ddt` is 0 and the mode is `Track`, so the result is `v(inp)`.
    let waveform = transient("SlewBench", 3e-6, 0.01e-6);
    for (time, wanted) in [(0.5e-6, 0.0), (0.9e-6, 0.0), (1.25e-6, 0.25), (1.5e-6, 0.5), (1.75e-6, 0.75), (2.5e-6, 1.0), (3.0e-6, 1.0)] {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("slew", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("slew hold", "v(o)", Coordinate::OperatingPoint), 0.7, operating_point("SlewHoldBench"));
}
