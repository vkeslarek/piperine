//! `bound_step`, `freq()` and `analysis(kind)` run from MSL source on the Rust backend (feature
//! `msl-fixes`, FIX-09 to FIX-11): each module is written in `tests/fixes/environment.msl`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the criterion it cites, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier, Solver, TransientAnalysisOptions};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::{build, series, value_at};

fn project() -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixes/environment.msl");
    Project::new(&path)
}

/// The instants of the accepted steps of the transient of `top`, run to `stop` with the host's
/// own step limit as wide as the run.
fn step_times(top: &str, stop: f64) -> Vec<f64> {
    let (_, result) = build(&project(), top).run_with(stop, stop);
    result.iter().map(|point| point.time()).collect()
}

/// The widest step after the first.
fn widest_after_the_first(times: &[f64]) -> f64 {
    times.windows(2).skip(1).map(|pair| pair[1] - pair[0]).fold(0.0, f64::max)
}

#[test]
fn bound_step_caps_every_step() {
    // FIX-09 AC 1: `bound_step(0.02 / f)` on a 1 kHz sine, run to 2 ms: no step after the first is
    // longer than 20 us. (That the cap is what limits a step is `the_smaller_of_two_caps_wins`: a
    // 5 us cap is under the 17 us the sine's own error control takes.)
    let capped = widest_after_the_first(&step_times("SineBench", 2e-3));
    assert!(capped <= 20e-6 * (1.0 + 1e-9), "a step of {capped:e} s past the 20 us cap");
    assert!(capped > 0.5 * 20e-6, "the steps reach the cap, widest {capped:e} s");
}

#[test]
fn the_smaller_of_two_caps_wins() {
    // FIX-09 AC 2: two instances capping at 20 us and 5 us: no step is longer than 5 us.
    let widest = widest_after_the_first(&step_times("TwoCapsBench", 0.5e-3));
    assert!(widest <= 5e-6 * (1.0 + 1e-9), "a step of {widest:e} s past the 5 us cap");
    assert!(widest > 0.5 * 5e-6, "the steps reach the smaller cap, widest {widest:e} s");
}

/// What the transient of `top` fails with.
fn transient_failure(top: &str) -> String {
    let built = build(&project(), top);
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = TransientAnalysisOptions::new(1e-4, 1e-4);
    options.dt_max = 1e-4;
    let mut solver = Solver::new(circuit).with_tran_opts(options).build();
    match solver.tran().and_then(|mut t| t.solve()) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_non_positive_bound_step_is_e0509() {
    // FIX-09 AC 3: a cap of 0 or of -1 us fails the run with E0509, naming the watch.
    for top in ["BadCapBench", "NegativeCapBench"] {
        let err = transient_failure(top);
        assert!(err.contains("E0509") && err.contains("cap"), "{top}: {err:?}");
    }
}

#[test]
fn freq_follows_the_ac_sweep() {
    // FIX-10 AC 1: a 1 mA AC source into `1m + freq() * 1n`: |v(a)| = 1m / (1m + f * 1n), whether
    // `freq()` is written in the block or in a function the block calls.
    for top in ["FreqAcBench", "FreqFnAcBench"] {
        let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
        let a = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.a")).expect("node a").id().index());
        let mut ac = running.circuit.ac(Context::default()).expect("ac");
        let sweep = ac
            .solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e8, steps: 11, logarithmic: true })
            .unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
        assert_eq!(sweep.iter().count(), 11);
        for step in sweep.iter() {
            let wanted = 1e-3 / (1e-3 + step.frequency * 1e-9);
            let actual = step.get_node(&a).expect("v(a)").norm();
            ToleranceContract::AC_MAGNITUDE.assert(&Point::new(top, "|v(a)|", Coordinate::Frequency(step.frequency)), wanted, actual);
        }
    }
}

#[test]
fn freq_is_zero_outside_a_sweep() {
    // FIX-10 AC 2: `v(a) = 1.0 + freq()` is 1.0 in an operating point and at every transient step.
    let mut running = project().compile("FreqHoldBench").unwrap_or_else(|r| panic!("{r}"));
    let id = NodeIdentifier::Anonymous(running.ir.node_at("FreqHoldBench.a").expect("node a").id().index());
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("freq op", "v(a)", Coordinate::OperatingPoint), 1.0, solved.get_node(&id).expect("v(a)"));
    let built = build(&project(), "FreqHoldBench");
    let id = NodeIdentifier::Anonymous(built.ir.node_at("FreqHoldBench.a").expect("node a").id().index());
    let (_, result) = built.run_with(1e-6, 0.1e-6);
    assert!(result.iter().count() > 2, "the transient takes steps");
    for point in result.iter() {
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("freq transient", "v(a)", Coordinate::Time(point.time())), 1.0, point.get_node(&id).expect("v(a)"));
    }
}

#[test]
fn freq_in_a_reaction_is_zero() {
    // FIX-10 AC 3: `r <- freq() + 7.0` at 1 ns commits 7.0: `freq()` is 0 in a reaction (§14.4.4).
    // The same holds when the reaction reads it through a function.
    for top in ["FreqReactionBench", "FreqFnReactionBench"] {
        let seen = build(&project(), top).run(3e-9, 0.5e-9);
        let last = series(&seen, &format!("{top}.r")).pop().expect("a value").1;
        assert_eq!(&last[..8], &7.0f64.to_le_bytes(), "{top}: {last:?}");
        assert_eq!(last[8], 1, "known");
    }
}

/// The four kind nodes `d`, `t`, `a`, `n` of `KindsBench` in `solved`, as volts.
fn kinds_of(ir: &piperine_ir::Ir, get: impl Fn(&NodeIdentifier) -> f64) -> [f64; 4] {
    ["d", "t", "a", "n"].map(|name| {
        let node = ir.node_at(&format!("KindsBench.{name}")).expect("a kind node");
        get(&NodeIdentifier::Anonymous(node.id().index()))
    })
}

#[test]
fn analysis_dc_holds_in_an_op() {
    // FIX-11 AC 1: in an operating point `analysis(Dc)` is true (1 V) and the three others false.
    let mut running = project().compile("KindsBench").unwrap_or_else(|r| panic!("{r}"));
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    let volts = kinds_of(&running.ir, |id| solved.get_node(id).expect("a kind node"));
    for (volt, wanted) in volts.iter().zip([1.0, 0.0, 0.0, 0.0]) {
        ToleranceContract::DC_VOLTAGE.assert(&Point::new("analysis op", "v(kind)", Coordinate::OperatingPoint), wanted, *volt);
    }
}

#[test]
fn analysis_transient_holds_in_a_transient() {
    // FIX-11 AC 2: in a transient `analysis(Transient)` is true in its operating point (the first
    // point) and in its steps (the last), and the three others false.
    let built = build(&project(), "KindsBench");
    let (_, result) = built.run_with(1e-6, 0.1e-6);
    let points: Vec<_> = result.iter().collect();
    assert!(points.len() > 2, "the transient takes steps");
    for point in [points[0], points[points.len() - 1]] {
        let volts = kinds_of(&built.ir, |id| point.get_node(id).expect("a kind node"));
        for (volt, wanted) in volts.iter().zip([0.0, 1.0, 0.0, 0.0]) {
            ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("analysis transient", "v(kind)", Coordinate::Time(point.time())), wanted, *volt);
        }
    }
}

#[test]
fn analysis_ac_holds_in_an_ac_run() {
    // FIX-11 AC 3: `c` of the load is 3m under Dc, 1m under Ac, 2m otherwise; with `1m v + c v^2`
    // fed 1 mA, the operating point is v0 with c v0^2 + 1m v0 = 1m and the AC gain 2 c v0 + 1m.
    // With Ac true in the operating point and the sweep and Dc false in both, c = 1m everywhere:
    // v0 = (sqrt(5) - 1) / 2, the gain is sqrt(5) * 1m, and |v(a)| = 1m / (sqrt(5) * 1m).
    let mut running = project().compile("AcKindBench").unwrap_or_else(|r| panic!("{r}"));
    let a = NodeIdentifier::Anonymous(running.ir.node_at("AcKindBench.a").expect("node a").id().index());
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac
        .solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e6, steps: 4, logarithmic: true })
        .unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    for step in sweep.iter() {
        let actual = step.get_node(&a).expect("v(a)").norm();
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("analysis ac", "|v(a)|", Coordinate::Frequency(step.frequency)), 1.0 / 5.0f64.sqrt(), actual);
    }
}

#[test]
fn a_reaction_reads_the_analysis_kind() {
    // FIX-11 AC 5: a reaction in a transient reads `Transient`: `tr` goes true, the others stay false.
    let seen = build(&project(), "KindReactionBench").run(3e-9, 0.5e-9);
    let value = |net: &str| {
        let s = series(&seen, &format!("KindReactionBench.{net}"));
        value_at(&s, 3_000_000).to_vec()
    };
    assert_eq!(value("tr"), support::HIGH.to_vec());
    for net in ["dc", "ac", "noise"] {
        assert_eq!(value(net), support::LOW.to_vec(), "{net}");
    }
}
