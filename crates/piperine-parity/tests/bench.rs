//! Scope: `Bench`, the chain from a twin's text to a bound solver circuit (requirements PAR-02 and
//! PAR-04 AC 4): a twin loads, a failing stage is named with its diagnostics, nodes and sources
//! are reached by the `.cir` names, and a skipped reference still solves the MSL side (that one test also runs ngspice once, for the
//! compared count of a real run). The other tests involve no ngspice binary.

use piperine_parity::bench::Bench;
use piperine_parity::ngspice::{Lookup, Ngspice};
use piperine_parity::operating_point::{OperatingPoint, Outcome};
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

/// A twin that fails in one stage: its text, the stage, the exact code, and the text the span covers.
struct Plant {
    stage: &'static str,
    source: &'static str,
    code: &'static str,
    covered: &'static str,
}

impl Plant {
    const ALL: [Plant; 3] = [
        Plant { stage: "parse", source: "module Broken() {\n    signal a Electrical;\n}\n", code: "E0900", covered: "Electrical" },
        Plant { stage: "typeck", source: "module Broken() {\n    signal a: Electrical;\n    x: f64 = true;\n}\n", code: "E0180", covered: "true" },
        Plant {
            stage: "elaborate",
            source: "module Broken() {\n    signal a: Electrical;\n    let c = Cap { c: -1.0 }(a, gnd);\n}\nmodule Cap(p: Electrical, n: Electrical) { c: f64 where c >= 0.0; }\nanalog for Cap { i(p, n) +> ddt(c * v(p, n)); }\n",
            code: "E0317",
            covered: "-1.0",
        },
    ];

    /// The message of loading this plant as the twin `broken`.
    fn message(&self) -> String {
        let directory = tempfile::tempdir().expect("temp dir");
        let file = directory.path().join("broken.msl");
        std::fs::write(&file, self.source).expect("write the twin");
        Panicked::message(move || {
            Bench::load_file("broken", file);
        })
    }
}

#[test]
fn a_failing_stage_is_named_with_its_diagnostics() {
    for plant in Plant::ALL {
        let message = plant.message();
        let start = plant.source.find(plant.covered).expect("the plant holds its covered text");
        let end = start + plant.covered.len();
        assert!(message.starts_with("broken: "), "the fixture is named first: {message}");
        assert!(message.contains(&format!("stage `{}` failed with 1 diagnostic(s)", plant.stage)), "the stage is {}: {message}", plant.stage);
        assert!(message.contains(&format!("\n{} (file ", plant.code)), "the exact code {} is shown: {message}", plant.code);
        assert!(message.contains(&format!("Span {{ start: {start}, end: {end} }}")), "the span {start}..{end} of `{}` is shown: {message}", plant.covered);
    }
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
    let twin = OperatingPoint { fixture: "divider", nodes: &["vin", "mid"] };
    // the parity test's own comparison under skip: the MSL side is solved, nothing is compared
    let skipped = twin.compare(&Ngspice::new(Lookup::Skip));
    assert_eq!(skipped, Outcome { solved: 2, compared: 0 });
    // the same comparison against a real run: both nodes are solved and compared
    let lookup = Ngspice::locate(None, std::env::var_os("PATH").as_deref()).unwrap_or_else(|message| panic!("{message}"));
    let real = twin.compare(&Ngspice::new(lookup));
    assert_eq!(real, Outcome { solved: 2, compared: 2 });
    // a twin that fails to load fails under skip too
    let missing = OperatingPoint { fixture: "no_such_twin", nodes: &["a"] };
    let message = Panicked::message(move || {
        missing.compare(&Ngspice::new(Lookup::Skip));
    });
    assert!(message.contains("no_such_twin"), "{message}");
}
