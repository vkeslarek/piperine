//! Correctness fixes run from MSL source on the Rust backend (feature `msl-fixes`): one test per
//! criterion, each module written in `tests/fixes/`, elaborated by the front end, compiled on the
//! transpiled backend and run on the solver. Every expected value is written from the criterion
//! it cites, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};
use piperine_solver::protocol::ObservationKind;

use piperine_parity::mixed;

use mixed::Seen;
use piperine_parity::buck_fixture::{BuckFixture, HIGH, LOW};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// Associated test helpers of `fixes`, grouped on one fixture type.
struct FixesFixture;

impl FixesFixture {
    fn project(file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixes").join(file);
        Project::new(&path)
    }

    /// The eight bytes of an `f64` carrier value.
    fn real_bytes(v: f64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// What `top` of `file` commits to the carrier `path` in a 10 ns transient: the one change at
    /// the first rising edge (2.5 ns).
    fn first_commit(file: &str, top: &str, path: &str) -> Vec<(i64, Vec<u8>)> {
        BuckFixture::transient(&BuckFixture::build(&FixesFixture::project(file), top).run(10e-9, 1e-9), path)
    }

    /// The operating point of `top` of `literal_sources.msl`: `v(a)`.
    fn literal_source_op(top: &str) -> f64 {
        let mut running = FixesFixture::project("literal_sources.msl").compile(top).unwrap_or_else(|r| panic!("{r}"));
        let a = running.ir.node_at(&format!("{top}.a")).expect("node a");
        let id = NodeIdentifier::Anonymous(a.id().index());
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        solved.get_node(&id).expect("v(a)")
    }

    /// The instants of every record of the transient of `top` of `file`, in order.
    fn record_times(file: &str, top: &str, stop: f64, step: f64) -> Vec<f64> {
        let seen = BuckFixture::build(&FixesFixture::project(file), top).run(stop, step);
        seen.all()
            .iter()
            .filter(|o| matches!(o.kind, ObservationKind::Record { .. }))
            .map(|o| o.time)
            .collect()
    }

    /// The instant (seconds) of the one change of `hit` in `top` of `cross_tol.msl`, run to 3 us.
    fn rc_crossing(top: &str) -> f64 {
        let seen = BuckFixture::build(&FixesFixture::project("cross_tol.msl"), top).run(3e-6, 0.05e-6);
        let changes = BuckFixture::transient(&seen, &format!("{top}.hit"));
        assert_eq!(changes.len(), 1, "{top}: one crossing: {changes:?}");
        changes[0].0 as f64 * 1e-15
    }

    /// The instant of the last step of the transient of `top` of `finish.msl`, run to 4 ns.
    fn last_step(top: &str) -> (Seen, f64) {
        let (seen, result) = BuckFixture::build(&FixesFixture::project("finish.msl"), top).run_with(4e-9, 0.5e-9);
        let last = result.iter().last().expect("a time point").time();
        (seen, last)
    }

    /// The instants of every record the run observed.
    fn all_record_instants(seen: &Seen) -> Vec<f64> {
        seen.all().iter().filter(|o| matches!(o.kind, ObservationKind::Record { .. })).map(|o| o.time).collect()
    }
}

#[test]
fn an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction() {
    let seen = BuckFixture::build(&FixesFixture::project("enum_order.msl"), "OrderBench").run(10e-9, 1e-9);
    // FIX-07 AC 1: for `enum Md { Lo, Hi }`, `Lo < Hi` is true (`lt` goes false to true) and
    // `Hi <= Lo` is false (`le` goes true to false), at the first rising edge, 2.5 ns.
    assert_eq!(BuckFixture::transient(&seen, "OrderBench.lt"), vec![(5 * NS / 2, HIGH.to_vec())]);
    assert_eq!(BuckFixture::transient(&seen, "OrderBench.le"), vec![(5 * NS / 2, LOW.to_vec())]);
}

#[test]
fn two_analog_blocks_act_as_one() {
    // FIX-08 AC 1: two blocks each a 0.5 mA source into 1 kΩ: v(a) = 2 · 0.5 mA · 1 kΩ. The sources
    // read a param (`ia`) because the literal `0.5m` is B1, FIX-04 (T22).
    let mut running = FixesFixture::project("two_blocks.msl").compile("Bench").unwrap_or_else(|r| panic!("{r}"));
    let a = running.ir.node_at("Bench.a").expect("node a");
    let id = NodeIdentifier::Anonymous(a.id().index());
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    let actual = solved.get_node(&id).expect("v(a)");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("two analog blocks", "v(a)", Coordinate::OperatingPoint), 1.0, actual);
}

#[test]
fn left_out_optional_arguments_are_none() {
    // FIX-06 AC 1: `f(1.0)` is 1 + 10 + 100.
    assert_eq!(FixesFixture::first_commit("optional_args.msl", "LeftOutBench", "LeftOutBench.r"), vec![(5 * NS / 2, FixesFixture::real_bytes(111.0))]);
}

#[test]
fn a_named_optional_argument_binds_by_name() {
    let seen = BuckFixture::build(&FixesFixture::project("optional_args.msl"), "NamedBench").run(10e-9, 1e-9);
    // FIX-06 AC 2: `f(1.0, z: 3.0)` is 1 + 10 + 3; the edge case `f(1.0, z: 3.0, y: 2.0)`
    // binds both by name, in any order: 1 + 2 + 3.
    assert_eq!(BuckFixture::transient(&seen, "NamedBench.r"), vec![(5 * NS / 2, FixesFixture::real_bytes(14.0))]);
    assert_eq!(BuckFixture::transient(&seen, "NamedBench.both"), vec![(5 * NS / 2, FixesFixture::real_bytes(6.0))]);
}

#[test]
fn a_plain_value_for_an_option_is_some() {
    // FIX-06 AC 3: `f(1.0, 2.0)` is 1 + 2 + 100.
    assert_eq!(FixesFixture::first_commit("optional_args.msl", "PlainBench", "PlainBench.r"), vec![(5 * NS / 2, FixesFixture::real_bytes(103.0))]);
}

#[test]
fn none_and_some_pass_positionally() {
    // FIX-06 AC 4: `f(1.0, None, Some(3.0))` is 1 + 10 + 3.
    assert_eq!(FixesFixture::first_commit("optional_args.msl", "PositionalBench", "PositionalBench.r"), vec![(5 * NS / 2, FixesFixture::real_bytes(14.0))]);
}

#[test]
fn a_literal_flow_source_builds_and_runs() {
    // FIX-04 AC 1: `i(gnd, a) +> 1m;` into 1 kΩ is 1.0 V on the Rust backend.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("literal flow source", "v(a)", Coordinate::OperatingPoint), 1.0, FixesFixture::literal_source_op("FlowBench"));
}

#[test]
fn a_literal_potential_source_builds_and_runs() {
    // FIX-04 AC 2: `v(a, gnd) +> 2.0;` is 2.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("literal potential source", "v(a)", Coordinate::OperatingPoint), 2.0, FixesFixture::literal_source_op("PotentialBench"));
}

#[test]
fn a_zero_argument_function_builds_and_runs() {
    // A function called with no argument has nothing to fix its `Real` type: `fn two() -> f64 { 2.0 }`
    // in `v(a, gnd) +> two();` is 2.0 V (it was a `rustc` E0283), and in a reaction commits 2.0.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("zero-argument function", "v(a)", Coordinate::OperatingPoint), 2.0, FixesFixture::literal_source_op("FnBench"));
    let seen = BuckFixture::build(&FixesFixture::project("literal_sources.msl"), "FnReactionBench").run(3e-9, 0.5e-9);
    let last = BuckFixture::transient(&seen, "FnReactionBench.lv").pop().expect("a commit").1;
    assert_eq!(&last[..8], &2.0f64.to_le_bytes(), "{last:?}");
}

#[test]
fn a_carrier_driven_current_builds_and_runs() {
    // FIX-04 AC 3: `lv` committed to 2.0 at 1 ns gives 2 mA into 1 kΩ: 2.0 V after 1 ns.
    let built = BuckFixture::build(&FixesFixture::project("literal_sources.msl"), "CarrierBench");
    let a = NodeIdentifier::Anonymous(built.ir.node_at("CarrierBench.a").expect("node a").id().index());
    let (_, result) = built.run_with(3e-9, 0.1e-9);
    let last = result.iter().last().expect("a time point");
    assert!(last.time() > 1.5e-9, "the run passes 1 ns: {}", last.time());
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("carrier driven current", "v(a)", Coordinate::OperatingPoint), 2.0, last.get_node(&a).expect("v(a)"));
}

#[test]
fn a_cross_over_a_maybe_unknown_carrier_builds() {
    // FIX-05 AC 1: a `cross` argument that reads a carrier that may be unknown (`absdelta`'s
    // `last`, unknown until its first commit) builds on the Rust backend: `build` compiles the
    // generated source and fails on any rustc error or warning.
    let built = BuckFixture::build(&FixesFixture::project("absdelta.msl"), "AbsdeltaBench");
    assert!(built.compiled.circuit().is_ok(), "the compiled design makes a circuit");
}

#[test]
fn absdelta_fires_every_two_tenths_of_a_volt() {
    // FIX-05 AC 2: `v(a)` = 1 V/µs · t; `absdelta(v(a), 0.2)` writes `hit` at 0 (`init()`) and at
    // 0.2, 0.4, 0.6 and 0.8 µs, each within the solver's default `ttol` (1 ps, `SeenSink` docs of
    // `TransientAnalysisOptions::ttol`) of the exact time.
    let times = FixesFixture::record_times("absdelta.msl", "AbsdeltaBench", 0.9e-6, 0.05e-6);
    let exact = [0.0, 0.2e-6, 0.4e-6, 0.6e-6, 0.8e-6];
    assert_eq!(times.len(), exact.len(), "five records: {times:?}");
    for (t, want) in times.iter().zip(exact) {
        assert!((t - want).abs() <= 1e-12, "a record at {t}, wanted {want} within 1 ps: {times:?}");
    }
}

#[test]
fn a_cross_over_a_nan_of_known_values_fails_the_solve() {
    // FIX-05 (Q2 amends abi.md A4 for an unknown value only): only an unknown value is NaN to the solver; `ln` of a negative
    // number, from known inputs, is a defect and fails the transient loud, naming the watch.
    let built = BuckFixture::build(&FixesFixture::project("cross_nan.msl"), "NanBench");
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = piperine_solver::prelude::TransientAnalysisOptions::new(0.5e-6, 0.1e-6);
    options.dt_max = 0.1e-6;
    let mut solver = piperine_solver::prelude::Solver::new(circuit).with_tran_opts(options).build();
    let err = solver.tran().and_then(|mut t| t.solve()).err().expect("the run fails").to_string();
    assert!(err.contains("not finite") && err.contains("abi.md A4"), "{err}");
}

#[test]
fn cross_with_tolerances_runs() {
    // FIX-12 AC 1: `cross(e, 1p)`, `cross(e, etol: 1u)` and `cross(e, 1p, 1u)` type-check, build
    // and run: each fires once, at the 0.5 V of a 1 V/µs ramp (0.5 µs), within the solver's default
    // `ttol` (1 ps).
    let seen = BuckFixture::build(&FixesFixture::project("cross_tol.msl"), "SpellingsBench").run(0.8e-6, 0.05e-6);
    for net in ["one", "two", "three"] {
        let changes = BuckFixture::transient(&seen, &format!("SpellingsBench.{net}"));
        assert_eq!(changes.len(), 1, "{net} fires once: {changes:?}");
        let at = changes[0].0 as f64 * 1e-15;
        assert!((at - 0.5e-6).abs() <= 1e-12, "{net} at {at}, wanted 0.5 µs within 1 ps");
    }
}

#[test]
fn a_cross_with_ttol_fires_within_it() {
    // FIX-12 AC 2 and 3: R = 1 kΩ, C = 1 nF charged by a 1 V step crosses 0.5 V at RC · ln 2;
    // with `ttol` = 1 ns the watch fires within 1 ns of it.
    let seen = BuckFixture::build(&FixesFixture::project("cross_tol.msl"), "RcBench").run(3e-6, 0.05e-6);
    let changes = BuckFixture::transient(&seen, "RcBench.hit");
    assert_eq!(changes.len(), 1, "one crossing: {changes:?}");
    let at = changes[0].0 as f64 * 1e-15;
    let wanted = 1e3 * 1e-9 * std::f64::consts::LN_2;
    assert!((at - wanted).abs() <= 1e-9, "fired at {at}, wanted {wanted} within 1 ns");
}

#[test]
fn the_ttol_given_to_a_cross_reaches_the_solver() {
    // FIX-12 AC 2 and 3, end to end: the tolerance is written to the watch's pin, so a 300 ns
    // `ttol` locates the same crossing (RC · ln 2) at a different instant than a 1 ps one, and a
    // `ttol` that is not positive fails the run with E0510 (§17.3).
    let tight = FixesFixture::rc_crossing("TightBench");
    let loose = FixesFixture::rc_crossing("LooseBench");
    let exact = 1e3 * 1e-9 * std::f64::consts::LN_2;
    assert!((tight - exact).abs() <= 1e-9, "the 1 ps ttol locates {tight}, exact {exact}");
    assert!((loose - tight).abs() > 1e-9, "the 300 ns ttol locates {loose}, no different from {tight}");
    assert!((loose - exact).abs() <= 300e-9, "the 300 ns ttol locates {loose}, within 300 ns of {exact}");
}

#[test]
fn a_cross_ttol_that_is_not_positive_is_e0510() {
    // FIX-12 AC 4: a `ttol` of -1 ns fails the run with E0510, naming the watch.
    let built = BuckFixture::build(&FixesFixture::project("cross_tol.msl"), "NegativeTolBench");
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = piperine_solver::prelude::TransientAnalysisOptions::new(1e-6, 0.1e-6);
    options.dt_max = 0.1e-6;
    let mut solver = piperine_solver::prelude::Solver::new(circuit).with_tran_opts(options).build();
    let err = solver.tran().and_then(|mut t| t.solve()).err().expect("the run fails").to_string();
    assert!(err.contains("E0510") && err.contains("ttol"), "{err}");
}

#[test]
fn above_with_an_emit_writes_one_record_at_its_crossing() {
    // FIX-13 AC 1: `on above(v(a) - 0.5) { emit!("hit", x = 1); }` on a 1 V/µs ramp to 1 µs writes
    // exactly one record, within the default `ttol` (1 ps) of 0.5 µs.
    let times = FixesFixture::record_times("fired.msl", "RampBench", 1e-6, 0.05e-6);
    assert_eq!(times.len(), 1, "one record: {times:?}");
    assert!((times[0] - 0.5e-6).abs() <= 1e-12, "at {}, wanted 0.5 µs within 1 ps", times[0]);
}

#[test]
fn above_true_from_the_start_writes_one_record_at_zero() {
    // FIX-13 AC 2: the same reaction on `v(a)` = 1 V from the start writes one record, at time 0.
    let times = FixesFixture::record_times("fired.msl", "HeldBench", 1e-7, 0.05e-6);
    assert_eq!(times, vec![0.0], "one record at 0");
}

#[test]
fn two_disjuncts_of_one_microstep_run_the_body_once() {
    // FIX-13 AC 3: `(init() && v(a) > 0.0) || (init() && v(a) > -1.0)` at 1 V: both disjuncts fire
    // in the first microstep and the body runs once.
    let times = FixesFixture::record_times("fired.msl", "TwiceBench", 1e-7, 0.05e-6);
    assert_eq!(times, vec![0.0], "one record");
}

#[test]
fn finish_ends_the_transient_at_its_instant() {
    // FIX-01 AC 1: `on at(1n) { finish(); } on at(3n) { k <- 9; emit!("late", x = 1); }` run to
    // 4 ns: the last step is at 1 ns, there is no `late` record and `k` is still 0.
    let (seen, last) = FixesFixture::last_step("FinishBench");
    assert!((last - 1e-9).abs() <= 1e-15, "last step at {last}, wanted 1 ns");
    assert!(FixesFixture::all_record_instants(&seen).iter().all(|t| *t <= 1e-9 + 1e-15), "no record after 1 ns (`late` is at 3 ns)");
    assert!(BuckFixture::transient(&seen, "FinishBench.k").iter().all(|(_, b)| b[..8] != 9f64.to_le_bytes()), "k stays 0");
}

#[test]
fn fatal_ends_the_transient_at_its_instant() {
    // FIX-01 AC 2: `fatal("boom")` ends the transient at 1 ns as `finish` does.
    let (seen, last) = FixesFixture::last_step("FatalBench");
    assert!((last - 1e-9).abs() <= 1e-15, "last step at {last}, wanted 1 ns");
    assert!(FixesFixture::all_record_instants(&seen).iter().all(|t| *t <= 1e-9 + 1e-15), "no record after 1 ns (`late` is at 3 ns)");
}

#[test]
fn a_finish_field_false_or_unknown_runs_to_stop() {
    // FIX-01 AC 3: a `sim` record with `finish` false (at 1 ns) or unknown (at 2 ns) leaves the
    // run to its `stop` (4 ns), and the reaction at 3 ns still writes `k`.
    let (seen, last) = FixesFixture::last_step("IdleBench");
    assert!((last - 4e-9).abs() <= 1e-15, "last step at {last}, wanted 4 ns");
    assert!(BuckFixture::transient(&seen, "IdleBench.k").iter().any(|(_, b)| b[..8] == 9f64.to_le_bytes()), "k reaches 9");
}

#[test]
fn finish_ends_an_operating_point() {
    // FIX-01 AC 4: `finish()` on the first instant (`init()`) ends the analysis at 0 and the
    // reaction of 3 ns never runs.
    let (seen, last) = FixesFixture::last_step("EarlyBench");
    assert!(last <= 1e-15, "last step at {last}, wanted 0");
    assert!(BuckFixture::transient(&seen, "EarlyBench.k").iter().all(|(_, b)| b[..8] != 9f64.to_le_bytes()), "k stays 0");
}
