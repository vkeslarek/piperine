//! The transient protocol (feature `solver-abi-core`, SAC-14, SAC-16 AC 9;
//! abi.md §5): on scripted stubs, a step with no landing is one attempt to the
//! proposed time; a landing's boundary preview is delivered before its
//! attempt, which is exempt only when the preview changed a value; a rejected
//! or shortened attempt restores both sides and the sink mark and advances
//! again; acceptance fires the crossings, runs the instant with the samples
//! and asks the next acceptance for a restart (the small one after a host
//! param write); with no unknown no attempt is made; settle runs at time
//! points and the stop time only.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::protocol::{
    Accepted, Advance, AnalysisKind, Attempt, Boundary, Ended, InstantMode, Observation, ObservationKind, Outcome,
    Protocol, ProtocolOptions, Restart, Sink,
};
use stub_sides::{Call, Log, ScriptedAnalog, ScriptedDiscrete, calls, new_log};

/// Half a second per tick: every time below is exact in binary.
const SPT: f64 = 0.5;

fn sides() -> (Log, ScriptedDiscrete, ScriptedAnalog) {
    let log = new_log();
    let d = ScriptedDiscrete::new(&log);
    let a = ScriptedAnalog::new(&log);
    (log, d, a)
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

fn attempts(log: &Log) -> Vec<Call> {
    calls(log, |c| matches!(c, Call::Attempt { .. }))
}

fn boundary() -> Boundary {
    Boundary { slot: 3, bytes: vec![1, 1] }
}

#[test]
fn no_landing_means_one_attempt_to_the_proposed_time() {
    let (log, d, mut a) = sides();
    a.proposals.extend([2.0, 5.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(5.0).unwrap(), Ended::StopTime);
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. })),
        [
            Call::Advance(4),
            Call::Attempt { to: 2.0, exempt: false },
            Call::Advance(10),
            Call::Attempt { to: 5.0, exempt: false },
        ]
    );
    assert_eq!(p.now(), 5.0);
}

#[test]
fn a_landing_delivers_its_preview_before_the_attempt() {
    let (log, mut d, mut a) = sides();
    d.advances.push_back(Advance::Landing { tick: 2, time_point: true });
    d.previews.borrow_mut().push_back(vec![boundary()]);
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Preview(_) | Call::Deliver(_) | Call::Attempt { .. })
    });
    assert_eq!(
        seq,
        [
            Call::Advance(6),
            Call::Preview(2),
            Call::Deliver(vec![boundary()]),
            Call::Attempt { to: 1.0, exempt: true },
            // The next step's preview changes nothing: no delivery, not exempt.
            Call::Advance(6),
            Call::Preview(6),
            Call::Attempt { to: 3.0, exempt: false },
        ]
    );
}

#[test]
fn a_rejected_attempt_restores_both_sides_and_replays() {
    let (log, mut d, mut a) = sides();
    let seen = Rc::new(RefCell::new(Vec::new()));
    // The first advance observes something; that instant is undone.
    d.advance_observations.push_back(vec![change("top.undone", 0.5)]);
    a.proposals.extend([2.0, 1.0]);
    a.attempts.push_back(Attempt::Rejected);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(Box::new(Collect(Rc::clone(&seen))));
    p.transient(2.0).unwrap();
    let mut seq = calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Attempt { .. } | Call::DiscreteRestore(_) | Call::AnalogRestore(_) | Call::DiscreteMark(_) | Call::AnalogMark(_))
    });
    seq.truncate(10);
    assert_eq!(
        seq,
        [
            Call::DiscreteMark(0),
            Call::AnalogMark(0),
            Call::Advance(4),
            Call::Attempt { to: 2.0, exempt: false },
            Call::DiscreteRestore(0),
            Call::AnalogRestore(0),
            Call::DiscreteMark(1),
            Call::AnalogMark(1),
            Call::Advance(2),
            Call::Attempt { to: 1.0, exempt: false },
        ]
    );
    assert!(seen.borrow().is_empty(), "the undone instant reached the sink: {:?}", seen.borrow());
    assert_eq!(p.buffered(), 0);
}

#[test]
fn an_earlier_landing_restores_and_replays() {
    let (log, d, mut a) = sides();
    a.proposals.extend([3.0, 3.0]);
    a.attempts.push_back(Attempt::LandedEarlier(1.5));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Attempt { .. } | Call::DiscreteRestore(_) | Call::AnalogRestore(_) | Call::Propose(..))
    });
    assert_eq!(
        seq,
        [
            Call::Propose(0.0, 3.0),
            Call::Advance(6),
            Call::Attempt { to: 3.0, exempt: false },
            Call::DiscreteRestore(0),
            Call::AnalogRestore(0),
            // No new proposal: the discrete side advances again to the landing.
            Call::Advance(3),
            Call::Attempt { to: 1.5, exempt: false },
            Call::Propose(1.5, 3.0),
            Call::Advance(6),
            Call::Attempt { to: 3.0, exempt: false },
        ]
    );
}

#[test]
fn acceptance_fires_crossings_runs_the_instant_and_asks_for_restart() {
    let (log, mut d, mut a) = sides();
    a.proposals.extend([1.0, 2.0]);
    a.accepts.push_back(Accepted { fired: vec![5], samples: vec![0.25] });
    d.instants.push_back(Outcome { boundary: vec![boundary()], ..Outcome::default() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(2.0).unwrap();
    let seq = calls(&log, |c| matches!(c, Call::Accept(_) | Call::Fire(..) | Call::RunInstant { .. } | Call::Deliver(_)));
    let mode = InstantMode { is_static: false, settle: true };
    assert_eq!(
        seq,
        [
            Call::Accept(Restart::None),
            Call::Fire(5, 2),
            Call::RunInstant { at: 2, samples: vec![0.25], limit: 4, mode },
            Call::Deliver(vec![boundary()]),
            Call::Accept(Restart::Integrator),
            Call::RunInstant { at: 4, samples: vec![], limit: 4, mode },
        ]
    );
}

#[test]
fn a_host_param_write_asks_for_the_small_restart() {
    let (log, mut d, mut a) = sides();
    a.proposals.extend([1.0, 2.0, 3.0]);
    d.instants.push_back(Outcome { boundary: vec![boundary()], host_param_write: true, ..Outcome::default() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Accept(_))),
        [Call::Accept(Restart::None), Call::Accept(Restart::IntegratorSmall), Call::Accept(Restart::None)]
    );
}

#[test]
fn no_unknowns_means_no_attempt() {
    let (log, mut d, mut a) = sides();
    a.unknowns = false;
    d.advances.extend([
        Advance::Landing { tick: 2, time_point: false },
        Advance::Landing { tick: 4, time_point: false },
        Advance::Landing { tick: 6, time_point: false },
    ]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(5.0).unwrap(), Ended::StopTime);
    let ats: Vec<i64> = calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, .. } => at,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(ats, [2, 4, 6, 10], "three instants, then the stop time");
    let analog = calls(&log, |c| {
        matches!(c, Call::Attempt { .. } | Call::Propose(..) | Call::Accept(_) | Call::Preview(_) | Call::Deliver(_))
    });
    assert!(analog.is_empty(), "no analog work without unknowns: {analog:?}");
    assert_eq!(attempts(&log).len(), 0);
}

#[test]
fn settle_runs_at_time_points_and_at_the_stop_time_only() {
    let (log, mut d, mut a) = sides();
    // An irreversible landing at 1 s (not a time point), an R4 landing at 2 s,
    // an ordinary accepted step at 3 s, the stop time at 4 s.
    d.advances.extend([Advance::Landing { tick: 2, time_point: false }, Advance::Landing { tick: 4, time_point: true }]);
    a.proposals.extend([3.0, 3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(4.0).unwrap();
    let instants: Vec<(i64, bool, bool)> = calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, mode, .. } => (at, mode.settle, mode.is_static),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(instants, [(2, false, false), (4, true, false), (6, true, false), (8, true, false)]);
}
