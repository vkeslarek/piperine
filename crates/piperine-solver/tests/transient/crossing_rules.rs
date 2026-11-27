//! Crossings across operating points and the run errors of crossings and step caps
//! (feature `solver-crossings`, SCX-02, SCX-03; docs/spec Part IV §13 rule 5, Part V §16.1
//! rule 2, §17.2 rule 5, §17.3): a `CROSS` watch compares successive solutions, so it fires in
//! the round of an operating point that moves it above zero, and at a later operating point of
//! the same circuit; an invalid `ttol` or `etol` is `E0510`, an invalid step cap `E0509`.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::crossing_rules::{Riser, ToleranceWatch};
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use std::cell::RefCell;
use std::rc::Rc;

use piperine_abi::layout::{HIGH, LOW};
use piperine_abi::raw::Module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

// ─── SCX-02 ─────────────────────────────────────────────────────────────────

#[test]
fn a_crossing_between_operating_point_rounds_fires_in_the_later_round() {
    // SCX-02 AC 6 (§13 rule 5: two rounds of an operating point, "fires in the round that
    // follows"): round 1 solves the riser at 0 V, below the comparator's 0.5 V; the START
    // reaction raises it; round 2 solves 1 V. The comparator fires once, in the operating
    // point, and `out` is HIGH after it.
    let mut b = CircuitBuilder::new("rounds");
    let gnd = b.ground();
    let a = b.node("a");
    let r = b.node("r");
    b.device(CrossingRulesCases::riser(), "src", &[], &[Port::Node(a.clone())]);
    CircuitBuilding::add_voltage_source(&mut b, "vr", 0.5, &r, &gnd);
    b.device(mixed::Comparator::module(), "cmp", &[], &[Port::Node(a), Port::Node(r), CrossingRulesCases::net("out")]);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut solver = Solver::new(b.build().expect("build")).with_sink(Box::new(Collect(Rc::clone(&seen)))).build();
    solver.dc().expect("dc").solve().expect("the operating point");
    let fired: Vec<Observation> =
        seen.borrow().iter().filter(|o| o.kind == ObservationKind::Crossing && o.path == "cmp.up").cloned().collect();
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert_eq!(fired[0].analysis, AnalysisKind::OperatingPoint);
    assert_eq!(mixed::ReactionCounts::of("comparator"), 1);
    let bound = solver.circuit().bound();
    assert_eq!(bound.digital.read(bound.names.nets["net:out"]), HIGH);
}

#[test]
fn a_crossing_between_two_operating_points_fires_at_the_later() {
    // SCX-02 AC 7 (§16.1 rule 2: "cross compares the last solution of one point with the first
    // of the next"): a source at 0.3 V against 0.5 V, then set to 0.7 V for the next operating
    // point of the same circuit. Nothing fires at the first, the comparator fires at the second.
    let mut b = CircuitBuilder::new("points");
    let gnd = b.ground();
    let a = b.node("a");
    let r = b.node("r");
    CircuitBuilding::add_voltage_source(&mut b, "va", 0.3, &a, &gnd);
    CircuitBuilding::add_voltage_source(&mut b, "vr", 0.5, &r, &gnd);
    b.device(mixed::Comparator::module(), "cmp", &[], &[Port::Node(a), Port::Node(r), CrossingRulesCases::net("out")]);
    let mut circuit = b.build().expect("build");
    circuit.dc(Context::default()).expect("dc").solve().expect("point 1");
    let out = |c: &piperine_solver::prelude::CircuitInstance| {
        let bound = c.bound();
        bound.digital.read(bound.names.nets["net:out"]).to_vec()
    };
    assert_eq!(mixed::ReactionCounts::of("comparator"), 0, "0.3 V: nothing fires");
    assert_eq!(out(&circuit), LOW);
    circuit.set("va", "dc", ParamValue::Real(0.7)).expect("set");
    circuit.dc(Context::default()).expect("dc").solve().expect("point 2");
    assert_eq!(mixed::ReactionCounts::of("comparator"), 1, "0.3 V then 0.7 V: it fires at the later point");
    assert_eq!(out(&circuit), HIGH);
}

#[test]
fn an_invalid_ttol_or_etol_fails_the_analysis_with_e0510() {
    // SCX-03 AC 8 (§17.3 resolution): a ttol or etol that is not finite and positive
    for which in ["ttol", "etol"] {
        for bad in [0.0, -1.0e-12, f64::NAN, f64::INFINITY] {
            let err = CrossingRulesCases::failing(|b, top| {
                let gnd = b.ground();
                let r = b.node("r");
                CircuitBuilding::add_voltage_source(b, "vr", 0.5, &r, &gnd);
                b.device(CrossingRulesCases::tol_comparator(), "cmp", &[(which, ParamValue::Real(bad))], &[Port::Node(top), Port::Node(r)]);
            });
            assert!(
                err.contains("E0510") && err.contains("`cmp`") && err.contains("`rise`") && err.contains(which),
                "{which} = {bad}: {err:?}"
            );
        }
    }
    let ok = CrossingRulesCases::failing(|b, top| {
        let gnd = b.ground();
        let r = b.node("r");
        CircuitBuilding::add_voltage_source(b, "vr", 0.5, &r, &gnd);
        b.device(CrossingRulesCases::tol_comparator(), "cmp", &[], &[Port::Node(top), Port::Node(r)]);
    });
    assert_eq!(ok, "", "1 ps and 1 ps are valid");
}

#[test]
fn an_invalid_step_cap_fails_the_analysis_with_e0509() {
    // SCX-03 AC 9 (§17.2 rule 5 and its resolution): a step cap the watch holds that is not
    // finite and positive. At the solver `+∞` is the internal encoding of a `bound_step` not
    // executed at this solution and caps nothing (abi.md, the `CAP` row); an executed
    // `bound_step(inf)` never reaches the watch, the generated code faults it as E0509 first
    // (`source_environment::a_non_positive_bound_step_is_e0509`).
    for bad in [0.0, -1.0e-9, f64::NAN, f64::NEG_INFINITY] {
        let err = CrossingRulesCases::failing(|b, top| {
            b.device(mixed::Capped::module(), "cc", &[("cap", ParamValue::Real(bad))], &[Port::Node(top)]);
        });
        assert!(err.contains("E0509") && err.contains("`cc`") && err.contains("watch `cap`"), "cap {bad}: {err:?}");
    }
    let ok = CrossingRulesCases::failing(|b, top| {
        b.device(mixed::Capped::module(), "cc", &[("cap", ParamValue::Real(1.0e-6))], &[Port::Node(top)]);
    });
    assert_eq!(ok, "", "1 µs is valid");
    let infinite = CrossingRulesCases::failing(|b, top| {
        b.device(mixed::Capped::module(), "cc", &[("cap", ParamValue::Real(f64::INFINITY))], &[Port::Node(top)]);
    });
    assert_eq!(infinite, "", "the watch encodes `not executed` as +∞: it caps nothing");
}

/// The helpers of the crossing_rules tests.
pub struct CrossingRulesCases;

impl CrossingRulesCases {
    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The riser helper `riser`.
    fn riser() -> &'static Module {
        Riser::module()
    }

    /// The tol comparator helper `tol_comparator`.
    fn tol_comparator() -> &'static Module {
        ToleranceWatch::module()
    }

    // ─── SCX-03 ─────────────────────────────────────────────────────────────────

    /// A transient of the 1 V 1 kHz sine with `add` placed on it; the error text.
    fn failing(add: impl FnOnce(&mut CircuitBuilder, piperine_solver::abi::AnalogReference)) -> String {
        let mut b = CircuitBuilder::new("failing");
        let gnd = b.ground();
        let top = b.node("top");
        b.device(circuit::SineSource::module(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
            Port::Node(top.clone()),
            Port::Node(gnd),
        ]);
        add(&mut b, top);
        let mut solver =
            Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(1.0e-4, 1.0e-6)).build();
        match solver.tran().expect("tran").solve() {
            Ok(_) => String::new(),
            Err(e) => e.to_string(),
        }
    }
}
