//! The protocol's sink (feature `solver-abi-core`, SAC-17; abi.md C5, A5):
//! observations of the discrete side reach the host's sink only after the
//! acceptance that makes them final, in instant order, stamped with their
//! path, analysis kind and time; an undone instant delivers nothing; a trace
//! record of a rejected attempt is delivered with its Newton iteration; traces
//! and observations reach the sink in time order; with no sink nothing is kept.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::protocol::{
    Advance, AnalysisKind, Attempt, Observation, ObservationKind, Protocol, ProtocolOptions, Sink,
};
use stub_sides::{Call, Log, ScriptedAnalog, ScriptedDiscrete, calls, new_log};

const SPT: f64 = 0.5;

/// Each observation with how many calls the log held when it arrived.
type Seen = Rc<RefCell<Vec<(usize, Observation)>>>;

/// A sink that keeps what it sees.
struct Collect {
    log: Log,
    seen: Seen,
}

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.seen.borrow_mut().push((self.log.borrow().len(), o.clone()));
    }
}

fn obs(path: &str, time: f64) -> Observation {
    Observation {
        path: path.into(),
        analysis: AnalysisKind::OperatingPoint,
        time,
        kind: ObservationKind::Commit { statement: 0, made: 0, due: 0, bytes: vec![1, 1] },
    }
}

fn collecting(log: &Log) -> (Box<dyn Sink>, Seen) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    (Box::new(Collect { log: Rc::clone(log), seen: Rc::clone(&seen) }), seen)
}

/// The index in the log of the `n`-th call for which `keep` holds.
fn position(log: &Log, n: usize, keep: impl Fn(&Call) -> bool) -> usize {
    log.borrow().iter().enumerate().filter(|(_, c)| keep(c)).nth(n).map(|(i, _)| i).expect("the call was made")
}

#[test]
fn observations_reach_the_sink_after_acceptance_in_instant_order() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    // The operating point's quiet and settle instants, then two transient landings.
    d.instant_observations.extend([vec![obs("top.op", 0.0)], vec![], vec![obs("top.b", 1.0)], vec![obs("top.c", 2.0)]]);
    // The first advance runs an instant at 0.5 s before the landing at 1 s.
    d.advance_observations.push_back(vec![obs("top.a", 0.5)]);
    a.proposals.extend([1.0, 2.0]);
    let (sink, seen) = collecting(&log);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(sink);
    p.operating_point().unwrap();
    p.transient(2.0).unwrap();

    let got: Vec<(String, AnalysisKind, f64)> =
        seen.borrow().iter().map(|(_, o)| (o.path.clone(), o.analysis, o.time)).collect();
    assert_eq!(
        got,
        [
            ("top.op".to_string(), AnalysisKind::OperatingPoint, 0.0),
            ("top.a".to_string(), AnalysisKind::Transient, 0.5),
            ("top.b".to_string(), AnalysisKind::Transient, 1.0),
            ("top.c".to_string(), AnalysisKind::Transient, 2.0),
        ]
    );
    // `top.a` and `top.b` arrive only after the first acceptance, `top.c` after the second.
    let first_accept = position(&log, 0, |c| matches!(c, Call::Accept));
    let second_accept = position(&log, 1, |c| matches!(c, Call::Accept));
    let at = |path: &str| seen.borrow().iter().find(|(_, o)| o.path == path).map(|(i, _)| *i).unwrap();
    assert!(at("top.a") > first_accept && at("top.b") > first_accept);
    assert!(at("top.c") > second_accept);
}

#[test]
fn an_undone_instant_delivers_nothing() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    // Each advance observes one instant; the first two are undone.
    d.advance_observations.extend([vec![obs("undone.rejected", 1.0)], vec![obs("undone.earlier", 1.5)], vec![obs("kept", 0.5)]]);
    d.advances.extend([Advance::Reached, Advance::Reached, Advance::Reached]);
    a.proposals.extend([2.0, 2.0, 2.0]);
    a.attempts.extend([Attempt::Rejected, Attempt::LandedEarlier(0.5)]);
    let (sink, seen) = collecting(&log);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(sink);
    p.transient(1.0).unwrap();
    let paths: Vec<String> = seen.borrow().iter().map(|(_, o)| o.path.clone()).collect();
    assert_eq!(paths, ["kept"], "only the instant of the accepted step is delivered");
    assert_eq!(p.buffered(), 0);
}

#[test]
fn a_trace_of_a_rejected_attempt_is_delivered_with_its_iteration() {
    let log = new_log();
    let d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    let trace = Observation {
        path: "top.d1.trace".into(),
        analysis: AnalysisKind::OperatingPoint,
        time: 1.0,
        kind: ObservationKind::Trace { iteration: 3, bytes: vec![0, 1] },
    };
    a.traces.push_back(vec![trace.clone()]);
    a.attempts.push_back(Attempt::Rejected);
    a.proposals.extend([1.0, 1.0]);
    let (sink, seen) = collecting(&log);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(sink);
    p.transient(1.0).unwrap();
    let got: Vec<Observation> = seen.borrow().iter().map(|(_, o)| o.clone()).collect();
    assert_eq!(got, [Observation { analysis: AnalysisKind::Transient, ..trace }]);
    // It arrived after the rejected attempt, at the acceptance that reached its
    // time (traces are delivered in time order, F13), though its step was undone.
    let index = seen.borrow()[0].0;
    assert!(index > position(&log, 0, |c| matches!(c, Call::DiscreteRestore(_))));
    assert!(index > position(&log, 0, |c| matches!(c, Call::Accept)));
}

#[test]
fn no_sink_means_observations_are_dropped_not_kept() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    d.advance_observations.extend((0..50).map(|i| vec![obs("x", i as f64)]));
    d.instant_observations.extend((0..50).map(|i| vec![obs("y", i as f64)]));
    a.traces.extend((0..50).map(|_| vec![obs("t", 0.0)]));
    a.proposals.extend((1..=50).map(|i| i as f64 * SPT));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.operating_point().unwrap();
    p.transient(25.0).unwrap();
    assert_eq!(p.buffered(), 0, "nothing is kept without a sink");
    // The sides are still drained, so their own buffers do not grow either.
    assert!(calls(&log, |c| matches!(c, Call::Drain)).len() >= 100);
    // One drain of traces after the equilibrium, one after each of the 50 attempts.
    assert_eq!(calls(&log, |c| matches!(c, Call::DrainTraces)).len(), 51);
}

fn trace(path: &str, time: f64, iteration: u32) -> Observation {
    Observation { path: path.into(), analysis: AnalysisKind::OperatingPoint, time, kind: ObservationKind::Trace { iteration, bytes: vec![] } }
}

#[test]
fn traces_reach_the_sink_in_time_order() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    // A step to 2 s is rejected (its trace stays, A5), a step to 1 s is accepted,
    // then a step to 2 s. Instants run before each landing and at it.
    d.advance_observations.extend([vec![obs("undone", 1.5)], vec![obs("before.1", 0.5)], vec![obs("before.2", 1.5)]]);
    d.instant_observations.extend([vec![obs("at.1", 1.0)], vec![obs("at.2", 2.0)]]);
    a.traces.extend([vec![trace("trace.rejected", 2.0, 1)], vec![trace("trace.1", 1.0, 2)], vec![trace("trace.2", 2.0, 3)]]);
    a.attempts.push_back(Attempt::Rejected);
    a.proposals.extend([2.0, 1.0, 2.0]);
    let (sink, seen) = collecting(&log);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(sink);
    p.transient(2.0).unwrap();
    let got: Vec<(String, f64)> = seen.borrow().iter().map(|(_, o)| (o.path.clone(), o.time)).collect();
    let want: Vec<(String, f64)> = [
        ("before.1", 0.5),
        ("trace.1", 1.0),
        ("at.1", 1.0),
        ("before.2", 1.5),
        ("trace.rejected", 2.0),
        ("trace.2", 2.0),
        ("at.2", 2.0),
    ]
    .iter()
    .map(|(p, t)| (p.to_string(), *t))
    .collect();
    assert_eq!(got, want, "one stream in time order; at one time the solve's traces precede its instant");
}
