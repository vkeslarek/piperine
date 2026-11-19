//! Contributions under control flow, switch branches and guarded sites, run from MSL source on the
//! Rust backend (feature `analog-structure`, AS-01, AS-03, AS-06): each module is written in
//! `tests/structure/switch.msl`, elaborated by the front end, compiled on the transpiled backend
//! and run on the solver. Every expected value is written from the criterion it cites, never read
//! back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier};

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
