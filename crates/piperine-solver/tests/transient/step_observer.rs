//! The step observer of the owner protocol (feature `long-run`, LR-01 and LR-02): the host sees
//! each accepted step once, in time order, after the sink has every observation up to it, and
//! never sees a rejected or retaken attempt. The first three tests run on the scripted sides of
//! the protocol tests; the rest run a transient of devices through `Solver`.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::digital;
use piperine_device_fixtures::scripted::Call;
use piperine_device_fixtures::scripted::ScriptedAnalog;
use piperine_device_fixtures::scripted::ScriptedDiscrete;
use piperine_device_fixtures::scripted::ScriptedLog;
use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::protocol::{
    AnalysisKind, Attempt, Observation, ObservationKind, Protocol, ProtocolOptions, Sink, StepObserver, StepView,
};
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, Context, Keep, ProbeSelection, Solver, TransientAnalysisOptions, TransientAnalysisResult, TransientStep,
};

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

struct Rig {
    times: Rc<RefCell<Vec<f64>>>,
    sink_len_at_call: Rc<RefCell<Vec<usize>>>,
    sink: Rc<RefCell<Vec<Observation>>>,
}

#[test]
fn the_observer_is_called_once_per_accepted_step_in_time_order() {
    // LR-01 AC 1: three proposals, three accepted steps, three calls at 1, 2 and 3 s
    let log = ScriptedLog::new();
    let d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    a.proposals.extend([1.0, 2.0, 3.0]);
    let (rig, _sink, observer) = StepObserverCases::rig();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_step_observer(observer);
    p.transient(3.0).unwrap();
    assert_eq!(*rig.times.borrow(), [1.0, 2.0, 3.0]);
    assert_eq!(ScriptedLog::calls(&log, |c| matches!(c, Call::Accept)).len(), 3);
}

#[test]
fn the_sink_has_every_observation_up_to_the_step_before_the_observer() {
    // LR-01 AC 3: what each step's instant observed is in the sink when the observer is called
    let log = ScriptedLog::new();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    d.instant_observations.extend([vec![StepObserverCases::change("n.a", 1.0)], vec![StepObserverCases::change("n.b", 2.0), StepObserverCases::change("n.c", 2.0)], vec![]]);
    a.proposals.extend([1.0, 2.0, 3.0]);
    let (rig, sink, observer) = StepObserverCases::rig();
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
    let log = ScriptedLog::new();
    let d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    a.proposals.extend([2.0, 2.0, 2.0]);
    a.attempts.extend([Attempt::Rejected, Attempt::LandedEarlier(0.5)]);
    let (rig, _sink, observer) = StepObserverCases::rig();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_step_observer(observer);
    p.transient(1.0).unwrap();
    let attempts = ScriptedLog::calls(&log, |c| matches!(c, Call::Attempt { .. })).len();
    let accepts = ScriptedLog::calls(&log, |c| matches!(c, Call::Accept)).len();
    assert!(attempts > accepts, "the script made {attempts} attempts for {accepts} acceptances");
    assert_eq!(*rig.times.borrow(), [0.5, 1.0]);
}

// ---------------------------------------------------------------------------
// A transient of devices: a 100 us half-period clock drives a DAC into an RC and a counter of
// its rising edges (the mixed circuit of `transient_reentry.rs`).
// ---------------------------------------------------------------------------

const HALF: f64 = 1.0e-4;
const STOP: f64 = 1.0e-3;

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

#[test]
fn a_transient_observes_its_start_and_every_accepted_step() {
    // LR-01 AC 4: one call for the start and one per accepted step; the first is at the start time
    let (circuit, _c) = StepObserverCases::build();
    let probe = StepObserverCases::build().0.transient(StepObserverCases::options(), Context::default()).expect("tran").solve().expect("solve");
    let (result, seen) = StepObserverCases::run_observed(circuit, StepObserverCases::options(), None, StepObserverCases::net_count(&probe));
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
    let (circuit, c) = StepObserverCases::build();
    let probe = StepObserverCases::build().0.transient(StepObserverCases::options(), Context::default()).expect("tran").solve().expect("solve");
    let count = StepObserverCases::net_count(&probe);
    let (result, seen) = StepObserverCases::run_observed(circuit, StepObserverCases::options(), None, count);
    assert_eq!(seen.times.len(), result.len());
    let mut distinct_code = std::collections::HashSet::new();
    for (i, step) in result.iter().enumerate() {
        assert_eq!(seen.times[i], step.time(), "time of step {i}");
        let names = step.digital_nets().expect("nets").names().to_vec();
        for id in 0..count as usize {
            assert_eq!(seen.nets[i][id], step.digital(id).expect("net bytes"), "net `{}` at {:e} s", names[id], step.time());
        }
        distinct_code.insert(seen.nets[i][names.iter().position(|n| n == "code").unwrap()].clone());
        let v_c = step.get_node(&CircuitBuilding::node_id(&c)).expect("v(c)");
        assert_eq!(seen.solutions[i][c.idx().expect("an unknown")], v_c, "v(c) at {:e} s", step.time());
        // every reference with an index holds the kept step's value
        for reference in StepObserverCases::result_references(&step) {
            assert_eq!(seen.solutions[i][reference.0], reference.1, "unknown {} at {:e} s", reference.0, step.time());
        }
    }
    assert_eq!(distinct_code.len(), 2, "the clock toggles over the run");
}

#[test]
fn a_resumed_transient_observes_its_start() {
    // LR-01 AC 6: a run re-entered at a captured step observes that state first, then each step
    let (mut circuit, c) = StepObserverCases::build();
    let leg1 = circuit
        .transient(TransientAnalysisOptions::new(STOP / 2.0, 1.0e-5), Context::default())
        .expect("tran")
        .solve()
        .expect("solve");
    let count = StepObserverCases::net_count(&leg1);
    let captured = leg1.last().expect("steps").clone();
    let opts = StepObserverCases::options().with_start(captured.time());
    let (result, seen) = StepObserverCases::run_observed(circuit, opts, Some(&captured), count);
    assert_eq!(seen.times.len(), result.stats.steps_accepted as usize + 1);
    assert_eq!(seen.times[0], captured.time());
    assert_eq!(seen.solutions[0][c.idx().expect("an unknown")], captured.get_node(&CircuitBuilding::node_id(&c)).expect("v(c)"));
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
        let (circuit, _c) = StepObserverCases::build();
        let log = Log::default();
        let mut solver = Solver::new(circuit).with_tran_opts(StepObserverCases::options()).with_sink(Box::new(log.clone()));
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
        for (r, v) in StepObserverCases::result_references(a) {
            assert_eq!(Some(v), StepObserverCases::result_references(b).into_iter().find(|(i, _)| *i == r).map(|(_, v)| v));
        }
    }
}

#[test]
fn keep_last_holds_only_the_last_step_with_its_reentry_state() {
    // LR-02 AC 1: one step, the last one, the very step a `Keep::Every` run ends on; and it
    // carries the state a transient re-enters from (a run started from it goes on)
    let (mut circuit, c) = StepObserverCases::build();
    let full = StepObserverCases::run_kept(&mut circuit, STOP, Keep::Every, None);
    let (mut circuit, _) = StepObserverCases::build();
    let last = StepObserverCases::run_kept(&mut circuit, STOP, Keep::Last, None);
    assert_eq!(last.len(), 1, "one step kept");
    assert_eq!(last.stats.steps_accepted, full.stats.steps_accepted);
    let (kept, whole) = (last.last().unwrap(), full.last().unwrap());
    assert_eq!(kept.time(), STOP);
    assert_eq!(kept.time(), whole.time());
    assert_eq!(StepObserverCases::contents(kept), StepObserverCases::contents(whole));
    assert_eq!(kept.get_node(&CircuitBuilding::node_id(&c)), whole.get_node(&CircuitBuilding::node_id(&c)));
    let go_on = StepObserverCases::run_kept(&mut circuit, 2.0 * STOP, Keep::Every, Some(kept));
    assert_eq!(go_on.iter().next().unwrap().time(), STOP, "re-entered at the kept step");
    assert!(go_on.last().unwrap().time() > STOP);
}

#[test]
fn reentry_from_the_kept_step_equals_reentry_from_a_full_run() {
    // LR-02 AC 2: the half run captured from a `Keep::Every` result and from a `Keep::Last` one
    // continue to the same steps, bit for bit
    let half = STOP / 2.0;
    let (mut a, _) = StepObserverCases::build();
    let from_full = StepObserverCases::run_kept(&mut a, half, Keep::Every, None).last().unwrap().clone();
    let (mut b, _) = StepObserverCases::build();
    let from_last = StepObserverCases::run_kept(&mut b, half, Keep::Last, None).last().unwrap().clone();
    assert_eq!(StepObserverCases::contents(&from_full), StepObserverCases::contents(&from_last));
    let a2 = StepObserverCases::run_kept(&mut a, STOP, Keep::Every, Some(&from_full));
    let b2 = StepObserverCases::run_kept(&mut b, STOP, Keep::Every, Some(&from_last));
    assert!(a2.len() > 10, "{} steps", a2.len());
    assert_eq!(a2.len(), b2.len());
    for (x, y) in a2.iter().zip(b2.iter()) {
        assert_eq!(x.time(), y.time());
        assert_eq!(StepObserverCases::contents(x), StepObserverCases::contents(y), "at {:e} s", x.time());
    }
    assert_eq!(a2.stats.steps_accepted, b2.stats.steps_accepted);
    assert_eq!(a2.stats.steps_rejected, b2.stats.steps_rejected);
}

#[test]
fn the_default_keeps_every_accepted_step() {
    // LR-02 AC 3: with no `keep` set a run holds the start and every accepted step
    assert_eq!(TransientAnalysisOptions::new(STOP, 1.0e-5).keep, Keep::Every);
    let (mut circuit, _) = StepObserverCases::build();
    let mut tran = circuit.transient(TransientAnalysisOptions::new(STOP, 1.0e-5), Context::default()).expect("tran");
    let result = tran.solve().expect("solve");
    assert_eq!(result.len(), result.stats.steps_accepted as usize + 1);
}

#[test]
fn keep_last_refuses_device_state_recording() {
    // LR-02 AC 4: `Keep::Last` with `record_device_state`, or with a probe selection, fails at setup
    let (mut circuit, _) = StepObserverCases::build();
    let with_state = TransientAnalysisOptions { record_device_state: true, ..StepObserverCases::options().with_keep(Keep::Last) };
    let err = circuit.transient(with_state, Context::default()).err().expect("refused");
    assert!(err.to_string().contains("Keep::Last") && err.to_string().contains("record_device_state"), "{err}");
    let selection = ProbeSelection::new().request("clk", "out");
    let with_selection = StepObserverCases::options().with_keep(Keep::Last).with_probe_selection(selection);
    let err = circuit.transient(with_selection, Context::default()).err().expect("refused");
    assert!(err.to_string().contains("Keep::Last") && err.to_string().contains("probe_selection"), "{err}");
    // and the same options keeping every step are accepted
    assert!(circuit.transient(TransientAnalysisOptions { record_device_state: true, ..StepObserverCases::options() }, Context::default()).is_ok());
}

/// The helpers of the step_observer tests.
pub struct StepObserverCases;

impl StepObserverCases {
    /// The change helper `change`.
    fn change(path: &str, time: f64) -> Observation {
        Observation { path: path.into(), analysis: AnalysisKind::OperatingPoint, time, kind: ObservationKind::Change { bytes: vec![1] } }
    }

    /// The rig helper `rig`.
    fn rig() -> (Rig, Box<dyn Sink>, Box<dyn StepObserver>) {
        let times = Rc::new(RefCell::new(Vec::new()));
        let lens = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::new(RefCell::new(Vec::new()));
        let watch = Watch { times: Rc::clone(&times), sink_len_at_call: Rc::clone(&lens), sink: Rc::clone(&sink) };
        (Rig { times, sink_len_at_call: lens, sink: Rc::clone(&sink) }, Box::new(Collect(sink)), Box::new(watch))
    }

    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The build helper `build`.
    fn build() -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("clock -> dac -> rc");
        let gnd = b.ground();
        let o = b.node("o");
        let c = b.node("c");
        b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(HALF))], &[StepObserverCases::net("code")]);
        b.device(digital::Counter::module(), "cnt", &[], &[StepObserverCases::net("code"), StepObserverCases::net("n")]);
        b.device(mixed::BitDigitalToAnalog::module(), "dac", &[("vref", ParamValue::Real(1.0))], &[StepObserverCases::net("code"), Port::Node(o.clone())]);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &o, &c);
        CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-7, &c, &gnd);
        (b.build().expect("build"), c)
    }

    /// The options helper `options`.
    fn options() -> TransientAnalysisOptions {
        TransientAnalysisOptions::new(STOP, 1.0e-5)
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

    /// Every `(index, value)` of the unknowns a step holds, read through the netlist of a fresh build.
    fn result_references(step: &TransientStep) -> Vec<(usize, f64)> {
        let (circuit, _) = StepObserverCases::build();
        circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter_map(|r| Some((r.idx()?, step.get(r.variable().clone())?)))
            .collect()
    }

    // ---------------------------------------------------------------------------
    // `Keep::Last`: a run that holds the last step only (LR-02).
    // ---------------------------------------------------------------------------

    /// The clocked RC to `stop`, from `from` when given, keeping `keep`.
    fn run_kept(circuit: &mut CircuitInstance, stop: f64, keep: Keep, from: Option<&TransientStep>) -> TransientAnalysisResult {
        let mut opts = TransientAnalysisOptions::new(stop, 1.0e-5).with_keep(keep);
        if let Some(step) = from {
            opts = opts.with_start(step.time());
        }
        let mut tran = circuit.transient(opts, Context::default()).expect("tran");
        if let Some(step) = from {
            tran.with_initial_state(step);
        }
        tran.solve().expect("solve")
    }

    /// A step's unknowns by index and its nets, to compare two steps value for value.
    fn contents(step: &TransientStep) -> (Vec<(usize, f64)>, Option<piperine_solver::prelude::DigitalNets>) {
        let mut unknowns = StepObserverCases::result_references(step);
        unknowns.sort_by_key(|(i, _)| *i);
        (unknowns, step.digital_nets().cloned())
    }
}
