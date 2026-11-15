//! Scope: `Bench`, the chain from a twin's text to a bound solver circuit (requirements PAR-02 and
//! PAR-04 AC 4): a twin loads, a failing stage is named with its diagnostics, nodes and sources
//! are reached by the `.cir` names, and a skipped reference still solves the MSL side. No ngspice
//! binary is involved.

use piperine_parity::bench::Bench;
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
use piperine_solver::prelude::Context;

/// Runs a closure expected to panic and returns the panic message.
struct Panicked;

impl Panicked {
    fn message(run: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let payload = std::panic::catch_unwind(run).expect_err("the closure panics");
        payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
    }
}

#[test]
fn a_twin_loads_into_a_bound_circuit() {
    let mut bench = Bench::load("divider");
    let mid = bench.node("mid");
    let result = bench.circuit().dc(Context::default()).expect("dc").solve().expect("solve");
    let v = result.get_node(&mid).expect("v(mid)");
    assert!((v - 7.5).abs() < 1e-9, "v(mid) = {v}");
}

#[test]
fn a_failing_stage_is_named_with_its_diagnostics() {
    let directory = tempfile::tempdir().expect("temp dir");
    let file = directory.path().join("broken.msl");
    std::fs::write(&file, "module Broken() {\n    signal a: Electrical;\n    x: f64 = true;\n}\n").expect("write the twin");
    let message = Panicked::message(move || {
        Bench::load_file("broken", file);
    });
    assert!(message.contains("typeck"), "the stage is named: {message}");
    assert!(message.contains("P0002") || message.contains("E0"), "a diagnostic code is shown: {message}");
    assert!(message.contains("broken"), "the fixture is named: {message}");
}

#[test]
fn nodes_and_sources_are_reached_by_cir_names() {
    let mut bench = Bench::load("divider");
    assert_eq!(bench.path("r1"), "Divider.r1");
    let vin = bench.node("vin");
    let source = bench.source("v1");
    let result = bench.circuit().dc(Context::default()).expect("dc").solve().expect("solve");
    assert!((result.get_node(&vin).expect("v(vin)") - 10.0).abs() < 1e-9);
    // 10 V across 4 kOhm: 2.5 mA; the flow of the source is the current through it
    let flow = result.get_branch(source).expect("the source's flow");
    assert!((flow.abs() - 2.5e-3).abs() < 1e-9, "flow = {flow}");
}

#[test]
fn a_skipped_reference_still_solves_the_msl_side() {
    let reference = Ngspice::new(Lookup::Skip).run("divider");
    assert!(matches!(reference, Reference::Skipped));
    let mut compared = 0;
    let mut bench = Bench::load("divider");
    let mid = bench.node("mid");
    let result = bench.circuit().dc(Context::default()).expect("the solve succeeds under skip").solve().expect("solve");
    assert!(result.get_node(&mid).is_some());
    if let Reference::Golden(golden) = &reference {
        compared += golden.stdout().len();
    }
    assert_eq!(compared, 0, "nothing is compared under skip");
    // a failing solve still panics: a twin that fails to load does
    let message = Panicked::message(|| {
        Bench::load("no_such_twin");
    });
    assert!(!message.is_empty());
}
