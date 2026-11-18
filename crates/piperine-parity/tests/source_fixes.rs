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

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::{HIGH, LOW, build, transient};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

fn project(file: &str) -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixes").join(file);
    Project::new(&path)
}

/// The 16 bytes of a known `f64` carrier value: the value, then the known byte.
fn real_bytes(v: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

/// What `top` of `file` commits to the carrier `path` in a 10 ns transient: the one change at
/// the first rising edge (2.5 ns).
fn first_commit(file: &str, top: &str, path: &str) -> Vec<(i64, Vec<u8>)> {
    transient(&build(&project(file), top).run(10e-9, 1e-9), path)
}

#[test]
fn an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction() {
    let seen = build(&project("enum_order.msl"), "OrderBench").run(10e-9, 1e-9);
    // FIX-07 AC 1: for `enum Md { Lo, Hi }`, `Lo < Hi` is true (`lt` goes false to true) and
    // `Hi <= Lo` is false (`le` goes true to false), at the first rising edge, 2.5 ns.
    assert_eq!(transient(&seen, "OrderBench.lt"), vec![(5 * NS / 2, HIGH.to_vec())]);
    assert_eq!(transient(&seen, "OrderBench.le"), vec![(5 * NS / 2, LOW.to_vec())]);
}

#[test]
fn two_analog_blocks_act_as_one() {
    // FIX-08 AC 1: two blocks each a 0.5 mA source into 1 kΩ: v(a) = 2 · 0.5 mA · 1 kΩ. The sources
    // read a param (`ia`) because the literal `0.5m` is B1, FIX-04 (T22).
    let mut running = project("two_blocks.msl").compile("Bench").unwrap_or_else(|r| panic!("{r}"));
    let a = running.ir.node_at("Bench.a").expect("node a");
    let id = NodeIdentifier::Anonymous(a.id().index());
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    let actual = solved.get_node(&id).expect("v(a)");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("two analog blocks", "v(a)", Coordinate::OperatingPoint), 1.0, actual);
}

#[test]
fn left_out_optional_arguments_are_none() {
    // FIX-06 AC 1: `f(1.0)` is 1 + 10 + 100.
    assert_eq!(first_commit("optional_args.msl", "LeftOutBench", "LeftOutBench.r"), vec![(5 * NS / 2, real_bytes(111.0))]);
}

#[test]
fn a_named_optional_argument_binds_by_name() {
    let seen = build(&project("optional_args.msl"), "NamedBench").run(10e-9, 1e-9);
    // FIX-06 AC 2: `f(1.0, z: 3.0)` is 1 + 10 + 3; the edge case `f(1.0, z: 3.0, y: 2.0)`
    // binds both by name, in any order: 1 + 2 + 3.
    assert_eq!(transient(&seen, "NamedBench.r"), vec![(5 * NS / 2, real_bytes(14.0))]);
    assert_eq!(transient(&seen, "NamedBench.both"), vec![(5 * NS / 2, real_bytes(6.0))]);
}

#[test]
fn a_plain_value_for_an_option_is_some() {
    // FIX-06 AC 3: `f(1.0, 2.0)` is 1 + 2 + 100.
    assert_eq!(first_commit("optional_args.msl", "PlainBench", "PlainBench.r"), vec![(5 * NS / 2, real_bytes(103.0))]);
}

#[test]
fn none_and_some_pass_positionally() {
    // FIX-06 AC 4: `f(1.0, None, Some(3.0))` is 1 + 10 + 3.
    assert_eq!(first_commit("optional_args.msl", "PositionalBench", "PositionalBench.r"), vec![(5 * NS / 2, real_bytes(14.0))]);
}

/// The operating point of `top` of `literal_sources.msl`: `v(a)`.
fn literal_source_op(top: &str) -> f64 {
    let mut running = project("literal_sources.msl").compile(top).unwrap_or_else(|r| panic!("{r}"));
    let a = running.ir.node_at(&format!("{top}.a")).expect("node a");
    let id = NodeIdentifier::Anonymous(a.id().index());
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    solved.get_node(&id).expect("v(a)")
}

#[test]
fn a_literal_flow_source_builds_and_runs() {
    // FIX-04 AC 1: `i(gnd, a) +> 1m;` into 1 kΩ is 1.0 V on the Rust backend.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("literal flow source", "v(a)", Coordinate::OperatingPoint), 1.0, literal_source_op("FlowBench"));
}

#[test]
fn a_literal_potential_source_builds_and_runs() {
    // FIX-04 AC 2: `v(a, gnd) +> 2.0;` is 2.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("literal potential source", "v(a)", Coordinate::OperatingPoint), 2.0, literal_source_op("PotentialBench"));
}

#[test]
fn a_carrier_driven_current_builds_and_runs() {
    // FIX-04 AC 3: `lv` committed to 2.0 at 1 ns gives 2 mA into 1 kΩ: 2.0 V after 1 ns.
    let built = build(&project("literal_sources.msl"), "CarrierBench");
    let a = NodeIdentifier::Anonymous(built.ir.node_at("CarrierBench.a").expect("node a").id().index());
    let (_, result) = built.run_with(3e-9, 0.1e-9);
    let last = result.iter().last().expect("a time point");
    assert!(last.time() > 1.5e-9, "the run passes 1 ns: {}", last.time());
    ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("carrier driven current", "v(a)", Coordinate::OperatingPoint), 2.0, last.get_node(&a).expect("v(a)"));
}

/// The instants of every record of the transient of `top` of `file`, in order.
fn record_times(file: &str, top: &str, stop: f64, step: f64) -> Vec<f64> {
    let seen = build(&project(file), top).run(stop, step);
    seen.all()
        .iter()
        .filter(|o| matches!(o.kind, ObservationKind::Record { .. }))
        .map(|o| o.time)
        .collect()
}

#[test]
fn a_cross_over_a_maybe_unknown_carrier_builds() {
    // FIX-05 AC 1: a `cross` argument that reads a carrier that may be unknown (`absdelta`'s
    // `last`, unknown until its first commit) builds on the Rust backend: `build` compiles the
    // generated source and fails on any rustc error or warning.
    let built = build(&project("absdelta.msl"), "AbsdeltaBench");
    assert!(built.compiled.circuit().is_ok(), "the compiled design makes a circuit");
}

#[test]
fn absdelta_fires_every_two_tenths_of_a_volt() {
    // FIX-05 AC 2: `v(a)` = 1 V/µs · t; `absdelta(v(a), 0.2)` writes `hit` at 0 (`init()`) and at
    // 0.2, 0.4, 0.6 and 0.8 µs, each within the solver's default `ttol` (1 ps, `Keep` docs of
    // `TransientAnalysisOptions::ttol`) of the exact time.
    let times = record_times("absdelta.msl", "AbsdeltaBench", 0.9e-6, 0.05e-6);
    let exact = [0.0, 0.2e-6, 0.4e-6, 0.6e-6, 0.8e-6];
    assert_eq!(times.len(), exact.len(), "five records: {times:?}");
    for (t, want) in times.iter().zip(exact) {
        assert!((t - want).abs() <= 1e-12, "a record at {t}, wanted {want} within 1 ps: {times:?}");
    }
}
