//! Irreversible instants, `FINISH`, host waiters and host writes in the
//! protocol (feature `solver-abi-core`, SAC-15, SAC-18; abi.md R6, R7, P5, P6,
//! §5 steps 2, 4, 5): an irreversible landing runs once after the acceptance
//! and settles only at a time point; `FINISH` ends the analysis after its
//! instant and is dropped when the step lands earlier; a host waiter runs once
//! at an accepted instant, never during `advance`, and its writes are due at
//! the next microstep; a host write is committed only after the shadow
//! accepts, a false `check` refuses it, a `RESHAPE` is refused inside a
//! transient and reported outside one, and a scheduled write is a host commit
//! at its tick.

use piperine_device_fixtures::scripted::Call;
use piperine_device_fixtures::scripted::Log;
use piperine_device_fixtures::scripted::ScriptedAnalog;
use piperine_device_fixtures::scripted::ScriptedDiscrete;
use piperine_device_fixtures::scripted::ScriptedLog;
use std::rc::Rc;

use piperine_solver::protocol::{
    Advance, LandingCause, Attempt, Ended, InstantMode, Outcome, Protocol, ProtocolOptions, SetOutcome, Shadow,
};

const SPT: f64 = 0.5;

const STEP: InstantMode = InstantMode { is_static: false, settle: true };
const FORCED: InstantMode = InstantMode { is_static: false, settle: false };

#[test]
fn an_irreversible_landing_runs_once_after_acceptance_and_does_not_settle() {
    let (log, mut d, mut a) = HostInstants::sides();
    // The instant at tick 2 fires an irreversible reaction: a landing that is no time point.
    d.advances.extend([Advance::Landing { tick: 2, cause: LandingCause::Irreversible }, Advance::Landing { tick: 2, cause: LandingCause::Irreversible }]);
    a.proposals.extend([2.0, 2.0, 2.0]);
    a.attempts.push_back(Attempt::Rejected);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(2.0).unwrap();
    assert_eq!(HostInstants::instants(&log), [(2, FORCED), (4, STEP)], "tick 2 runs once, with no settle");
    // It runs after the acceptance of the landing, not before the rejected attempt.
    let seq = ScriptedLog::calls(&log, |c| matches!(c, Call::Attempt { .. } | Call::Accept | Call::RunInstant { .. }));
    assert!(matches!(seq[0], Call::Attempt { to: 1.0, .. }));
    assert!(matches!(seq[1], Call::Attempt { to: 1.0, .. }));
    assert!(matches!(seq[2], Call::Accept));
    assert!(matches!(seq[3], Call::RunInstant { at: 2, .. }));
}

#[test]
fn finish_ends_after_its_instant_and_is_dropped_on_an_earlier_landing() {
    let (log, mut d, mut a) = HostInstants::sides();
    // A reaction at tick 6 (3 s) returns FINISH; the first step lands earlier at 1 s.
    d.advances.extend([Advance::Final(6), Advance::Reached, Advance::Final(6)]);
    a.proposals.extend([4.0, 4.0]);
    a.attempts.push_back(Attempt::LandedEarlier(1.0));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(10.0).unwrap(), Ended::Finish(3.0));
    assert_eq!(p.now(), 3.0);
    assert_eq!(HostInstants::instants(&log), [(2, STEP), (6, STEP)], "the analysis went on after 1 s and ended after 3 s");
    assert_eq!(ScriptedLog::calls(&log, |c| matches!(c, Call::Propose(..))).len(), 2, "nothing is proposed after the FINISH");

    // A FINISH returned by the landing instant itself ends the analysis too.
    let (log, mut d, mut a) = HostInstants::sides();
    d.instants.push_back(Outcome { finish: true, ..Outcome::default() });
    a.proposals.push_back(1.0);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(10.0).unwrap(), Ended::Finish(1.0));
    assert_eq!(HostInstants::instants(&log), [(2, STEP)]);
}

#[test]
fn a_host_waiter_runs_once_and_its_writes_are_due_next_microstep() {
    let (log, d, a) = HostInstants::sides();
    let (mut p, results) = HostInstants::waited(&log, d, a);
    p.transient(2.0).unwrap();
    let seq = ScriptedLog::calls(&log, |c| {
        matches!(c, Call::Wait(_) | Call::RunInstant { .. } | Call::Waiter(_) | Call::Shadow(..) | Call::HostCommit { .. })
    });
    assert_eq!(
        seq,
        [
            Call::Wait(4),
            Call::RunInstant { at: 2, samples: vec![], limit: ScriptedLog::horizon(4, 2), mode: FORCED },
            Call::Waiter(4),
            Call::Shadow(9, vec![1]),
            // Due at the instant's own tick: the next microstep.
            Call::HostCommit { net: 9, offset: 0, bytes: vec![1], at: 2 },
            // The instant continues.
            Call::RunInstant { at: 2, samples: vec![], limit: ScriptedLog::horizon(4, 2), mode: FORCED },
            Call::RunInstant { at: 4, samples: vec![], limit: ScriptedLog::horizon(4, 4), mode: STEP },
        ]
    );
    assert_eq!(*results.borrow(), [SetOutcome::Committed]);
}

#[test]
fn a_waiter_is_never_called_during_advance() {
    let (log, mut d, mut a) = HostInstants::sides();
    // The landing is rejected once and replayed.
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::Irreversible });
    a.attempts.push_back(Attempt::Rejected);
    a.proposals.push_back(2.0);
    let (mut p, _) = HostInstants::waited(&log, d, a);
    p.transient(2.0).unwrap();
    let log = log.borrow();
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Waiter(_))).count(), 1, "called once");
    let waiter = log.iter().position(|c| matches!(c, Call::Waiter(_))).unwrap();
    assert!(matches!(log[waiter - 1], Call::RunInstant { .. }), "a waiter follows a run_instant: {:?}", log[waiter - 1]);
    let last_accept_before = log[..waiter].iter().rposition(|c| matches!(c, Call::Accept));
    let last_advance_before = log[..waiter].iter().rposition(|c| matches!(c, Call::Advance(_)));
    assert!(last_accept_before > last_advance_before, "an acceptance separates the waiter from any advance");
}

#[test]
fn a_write_is_committed_only_after_the_shadow_accepts() {
    let (log, d, a) = HostInstants::sides();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.set(3, 8, &[0, 0, 0, 0, 0, 0, 0xF0, 0x3F], None).unwrap(), SetOutcome::Committed);
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Shadow(..) | Call::HostCommit { .. })),
        [
            Call::Shadow(3, vec![0, 0, 0, 0, 0, 0, 0xF0, 0x3F]),
            Call::HostCommit { net: 3, offset: 8, bytes: vec![0, 0, 0, 0, 0, 0, 0xF0, 0x3F], at: 0 },
        ]
    );
}

#[test]
fn a_false_check_refuses_and_commits_nothing() {
    let (log, d, a) = HostInstants::sides();
    d.shadows.borrow_mut().push_back(Shadow::Refused { check: "top.r1.r_ok".into() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    let outcome = p.set(3, 0, &[1], None).unwrap();
    let SetOutcome::Refused { reason } = outcome else { panic!("not refused: {outcome:?}") };
    assert!(reason.contains("top.r1.r_ok"), "{reason}");
    assert!(HostInstants::commits(&log).is_empty(), "nothing is committed");
    assert_eq!(ScriptedLog::calls(&log, |c| matches!(c, Call::Shadow(..))).len(), 1);
}

#[test]
fn reshape_is_refused_in_a_transient_and_reported_outside_one() {
    // Between analyses: reported, nothing committed.
    let (log, d, a) = HostInstants::sides();
    d.shadows.borrow_mut().push_back(Shadow::Reshape { instance: "top.m1".into() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.set(5, 0, &[2], None).unwrap(), SetOutcome::Reshape { instance: "top.m1".into() });
    assert!(HostInstants::commits(&log).is_empty());

    // Inside a transient (a waiter's write): refused.
    let (log, d, a) = HostInstants::sides();
    d.shadows.borrow_mut().push_back(Shadow::Reshape { instance: "top.m1".into() });
    let (mut p, results) = HostInstants::waited(&log, d, a);
    p.transient(2.0).unwrap();
    let got = results.borrow().clone();
    assert_eq!(got.len(), 1);
    let SetOutcome::Refused { reason } = &got[0] else { panic!("not refused: {got:?}") };
    assert!(reason.contains("top.m1"), "{reason}");
    assert!(HostInstants::commits(&log).is_empty(), "nothing is committed inside the transient");
    // After the transient the same write is reported again, not refused.
    p.discrete().shadows.borrow_mut().push_back(Shadow::Reshape { instance: "top.m1".into() });
    assert_eq!(p.set(5, 0, &[2], None).unwrap(), SetOutcome::Reshape { instance: "top.m1".into() });
}

#[test]
fn a_scheduled_write_is_a_host_commit_at_its_tick() {
    let (log, d, a) = HostInstants::sides();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.set(7, 0, &[1], Some(2.5)).unwrap(), SetOutcome::Committed);
    assert_eq!(HostInstants::commits(&log), [Call::HostCommit { net: 7, offset: 0, bytes: vec![1], at: 5 }]);
}

#[test]
fn irreversible_reactions_are_bounded_by_the_current_instant() {
    // With analog unknowns a crossing can fire right after the instant, so a
    // reaction a CROSS watch can reach may not run its clock past it (R5).
    let (log, mut d, mut a) = HostInstants::sides();
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::Irreversible });
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let bounds: Vec<(i64, piperine_solver::protocol::Horizon)> = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, limit, .. } => (at, limit),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(bounds, [(2, ScriptedLog::horizon(6, 2)), (6, ScriptedLog::horizon(6, 6))]);

    // With no unknown no crossing exists: the bound is the stop time.
    let (log, mut d, mut a) = HostInstants::sides();
    a.unknowns = false;
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::Irreversible });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let first = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { .. })).remove(0);
    assert!(matches!(first, Call::RunInstant { at: 2, limit, .. } if limit == ScriptedLog::horizon(6, 6)), "{first:?}");
}

/// The instants a host waits at, the sides it writes and the commits it sees.
struct HostInstants;

impl HostInstants {
    /// The sides helper `sides`.
    fn sides() -> (Log, ScriptedDiscrete, ScriptedAnalog) {
        let log = ScriptedLog::new();
        let d = ScriptedDiscrete::new(&log);
        let a = ScriptedAnalog::new(&log);
        (log, d, a)
    }

    /// The `(at, mode)` of every recorded `run_instant`.
    fn instants(log: &Log) -> Vec<(i64, InstantMode)> {
        ScriptedLog::calls(log, |c| matches!(c, Call::RunInstant { .. }))
            .into_iter()
            .map(|c| match c {
                Call::RunInstant { at, mode, .. } => (at, mode),
                _ => unreachable!(),
            })
            .collect()
    }

    /// A protocol whose instant at tick 2 fires host event 4, with a waiter that
    /// logs `Waiter(4)` and writes `[1]` to net 9.
    fn waited(log: &Log, mut d: ScriptedDiscrete, mut a: ScriptedAnalog) -> (Protocol<ScriptedDiscrete, ScriptedAnalog>, Rc<std::cell::RefCell<Vec<SetOutcome>>>) {
        d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::Irreversible });
        d.instants.push_back(Outcome { host_events: vec![4], ..Outcome::default() });
        a.proposals.extend([2.0, 2.0]);
        let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
        let results = Rc::new(std::cell::RefCell::new(Vec::new()));
        let (log, out) = (Rc::clone(log), Rc::clone(&results));
        p.on_event(4, Box::new(move |cx| {
            log.borrow_mut().push(Call::Waiter(cx.event()));
            assert_eq!(cx.time(), 1.0);
            out.borrow_mut().push(cx.set(9, 0, &[1]).unwrap());
        }));
        (p, results)
    }

    /// The commits helper `commits`.
    fn commits(log: &Log) -> Vec<Call> {
        ScriptedLog::calls(log, |c| matches!(c, Call::HostCommit { .. }))
    }
}
