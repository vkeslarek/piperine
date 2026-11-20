//! `inst.name` and `p.inflow()` run from MSL source on the Rust backend (feature
//! `analog-structure`, AS-11, AS-13): each module is written in `tests/structure/reach.msl`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the criterion it cites, with its closed form beside the assertion.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
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

use support::build;

/// The project of `tests/structure/reach.msl`.
fn project() -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/reach.msl");
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

/// `v(o)` of the bench `top` over a transient to `stop` with the step cap `step`, as a waveform.
fn transient(top: &str, stop: f64, step: f64) -> Waveform {
    let built = build(&project(), top);
    let out = node(&built.ir, top, "o");
    let (_, result) = built.run_with(stop, step);
    let steps: Vec<_> = result.iter().collect();
    Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(o)")).collect())
}

#[test]
fn the_parent_probes_a_pub_node_of_its_child() {
    // AS-11 AC 1: `Half` drives its `pub` node `mid` at 0.5 * v(a) = 0.5 * 2 V = 1.0 V, and the parent
    // reads it as `v(c.mid)`: `v(o)` = 1.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("probe", "v(o)", Coordinate::OperatingPoint), 1.0, operating_point("ProbeBench"));
}

#[test]
fn the_parent_loads_a_pub_node_of_its_child() {
    // AS-11 AC 2: `Series` is 1 kOhm from `a` (2 V) to `mid`; the parent adds 1 kOhm from `c.mid` to
    // ground as its own branch: a divider, `v(c.mid)` = 2 V * 1k / (1k + 1k) = 1.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("load", "v(o)", Coordinate::OperatingPoint), 1.0, operating_point("LoadBench"));
}

#[test]
fn the_parent_reads_a_pub_carrier_of_its_child() {
    // AS-11 AC 3: the child's `state` is 1.0 until 1 us and 3.0 from then on, and the parent reads it
    // through `known_or`: `v(o)` = 1.0 before the change and 3.0 after.
    let waveform = transient("CarrierReadBench", 2e-6, 0.05e-6);
    for (time, wanted) in [(0.5e-6, 1.0), (0.9e-6, 1.0), (1.1e-6, 3.0), (2.0e-6, 3.0)] {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("carrier read", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}

#[test]
fn the_parent_reads_a_pub_param_and_a_pub_let_of_its_child() {
    // AS-11 AC 4 (§8.2.25): the placement gives `r` = k = 4.0; the child's `let e = r * 3.0` is
    // evaluated in the reader's context: 12.0. `v(o)` = `c.r + c.e` = 4 + 12 = 16.0 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("param and let", "v(o)", Coordinate::OperatingPoint), 16.0, operating_point("ParamBench"));
}
