//! Correctness fixes run from MSL source on the Rust backend (feature `msl-fixes`): one test per
//! criterion, each module written in `tests/fixes/`, elaborated by the front end, compiled on the
//! transpiled backend and run on the solver. Every expected value is written from the criterion
//! it cites, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

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
