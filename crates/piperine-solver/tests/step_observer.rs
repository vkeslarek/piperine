//! The step observer of the owner protocol (feature `long-run`, LR-01 and LR-02): the host sees
//! each accepted step once, in time order, after the sink has every observation up to it, and
//! never sees a rejected or retaken attempt. The first three tests run on the scripted sides of
//! the protocol tests; the rest run a transient of devices through `Solver`.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::protocol::{
    AnalysisKind, Attempt, Observation, ObservationKind, Protocol, ProtocolOptions, Sink, StepObserver, StepView,
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
