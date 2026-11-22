//! The time-point rule as a solver option (feature `transient-performance`, TP-06 AC 3, TP-09
//! AC 1 and AC 3): an open-loop mixed design, a 200 MHz oscillator, a counter that commits on
//! every edge and a flip-flop that hands a once-per-400-edges change to a D2A into an RC, runs
//! under `Observable` (the default) and under `Declared` (the reference mode) with every
//! observation equal field for field and in time order; the declared rule lands on every clock
//! edge and the observable one only where the D2A's input changes; a located `cross` and a
//! `bound_step` cap behave the same under both.

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::RefCell;
use std::rc::Rc;

use abi_devices::{add_capacitor, add_resistor, add_vsrc, vramp};
use abi_mixed::{capped, comparator, count_of, counter, dac, dff, timer};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TimePointRule, TransientAnalysisOptions, TransientAnalysisResult};
use piperine_solver::protocol::{Observation, ObservationKind, Sink, StepEnd, StepStatistics};

/// Half a period of the 200 MHz oscillator, seconds.
const HALF_PERIOD: f64 = 2.5e-9;
/// The oscillator's edges between two changes of the D2A's input.
const EDGES_PER_CHANGE: f64 = 400.0;
/// The run: 1400 oscillator edges, a millisecond-scale RC step and a margin of one nanosecond.
const STOP: f64 = 3.5e-6 + 1.0e-9;

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

/// Keeps every observation in the order the sink is handed them.
struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, observation: &Observation) {
        self.0.borrow_mut().push(observation.clone());
    }
}

/// What one run of a circuit under one rule gave.
struct Run {
    result: TransientAnalysisResult,
    seen: Vec<Observation>,
}

impl Run {
    /// The run's step statistics.
    fn statistics(&self) -> &StepStatistics {
        self.result.statistics()
    }

    /// The accepted steps that ended on a landing, whatever its cause.
    fn landings(&self) -> u64 {
        StepEnd::ALL.iter().filter(|end| matches!(end, StepEnd::Landing(_))).map(|end| self.statistics().ended(*end)).sum()
    }
}

/// The transient of `builder`'s circuit to `stop` under `rule`, its observations kept.
fn run(mut builder: CircuitBuilder, rule: TimePointRule, stop: f64, max_step: f64) -> Run {
    builder.time_point_rule(rule);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut solver = Solver::new(builder.build().expect("build"))
        .with_context(Context::default())
        .with_tran_opts(TransientAnalysisOptions::new(stop, max_step))
        .with_sink(Box::new(Collect(Rc::clone(&seen))))
        .build();
    let result = solver.tran().expect("tran").solve().expect("solve");
    drop(solver);
    let seen = seen.borrow().clone();
    Run { result, seen }
}

/// The open-loop fixture: `osc` (200 MHz) clocks a counter and a flip-flop; `slow` toggles every
/// 400 edges and the flip-flop passes it on at the next rising edge to a D2A that charges an RC.
fn open_loop() -> CircuitBuilder {
    let mut b = CircuitBuilder::new("open loop");
    let gnd = b.ground();
    let o = b.node("o");
    let c = b.node("c");
    b.device(timer(), "osc", &[("half", ParamValue::Real(HALF_PERIOD))], &[net("clk")]);
    b.device(counter(), "cnt", &[], &[net("clk"), net("n")]);
    b.device(timer(), "slow", &[("half", ParamValue::Real(HALF_PERIOD * EDGES_PER_CHANGE))], &[net("d")]);
    b.device(dff(), "ff", &[], &[net("clk"), net("d"), net("q")]);
    b.device(dac(), "dac", &[("vref", ParamValue::Real(1.0))], &[net("q"), Port::Node(o.clone())]);
    add_resistor(&mut b, "r1", 1.0e3, &o, &c);
    add_capacitor(&mut b, "c1", 1.0e-9, &c, &gnd);
    b
}

#[test]
fn the_open_loop_fixture_gives_the_same_observations_in_both_rules() {
    // TP-09 AC 1: every change, commit and fire, instant for instant and byte for byte
    let observable = run(open_loop(), TimePointRule::Observable, STOP, 1.0e-7);
    let declared = run(open_loop(), TimePointRule::Declared, STOP, 1.0e-7);
    assert!(observable.seen.len() > 1000, "the oscillator and the counter were observed");
    assert_eq!(observable.seen.len(), declared.seen.len());
    for (index, (a, b)) in observable.seen.iter().zip(&declared.seen).enumerate() {
        assert_eq!(a, b, "observation {index} differs between the rules");
    }
}

#[test]
fn the_observations_of_both_rules_come_in_time_order() {
    // TP-09 AC 3: instants run in increasing order whichever rule decides the landings
    for rule in [TimePointRule::Observable, TimePointRule::Declared] {
        let run = run(open_loop(), rule, STOP, 1.0e-7);
        let times: Vec<f64> = run.seen.iter().map(|o| o.time).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]), "{rule:?}: observations are out of time order");
    }
}

#[test]
fn the_declared_rule_lands_on_every_clock_edge_and_the_observable_rule_on_the_input_changes() {
    // TP-06 AC 3: 1400 edges; the D2A's input changes at the first rising edge after 1, 2 and 3 us
    let declared = run(open_loop(), TimePointRule::Declared, STOP, 1.0e-7);
    let observable = run(open_loop(), TimePointRule::Observable, STOP, 1.0e-7);
    assert_eq!(declared.landings(), 1400, "{:?}", declared.statistics());
    assert_eq!(observable.landings(), 3, "{:?}", observable.statistics());
    assert!(observable.result.len() < declared.result.len() / 10, "the skipped landings are skipped steps");
}

#[test]
fn the_counter_counts_every_edge_in_both_rules() {
    // the instants nothing observes still run: the counter sees all 700 rising edges
    for rule in [TimePointRule::Observable, TimePointRule::Declared] {
        let mut builder = open_loop();
        builder.time_point_rule(rule);
        let mut circuit = builder.build().expect("build");
        circuit.transient(TransientAnalysisOptions::new(STOP, 1.0e-7), Context::default()).expect("tran").solve().expect("solve");
        let bound = circuit.bound().expect("bound");
        let n = count_of(bound.digital.read(bound.names.nets["net:n"]));
        assert_eq!(n, 700, "{rule:?}");
    }
}

/// A ramp of 1 kV per second across 1 kΩ, watched by a comparator against 2 mV: it crosses at 2 us.
fn crossing() -> CircuitBuilder {
    let mut b = CircuitBuilder::new("crossing");
    let gnd = b.ground();
    let ramp = b.node("ramp");
    let reference = b.node("ref");
    b.device(vramp(), "ramp", &[], &[Port::Node(ramp.clone()), Port::Node(gnd.clone())]);
    add_resistor(&mut b, "load", 1.0e3, &ramp, &gnd);
    add_vsrc(&mut b, "vr", 2.0e-3, &reference, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(ramp), Port::Node(reference), net("above")]);
    b
}

#[test]
fn a_cross_is_located_in_both_rules() {
    let times = |rule| {
        let run = run(crossing(), rule, 4.0e-6, 1.0e-6);
        run.seen.iter().filter(|o| o.kind == ObservationKind::Crossing && o.path == "cmp.up").map(|o| o.time).collect::<Vec<_>>()
    };
    let observable = times(TimePointRule::Observable);
    assert_eq!(observable.len(), 1);
    assert!((observable[0] - 2.0e-6).abs() < 1.0e-9, "{observable:?}");
    assert_eq!(observable, times(TimePointRule::Declared));
}

#[test]
fn a_bound_step_cap_applies_in_both_rules() {
    let cap = 1.0e-7;
    for rule in [TimePointRule::Observable, TimePointRule::Declared] {
        let mut b = CircuitBuilder::new("capped");
        let gnd = b.ground();
        let top = b.node("top");
        add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
        add_resistor(&mut b, "r1", 1.0e3, &top, &gnd);
        b.device(capped(), "cc", &[("cap", ParamValue::Real(cap))], &[Port::Node(top.clone())]);
        let run = run(b, rule, 1.0e-5, 1.0e-6);
        let times: Vec<f64> = run.result.iter().map(|step| step.time()).collect();
        let widest = times.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
        assert!(widest <= cap * (1.0 + 1.0e-9), "{rule:?}: a step of {widest:e} s beat the cap");
        assert!(run.statistics().ended(StepEnd::Cap) > 0, "{rule:?}: {:?}", run.statistics());
    }
}
