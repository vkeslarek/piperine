//! Contributions under control flow, switch branches and guarded sites, run from MSL source on the
//! Rust backend (feature `analog-structure`, AS-01, AS-03, AS-06): each module is written in
//! `tests/structure/switch.msl`, elaborated by the front end, compiled on the transpiled backend
//! and run on the solver. Every expected value is written from the criterion it cites, never read
//! back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, BranchIdentifier, Context, NodeIdentifier};

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

fn project() -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/switch.msl");
    Project::new(&path)
}

/// `v(a)` of the bench `top` at its operating point.
fn operating_point(top: &str) -> f64 {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    let a = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.a")).expect("node a").id().index());
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    solved.get_node(&a).expect("v(a)")
}

/// What the operating point of `top` fails with.
fn operating_point_failure(top: &str) -> String {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    match running.circuit.dc(Context::default()).and_then(|mut dc| dc.solve()) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

/// `v(a)` of `SwitchBench` at every point of a transient to 1.9 us (the oscillator rises at 1 us
/// and falls at 2 us), as `(time, volts)`.
fn switch_transient() -> Vec<(f64, f64)> {
    let built = build(&project(), "SwitchBench");
    let a = NodeIdentifier::Anonymous(built.ir.node_at("SwitchBench.a").expect("node a").id().index());
    let (_, result) = built.run_with(1.9e-6, 0.1e-6);
    result.iter().map(|point| (point.time(), point.get_node(&a).expect("v(a)"))).collect()
}

/// The flow of the branch `(a, gnd)` of the switch of `SwitchBench` at every point of the same
/// transient, as `(time, amps)`: the current the switch takes from `a`.
fn switch_flow() -> Vec<(f64, f64)> {
    let built = build(&project(), "SwitchBench");
    let branch = BranchIdentifier::new("SwitchBench.sw".to_string(), "b0.flow".to_string());
    let (_, result) = built.run_with(1.9e-6, 0.1e-6);
    result.iter().map(|point| (point.time(), point.get_branch(branch.clone()).expect("the flow of the switch"))).collect()
}

#[test]
fn a_flow_contribution_under_an_if_on_a_probe_gives_one_volt() {
    // AS-01 AC 1: `if v(a) > 0.5 { i +> v/1k } else { i +> v/2k }` driven by 1 mA: the first arm
    // holds at v = 1 mA * 1 kOhm = 1.0 V (the second would give 2.0 V, which is not below 0.5).
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("halved", "v(a)", Coordinate::OperatingPoint), 1.0, operating_point("HalvedBench"));
}

#[test]
fn a_match_on_a_carrier_adds_only_the_selected_arm() {
    // AS-01 AC 2: 1 mA into `v/1k` (`Fast`) is 1.0 V and into `v/2k` (`Slow`) is 2.0 V: the solve
    // adds only the arm the carrier's settled value selects.
    for (top, volts) in [("FastBench", 1.0), ("SlowBench", 2.0)] {
        ToleranceContract::DC_VOLTAGE.assert(&Point::new(top, "v(a)", Coordinate::OperatingPoint), volts, operating_point(top));
    }
}

#[test]
fn an_untaken_contribution_leaves_the_branch_with_zero_flow() {
    // AS-01 AC 4 (§11 rule 4): beside 1 MOhm and 1 mA, `en` low leaves the branch with no flow:
    // v = 1 mA * 1 MOhm = 1000 V; `en` high adds 1 kOhm: v = 1 mA / (1/1k + 1/1M) = 0.999000999 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gated low", "v(a)", Coordinate::OperatingPoint), 1000.0, operating_point("GatedLowBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gated high", "v(a)", Coordinate::OperatingPoint), 1e-3 / (1.0 / 1e3 + 1.0 / 1e6), operating_point("GatedHighBench"));
}

#[test]
fn two_enabled_flow_arms_sum() {
    // §11 rule 1: two enabled arms each add `v/1k`: 1 mA into 2/1k is 0.5 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("twice", "v(a)", Coordinate::OperatingPoint), 0.5, operating_point("TwiceBench"));
}

#[test]
fn the_switch_shorts_at_one_microsecond() {
    // AS-03 AC 1: behind 1 kOhm from 1 V the open switch leaves v(a) = 1 V (no flow); once `ctl`
    // rises at 1 us it shorts: v(a) = 0 V, with 1 V / 1 kOhm = 1 mA through the series resistor.
    let points = switch_transient();
    for (time, volts) in &points {
        let wanted = if *time < 1e-6 - 1e-12 { 1.0 } else if *time > 1e-6 + 1e-12 { 0.0 } else { continue };
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("switch", "v(a)", Coordinate::Time(*time)), wanted, *volts);
    }
    assert!(points.iter().any(|(t, _)| *t < 1e-6 - 1e-12) && points.iter().any(|(t, _)| *t > 1e-6 + 1e-12), "the run covers both sides of 1 us");
    // and the flow of `(a, gnd)`: 0 while it is open, 1 V / 1 kOhm = 1 mA once it shorts
    for (time, amps) in switch_flow() {
        let wanted = if time < 1e-6 - 1e-12 { 0.0 } else if time > 1e-6 + 1e-12 { 1e-3 } else { continue };
        ToleranceContract::DC_CURRENT.assert(&Point::new("switch", "i(a, gnd)", Coordinate::Time(time)), wanted, amps);
    }
}

#[test]
fn a_switch_changes_kind_at_the_step_after_the_instant() {
    // AS-03 AC 2 (§11 rule 18, §15.2): the solve at the instant of the change uses the old kind and
    // the solve after it the new one, as a jump: v(a) is still 1 V at 1 us and 0 V at the next point.
    let points = switch_transient();
    let at = points.iter().rposition(|(t, _)| (*t - 1e-6).abs() <= 1e-12).expect("a point at 1 us");
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("switch", "v(a) at the instant", Coordinate::Time(points[at].0)), 1.0, points[at].1);
    let after = points.get(at + 1).expect("a point after 1 us");
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("switch", "v(a) after the instant", Coordinate::Time(after.0)), 0.0, after.1);
}

#[test]
fn both_kinds_enabled_in_one_solve_fail_with_e0418() {
    // AS-03 AC 3 (§11 rule 2): `en` high enables a potential and a flow contribution to one branch
    // in the same solve: the run fails with E0418 naming the instance and the branch.
    let err = operating_point_failure("BothBench");
    assert!(err.contains("E0418") && err.contains("both"), "{err:?}");
    assert!(err.contains("BothBench.both") && err.contains("branch v(a)"), "it names the instance and the branch: {err:?}");
}

#[test]
fn an_ac_sweep_takes_the_switch_kind_with_static_false() {
    // AS-03 AC 4 (§11.1.5): `analysis(Static)` is true at the operating point (the branch is open and
    // v(a) = 1 V) and false in the sweep, where the branch is the short `v(a) = 0`: |v(a)| = 0.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("sweep", "v(a)", Coordinate::OperatingPoint), 1.0, operating_point("SweepBench"));
    let mut running = project().compile("SweepBench").unwrap_or_else(|r| panic!("{r}"));
    let a = NodeIdentifier::Anonymous(running.ir.node_at("SweepBench.a").expect("node a").id().index());
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac
        .solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e6, steps: 4, logarithmic: true })
        .unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    for step in sweep.iter() {
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("sweep", "|v(a)|", Coordinate::Frequency(step.frequency)), 0.0, step.get_node(&a).expect("v(a)").norm());
    }
}

/// What the operating point of `top` solves, as the voltage at `a`; the failure text otherwise.
fn operating_point_result(top: &str) -> Result<f64, String> {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    let a = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.a")).expect("node a").id().index());
    match running.circuit.dc(Context::default()).and_then(|mut dc| dc.solve()) {
        Ok(solved) => Ok(solved.get_node(&a).expect("v(a)")),
        Err(e) => Err(e.to_string()),
    }
}

#[test]
fn a_limit_under_an_untaken_branch_neither_vetoes_nor_moves() {
    // AS-06 AC 1 (§14.7.4, abi.md A3): `Stubborn` limits to the last iterate whenever its limit runs,
    // so a solve that runs it is vetoed at every iterate and never converges. With `en` low the call is
    // not evaluated: it writes PROPOSED = LIMITED = LAST, the solver sees no gap, and the operating
    // point is the plain 1 mA into 1 kOhm: 1.0 V.
    assert_eq!(operating_point_result("StubbornOnBench").is_err(), true, "an enabled limiter that never moves is vetoed forever");
    let off = operating_point_result("StubbornOffBench").expect("a limit that is not evaluated does not veto");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("stubborn off", "v(a)", Coordinate::OperatingPoint), 1.0, off);
}

/// The drop across `tests/ngspice/rdiode.cir`'s junction (5 V behind 1 kOhm into a diode of `is` 1e-14 at
/// 27 C, `gmin` 1e-12): the root of `(5 - v) / 1k = is (exp(v / vt) - 1) + gmin v`, by bisection, so it
/// is written from the circuit and not read from the model under test.
fn rdiode_drop() -> f64 {
    let thermal_voltage = 1.380649e-23 * 300.15 / 1.602176634e-19;
    let residual = |v: f64| (5.0 - v) / 1e3 - 1e-14 * ((v / thermal_voltage).exp() - 1.0) - 1e-12 * v;
    let (mut low, mut high) = (0.0_f64, 5.0_f64);
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if residual(middle) > 0.0 { low = middle } else { high = middle }
    }
    0.5 * (low + high)
}

#[test]
fn a_junction_enabled_after_the_operating_point_reaches_the_rdiode_solution() {
    // AS-06 AC 2: the limited junction behind 1 kOhm from 5 V is open (v(a) = 5 V) until `en` rises at
    // 1 us; once it runs, the circuit settles at the `rdiode` solution (about 0.7 V): `rdiode.cir`'s
    // `v(out)` where ngspice is present, the root of that circuit's equation otherwise, under
    // `DC_VOLTAGE`; the same junction with no enable lands there too.
    let wanted = rdiode_drop();
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("ungated junction", "v(a)", Coordinate::OperatingPoint), wanted, operating_point_result("JunctionBench").expect("the ungated junction solves"));
    let built = build(&project(), "GatedJunctionBench");
    let a = NodeIdentifier::Anonymous(built.ir.node_at("GatedJunctionBench.a").expect("node a").id().index());
    let (_, result) = built.run_with(1.9e-6, 0.1e-6);
    let points: Vec<(f64, f64)> = result.iter().map(|p| (p.time(), p.get_node(&a).expect("v(a)"))).collect();
    let before = points.iter().rev().find(|(t, _)| *t < 1e-6 - 1e-12).expect("a point before 1 us");
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("gated junction", "v(a) before", Coordinate::Time(before.0)), 5.0, before.1);
    let last = points.last().expect("points");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gated junction", "v(a) after", Coordinate::Time(last.0)), wanted, last.1);
    if let Reference::Golden(golden) = Ngspice::from_environment().run("rdiode") {
        let at = Point::new("gated junction", "v(a) after against ngspice", Coordinate::Time(last.0)).with_ngspice(golden.version());
        ToleranceContract::DC_VOLTAGE.assert(&at, golden.printed("v(out)"), last.1);
    }
}

#[test]
fn a_limit_reads_a_carrier_through_known_or() {
    // AS-06 AC 3 (abi.md F2): `limit(lv.known_or(0.0) + 0.5 * v(a), ..) * 1m` with `lv` = 2.0 into
    // 1 kOhm: the value input is the sum of a carrier and a probe, which the limiter (a damped step with
    // the input as its fixed point) settles at, so v(a) = 1 kOhm * 1 mA * (2.0 + 0.5 v(a)): v = 4.0 V.
    // The carrier alone would give 2.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("carrier limit", "v(a)", Coordinate::OperatingPoint), 4.0, operating_point("CarrierLimitBench"));
}

/// `|v(a)|` of the bench `top` at 100 kHz in an AC sweep of one point.
fn ac_magnitude(top: &str) -> f64 {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    let a = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.a")).expect("node a").id().index());
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e5, stop_frequency: 1e5, steps: 1, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    sweep.iter().next().expect("a point").get_node(&a).expect("v(a)").norm()
}

#[test]
fn an_ac_stim_under_an_if_drives_only_when_enabled() {
    // AS-06 AC 4 (the expression form): `v(a) +> if en.high() { ac_stim(1.0) } else { 0.0 }`: the sweep injects the unit
    // stimulus with `en` high (|v(a)| = 1) and nothing with it low (|v(a)| = 0): a term in an arm
    // not taken does not read the stimulus.
    ToleranceContract::AC_MAGNITUDE.assert(&Point::new("stim on", "|v(a)|", Coordinate::Frequency(1e5)), 1.0, ac_magnitude("StimOnBench"));
    ToleranceContract::AC_MAGNITUDE.assert(&Point::new("stim off", "|v(a)|", Coordinate::Frequency(1e5)), 0.0, ac_magnitude("StimOffBench"));
}

#[test]
fn a_flow_ac_stim_under_a_statement_if_drives_only_when_enabled() {
    // AS-06 AC 4 as written: `if en.high() { i(a, gnd) +> ac_stim(1m); }` with 1 kOhm from `a` to ground
    // gives |v(a)| = 1 mA * 1 kOhm = 1 V with `en` high and 0 V with it low.
    ToleranceContract::AC_MAGNITUDE.assert(&Point::new("flow stim on", "|v(a)|", Coordinate::Frequency(1e5)), 1.0, ac_magnitude("FlowStimOnBench"));
    ToleranceContract::AC_MAGNITUDE.assert(&Point::new("flow stim off", "|v(a)|", Coordinate::Frequency(1e5)), 0.0, ac_magnitude("FlowStimOffBench"));
}

#[test]
fn an_ac_stim_reads_its_magnitude_from_a_carrier() {
    // AS-06 AC 5: `ac_stim(lv.known_or(0.0))` with `lv` = 3.0 gives |v(a)| = 3.0.
    ToleranceContract::AC_MAGNITUDE.assert(&Point::new("carrier stim", "|v(a)|", Coordinate::Frequency(1e5)), 3.0, ac_magnitude("CarrierStimBench"));
}

#[test]
fn a_bound_step_under_an_if_caps_only_the_steps_that_executed_it() {
    // AS-01 AC 5 (§14.10.2 and Part V §16.2 rule 5: `bound_step` caps the step that follows the time
    // point at which it was evaluated): `if ctl.high() { bound_step(1n); }` on a 1 kHz sine with the
    // host's step limit at 20 ns. Until the solution at which `ctl` is high has been
    // accepted nothing caps a step (one reaches well over 1 ns); the point after the rise at 1 us is
    // the first to execute the call, and every step that follows a point from there on is at most 1 ns.
    let built = build(&project(), "CapperBench");
    let (_, result) = built.run_with(1.05e-6, 20e-9);
    let times: Vec<f64> = result.iter().map(|p| p.time()).collect();
    let first_executed = times.iter().copied().find(|t| *t > 1e-6 + 1e-12).expect("a point after the rise");
    let before: Vec<f64> = times.windows(2).filter(|w| w[0] < first_executed - 1e-12).map(|w| w[1] - w[0]).collect();
    let after: Vec<f64> = times.windows(2).filter(|w| w[0] >= first_executed - 1e-12).map(|w| w[1] - w[0]).collect();
    assert!(before.iter().cloned().fold(0.0, f64::max) > 5e-9, "nothing caps the steps before the call is executed: {before:?}");
    assert!(after.len() > 5 && after.iter().all(|dt| *dt <= 1e-9 * (1.0 + 1e-9)), "every step after it is capped at 1 ns: {after:?}");
}

#[test]
fn capic_has_the_admittance_of_a_capacitor_in_ac() {
    // AS-03 AC 6 (§16 H7, §11.1.5): in a sweep `analysis(Transient)` is false, so `CapIc` is the
    // capacitor `i +> ddt(c * v)` and `ic` is gone. With R = 1 kOhm, C = 1/(2 pi 100 kHz R) the
    // divider is 1 / (1 + j w R C) = 1 / (1 + j) at 100 kHz: |v(a)| = 1 / sqrt(2).
    ToleranceContract::AC_MAGNITUDE.assert(&Point::new("capic", "|v(a)|", Coordinate::Frequency(1e5)), std::f64::consts::FRAC_1_SQRT_2, ac_magnitude("CapIcBench"));
}
