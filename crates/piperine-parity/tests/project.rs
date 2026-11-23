//! `Project` (feature `msl-digital`, T2; checks C170 and the loader DIG-15 AC 3 stands on): a
//! root crate and a dependency crate read from disk (`tests/buck/divider/`), elaborated and
//! compiled on the transpiled backend, and run on the solver; a design a stage refuses comes
//! back with the name of that stage. The compiled library is built under `CARGO_TARGET_TMPDIR`.

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

fn fixture(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/buck/divider").join(file)
}

fn project(root: &str) -> Project {
    let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("project-cache");
    Project::new(&fixture(root)).dependency("devices", &fixture("devices/lib.msl")).cache(&cache)
}

#[test]
fn a_project_with_a_dependency_crate_loads_and_runs() {
    let mut running = project("bench.msl").compile("Divider").unwrap_or_else(|r| panic!("{r}"));
    assert!(running.compiled().report().warnings.is_empty(), "the generated source compiles with no warning");
    let mid = running.ir.node_at("Divider.mid").expect("the divider's mid node");
    let id = NodeIdentifier::Anonymous(mid.id().index());
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    let actual = solved.get_node(&id).expect("v(mid)");
    // 10 V across 1 k over 3 k: v(mid) = 10 · 3k / (1k + 3k).
    let wanted = 10.0 * 3e3 / (1e3 + 3e3);
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("project divider", "v(mid)", Coordinate::OperatingPoint), wanted, actual);
}

#[test]
fn a_diagnostic_is_returned_with_its_stage() {
    // A module the dependency does not have: the front end refuses it before the type check.
    let refused = match project("unresolved.msl").compile("Divider") {
        Ok(_) => panic!("an unresolved placement compiled"),
        Err(r) => r,
    };
    assert_eq!(refused.stage, "parse", "{refused}");
    assert!(!refused.diagnostics.is_empty(), "the diagnostics come back with the stage");

    // A `bool` given to an `f64` param: the type checker's stage.
    let refused = match project("typeck.msl").elaborate("Divider") {
        Ok(_) => panic!("a bool param value elaborated"),
        Err(r) => r,
    };
    assert_eq!(refused.stage, "typeck", "{refused}");
    assert!(refused.diagnostics.iter().any(|d| d.diagnostic.code == "E0180"), "{refused}");

    // A top that is not a module of the root crate: the type checker names it.
    let refused = match project("bench.msl").elaborate("Resistor") {
        Ok(_) => panic!("a dependency's module elaborated as the top"),
        Err(r) => r,
    };
    assert_eq!(refused.stage, "typeck", "{refused}");
    assert!(refused.diagnostics.iter().any(|d| d.diagnostic.code == "P0003"), "{refused}");
}

#[test]
fn the_source_runner_checks_the_whole_model() {
    // GN-09: `Divider` is clean and `broken` is called by nothing, yet the runner refuses it at the
    // type check as a host does; the same top without the function elaborates
    let refused = match project("unreached.msl").elaborate("Divider") {
        Ok(_) => panic!("an unreached type error elaborated"),
        Err(r) => r,
    };
    assert_eq!(refused.stage, "typeck", "{refused}");
    assert_eq!(refused.diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0180"]);
    assert!(project("bench.msl").elaborate("Divider").is_ok());
}
