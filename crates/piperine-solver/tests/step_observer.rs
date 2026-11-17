//! The step observer of the owner protocol (feature `long-run`, LR-01 and LR-02): the host sees
//! each accepted step once, in time order, after the sink has every observation up to it, and
//! never sees a rejected or retaken attempt. The first three tests run on the scripted sides of
//! the protocol tests; the rest run a transient of devices through `Solver`.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::protocol::{
    AnalysisKind, Attempt, Observation, ObservationKind, Protocol, ProtocolOptions, Sink, StepObserver, StepView,
};
use abi_devices::{add_capacitor, add_resistor, node_id};
use abi_mixed::{counter, dac, timer};
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, Context, Solver, TransientAnalysisOptions, TransientAnalysisResult, TransientStep,
};
use stub_sides::{Call, ScriptedAnalog, ScriptedDiscrete, calls, new_log};

/// Half a second per tick: every time below is exact in binary.
const SPT: f64 = 0.5;

/// What the sink holds at each call of the observer.
struct Watch {
    times: Rc<RefCell<Vec<f64>>>,
    sink_len_at_call: Rc<RefCell<Vec<usize>>>,
    sink: Rc<RefCell<Vec<Observation>>>,
}

impl StepObserver for Watch {
    fn step(&mut self, step: &StepView<'_>) {
        self.times.borrow_mut().push(step.time());
        self.sink_len_at_call.borrow_mut().push(self.sink.borrow().len());
    }
}

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

fn change(path: &str, time: f64) -> Observation {
    Observation { path: path.into(), analysis: AnalysisKind::OperatingPoint, time, kind: ObservationKind::Change { bytes: vec![1] } }
}

struct Rig {
    times: Rc<RefCell<Vec<f64>>>,
    sink_len_at_call: Rc<RefCell<Vec<usize>>>,
    sink: Rc<RefCell<Vec<Observation>>>,
}

fn rig() -> (Rig, Box<dyn Sink>, Box<dyn StepObserver>) {
    let times = Rc::new(RefCell::new(Vec::new()));
    let lens = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::new(RefCell::new(Vec::new()));
    let watch = Watch { times: Rc::clone(&times), sink_len_at_call: Rc::clone(&lens), sink: Rc::clone(&sink) };
    (Rig { times, sink_len_at_call: lens, sink: Rc::clone(&sink) }, Box::new(Collect(sink)), Box::new(watch))
}

#[test]
fn the_observer_is_called_once_per_accepted_step_in_time_order() {
    // LR-01 AC 1: three proposals, three accepted steps, three calls at 1, 2 and 3 s
    let log = new_log();
    let d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    a.proposals.extend([1.0, 2.0, 3.0]);
    let (rig, _sink, observer) = rig();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_step_observer(observer);
    p.transient(3.0).unwrap();
    assert_eq!(*rig.times.borrow(), [1.0, 2.0, 3.0]);
    assert_eq!(calls(&log, |c| matches!(c, Call::Accept)).len(), 3);
}

#[test]
fn the_sink_has_every_observation_up_to_the_step_before_the_observer() {
    // LR-01 AC 3: what each step's instant observed is in the sink when the observer is called
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    d.instant_observations.extend([vec![change("n.a", 1.0)], vec![change("n.b", 2.0), change("n.c", 2.0)], vec![]]);
    a.proposals.extend([1.0, 2.0, 3.0]);
    let (rig, sink, observer) = rig();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(sink).with_step_observer(observer);
    p.transient(3.0).unwrap();
    assert_eq!(*rig.sink_len_at_call.borrow(), [1, 3, 3]);
    let paths: Vec<String> = rig.sink.borrow().iter().map(|o| o.path.clone()).collect();
    assert_eq!(paths, ["n.a", "n.b", "n.c"]);
}

#[test]
fn a_rejected_or_retaken_attempt_is_never_observed() {
    // LR-01 AC 2: a rejection and a landing earlier restore the sides; only the two accepted
    // steps (at 0.5 s and 1 s) are observed
    let log = new_log();
    let d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    a.proposals.extend([2.0, 2.0, 2.0]);
    a.attempts.extend([Attempt::Rejected, Attempt::LandedEarlier(0.5)]);
    let (rig, _sink, observer) = rig();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_step_observer(observer);
    p.transient(1.0).unwrap();
    let attempts = calls(&log, |c| matches!(c, Call::Attempt { .. })).len();
    let accepts = calls(&log, |c| matches!(c, Call::Accept)).len();
    assert!(attempts > accepts, "the script made {attempts} attempts for {accepts} acceptances");
    assert_eq!(*rig.times.borrow(), [0.5, 1.0]);
}

// ---------------------------------------------------------------------------
// A transient of devices: a 100 us half-period clock drives a DAC into an RC and a counter of
// its rising edges (the mixed circuit of `transient_reentry.rs`).
// ---------------------------------------------------------------------------

const HALF: f64 = 1.0e-4;
const STOP: f64 = 1.0e-3;

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

fn build() -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("clock -> dac -> rc");
    let gnd = b.ground();
    let o = b.node("o");
    let c = b.node("c");
    b.device(timer(), "clk", &[("half", ParamValue::Real(HALF))], &[net("code")]);
    b.device(counter(), "cnt", &[], &[net("code"), net("n")]);
    b.device(dac(), "dac", &[("vref", ParamValue::Real(1.0))], &[net("code"), Port::Node(o.clone())]);
    add_resistor(&mut b, "r1", 1.0e3, &o, &c);
    add_capacitor(&mut b, "c1", 1.0e-7, &c, &gnd);
    (b.build().expect("build"), c)
}

fn options() -> TransientAnalysisOptions {
    TransientAnalysisOptions::new(STOP, 1.0e-5)
}

/// Everything the observer saw: per call, the time, the whole solution and every net's bytes.
#[derive(Default)]
struct Seen {
    times: Vec<f64>,
    solutions: Vec<Vec<f64>>,
    nets: Vec<Vec<Vec<u8>>>,
}

struct Reader {
    seen: Rc<RefCell<Seen>>,
    net_count: u32,
}

impl StepObserver for Reader {
    fn step(&mut self, step: &StepView<'_>) {
        let mut seen = self.seen.borrow_mut();
        seen.times.push(step.time());
        seen.solutions.push((0..).map_while(|i| step.unknown(i)).collect());
        seen.nets.push((0..self.net_count).map(|id| step.net(id).expect("a net of the plan").to_vec()).collect());
    }
}

/// A full run of the clocked RC with the observer reading every net; the circuit comes back
/// with the result and what was seen.
fn run_observed(circuit: CircuitInstance, opts: TransientAnalysisOptions, from: Option<&TransientStep>, net_count: u32) -> (TransientAnalysisResult, Seen) {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let mut solver = Solver::new(circuit)
        .with_tran_opts(opts)
        .with_step_observer(Box::new(Reader { seen: Rc::clone(&seen), net_count }));
    let mut tran = solver.tran().expect("tran");
    if let Some(step) = from {
        tran.with_initial_state(step);
    }
    let result = tran.solve().expect("solve");
    drop(tran);
    drop(solver);
    (result, Rc::try_unwrap(seen).ok().expect("the solver is gone").into_inner())
}

/// The number of nets of the clocked RC's digital plan, read off a run's step.
fn net_count(result: &TransientAnalysisResult) -> u32 {
    result.last().and_then(|s| s.digital_nets()).expect("digital nets").names().len() as u32
}

#[test]
fn a_transient_observes_its_start_and_every_accepted_step() {
    // LR-01 AC 4: one call for the start and one per accepted step; the first is at the start time
    let (circuit, _c) = build();
    let probe = build().0.transient(options(), Context::default()).expect("tran").solve().expect("solve");
    let (result, seen) = run_observed(circuit, options(), None, net_count(&probe));
    assert!(result.stats.steps_accepted > 20, "{} steps", result.stats.steps_accepted);
    assert_eq!(seen.times.len(), result.stats.steps_accepted as usize + 1);
    assert_eq!(seen.times[0], 0.0);
    assert!(seen.times.windows(2).all(|w| w[0] < w[1]), "time order");
    assert_eq!(*seen.times.last().unwrap(), STOP);
}

#[test]
fn each_observed_step_holds_the_values_of_the_kept_step() {
    // LR-01 AC 5: every unknown and every net's bytes at each call equal the step of the
    // `Keep::Every` result at the same position
    let (circuit, c) = build();
    let probe = build().0.transient(options(), Context::default()).expect("tran").solve().expect("solve");
    let count = net_count(&probe);
    let (result, seen) = run_observed(circuit, options(), None, count);
    assert_eq!(seen.times.len(), result.len());
    let mut distinct_code = std::collections::HashSet::new();
    for (i, step) in result.iter().enumerate() {
        assert_eq!(seen.times[i], step.time(), "time of step {i}");
        let names = step.digital_nets().expect("nets").names().to_vec();
        for id in 0..count as usize {
            assert_eq!(seen.nets[i][id], step.digital(id).expect("net bytes"), "net `{}` at {:e} s", names[id], step.time());
        }
        distinct_code.insert(seen.nets[i][names.iter().position(|n| n == "code").unwrap()].clone());
        let v_c = step.get_node(&node_id(&c)).expect("v(c)");
        assert_eq!(seen.solutions[i][c.idx().expect("an unknown")], v_c, "v(c) at {:e} s", step.time());
        // every reference with an index holds the kept step's value
        for reference in result_references(&step) {
            assert_eq!(seen.solutions[i][reference.0], reference.1, "unknown {} at {:e} s", reference.0, step.time());
        }
    }
    assert_eq!(distinct_code.len(), 2, "the clock toggles over the run");
}

/// Every `(index, value)` of the unknowns a step holds, read through the netlist of a fresh build.
fn result_references(step: &TransientStep) -> Vec<(usize, f64)> {
    let (circuit, _) = build();
    circuit
        .netlist()
        .all_references()
        .into_iter()
        .filter_map(|r| Some((r.idx()?, step.get(r.variable().clone())?)))
        .collect()
}

#[test]
fn a_resumed_transient_observes_its_start() {
    // LR-01 AC 6: a run re-entered at a captured step observes that state first, then each step
    let (mut circuit, c) = build();
    let leg1 = circuit
        .transient(TransientAnalysisOptions::new(STOP / 2.0, 1.0e-5), Context::default())
        .expect("tran")
        .solve()
        .expect("solve");
    let count = net_count(&leg1);
    let captured = leg1.last().expect("steps").clone();
    let opts = options().with_start(captured.time());
    let (result, seen) = run_observed(circuit, opts, Some(&captured), count);
    assert_eq!(seen.times.len(), result.stats.steps_accepted as usize + 1);
    assert_eq!(seen.times[0], captured.time());
    assert_eq!(seen.solutions[0][c.idx().expect("an unknown")], captured.get_node(&node_id(&c)).expect("v(c)"));
    for id in 0..count as usize {
        assert_eq!(seen.nets[0][id], captured.digital(id).expect("net bytes"), "net {id} at the re-entry");
    }
    assert!(seen.times[1] > captured.time());
}

#[derive(Clone, Default)]
struct Log(Rc<RefCell<Vec<Observation>>>);

impl Sink for Log {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

struct Nothing;

impl StepObserver for Nothing {
    fn step(&mut self, _: &StepView<'_>) {}
}

#[test]
fn without_an_observer_the_result_and_the_sink_stream_are_unchanged() {
    // LR-01 AC 7: a run with no observer and a run with one return the same steps and deliver
    // the same observations to the sink, value for value
    let run = |observer: bool| {
        let (circuit, _c) = build();
        let log = Log::default();
        let mut solver = Solver::new(circuit).with_tran_opts(options()).with_sink(Box::new(log.clone()));
        if observer {
            solver = solver.with_step_observer(Box::new(Nothing));
        }
        let result = solver.tran().expect("tran").solve().expect("solve");
        let stream = log.0.borrow().clone();
        (result, stream)
    };
    let (plain, plain_stream) = run(false);
    let (observed, observed_stream) = run(true);
    assert!(!plain_stream.is_empty(), "the clock and the counter observe changes");
    assert_eq!(plain_stream, observed_stream);
    assert_eq!(plain.len(), observed.len());
    assert_eq!(plain.stats.steps_accepted, observed.stats.steps_accepted);
    assert_eq!(plain.stats.steps_rejected, observed.stats.steps_rejected);
    for (a, b) in plain.iter().zip(observed.iter()) {
        assert_eq!(a.time(), b.time());
        assert_eq!(a.digital_nets(), b.digital_nets());
        for (r, v) in result_references(a) {
            assert_eq!(Some(v), result_references(b).into_iter().find(|(i, _)| *i == r).map(|(_, v)| v));
        }
    }
}
