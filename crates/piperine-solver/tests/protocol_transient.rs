//! The transient protocol (feature `solver-abi-core`, SAC-14, SAC-16 AC 9;
//! abi.md §5): on scripted stubs, a step with no landing is one attempt to the
//! proposed time; a landing is solved on the value before its instant (exempt),
//! then the instant runs and what it changed is delivered with a restart (the
//! user's order, superseding the boundary preview of AC 3); a rejected
//! or shortened attempt restores both sides and the sink mark and advances
//! again; acceptance fires the crossings, runs the instant with the samples
//! and asks the next acceptance for a restart (the small one after a host
//! param write); with no unknown no attempt is made; settle runs at time
//! points and the stop time only; a resume off the grid steps on from the
//! tick its discrete side ran at, and a stop inside that tick is an error.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::protocol::{
    Accepted, Advance, AnalysisKind, Attempt, Boundary, Ended, InstantMode, Observation, ObservationKind, Outcome,
    Protocol, ProtocolOptions, Restart, SetOutcome, Sink,
};
use stub_sides::{Call, horizon, Log, ScriptedAnalog, ScriptedDiscrete, calls, new_log};

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
fn a_proposal_a_few_ulps_short_of_a_tick_is_that_tick() {
    let (log, d, mut a) = sides();
    // 5e9 ticks, less two ulps: the rounding error of a sum of controller steps.
    let stop: f64 = 2.5e9;
    let short = f64::from_bits(stop.to_bits() - 2);
    a.proposals.push_back(short);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(stop).unwrap(), Ended::StopTime);
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. })),
        [Call::Advance(5_000_000_000), Call::Attempt { to: stop, exempt: false }],
        "one step to the tick, not a step a tick short and a sliver after it"
    );
}

#[test]
fn a_landing_is_solved_on_the_left_value_then_its_instant_runs() {
    let (log, mut d, mut a) = sides();
    // An R4 landing at 1 s whose instant changes a value the analog side reads.
    d.advances.push_back(Advance::Landing { tick: 2, time_point: true });
    d.instants.push_back(Outcome { boundary: vec![boundary()], ..Outcome::default() });
    a.accepts.push_back(Accepted { fired: vec![], samples: vec![0.5] });
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Deliver(..) | Call::Attempt { .. } | Call::Accept | Call::RunInstant { .. })
    });
    let mode = InstantMode { is_static: false, settle: true };
    assert_eq!(
        seq,
        [
            Call::Advance(6),
            // Solved at 1 s on the value before the instant: nothing delivered yet;
            // exempt, as a step that lands on a scheduled instant.
            Call::Attempt { to: 1.0, exempt: true },
            Call::Accept,
            // The instant runs after the solve; its reactions read the solution at 1 s.
            Call::RunInstant { at: 2, samples: vec![0.5], limit: horizon(6, 2), mode },
            // What it changed (microstep 0 included) reaches the analog side after
            // that solve, and the next step restarts from the jump.
            Call::Deliver(vec![boundary()], Restart::Integrator),
            Call::Advance(6),
            Call::Attempt { to: 3.0, exempt: false },
            Call::Accept,
            Call::RunInstant { at: 6, samples: vec![], limit: horizon(6, 6), mode },
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
    let seq = calls(&log, |c| matches!(c, Call::Accept | Call::Fire(..) | Call::RunInstant { .. } | Call::Deliver(..)));
    let mode = InstantMode { is_static: false, settle: true };
    assert_eq!(
        seq,
        [
            Call::Accept,
            Call::Fire(5, 2),
            Call::RunInstant { at: 2, samples: vec![0.25], limit: horizon(4, 2), mode },
            Call::Deliver(vec![boundary()], Restart::Integrator),
            Call::Accept,
            Call::RunInstant { at: 4, samples: vec![], limit: horizon(4, 4), mode },
        ]
    );
}

#[test]
fn a_host_param_write_asks_for_the_small_restart() {
    for (param, restart) in [(true, Restart::IntegratorSmall), (false, Restart::Integrator)] {
        let (log, mut d, mut a) = sides();
        // The operating point (two instants) and one step to 1 s, then the instant
        // a host `set` at 1 s runs: it changes a value the analog side reads.
        d.instants.extend([Outcome::default(), Outcome::default(), Outcome::default()]);
        d.instants.push_back(Outcome { boundary: vec![boundary()], host_param_write: param, ..Outcome::default() });
        a.accepts.push_back(Accepted { fired: vec![], samples: vec![0.25] });
        a.proposals.extend([1.0, 2.0]);
        let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
        p.operating_point().unwrap();
        p.transient(1.0).unwrap();
        let before = log.borrow().len();
        assert_eq!(p.set(5, 0, &[2], None).unwrap(), SetOutcome::Committed);
        p.transient(2.0).unwrap();
        let seq: Vec<Call> = log.borrow()[before..]
            .iter()
            .filter(|c| matches!(c, Call::Shadow(..) | Call::HostCommit { .. } | Call::RunInstant { .. } | Call::Deliver(..) | Call::Attempt { .. }))
            .cloned()
            .collect();
        let quiet = InstantMode { is_static: false, settle: false };
        let step = InstantMode { is_static: false, settle: true };
        assert_eq!(
            seq,
            [
                Call::Shadow(5, vec![2]),
                Call::HostCommit { net: 5, offset: 0, bytes: vec![2], at: 2 },
                // The write is applied at once at 1 s, reading the accepted solution there.
                Call::RunInstant { at: 2, samples: vec![0.25], limit: horizon(2, 2), mode: quiet },
                Call::Deliver(vec![boundary()], restart),
                // No zero-length step: the next attempt goes forward.
                Call::Attempt { to: 2.0, exempt: false },
                Call::RunInstant { at: 4, samples: vec![], limit: horizon(4, 4), mode: step },
            ],
            "param write: {param}"
        );
    }
}

#[test]
fn a_waiter_param_write_asks_for_the_small_restart() {
    let (log, mut d, mut a) = sides();
    a.proposals.extend([1.0, 2.0, 3.0]);
    // The instant at 1 s applied a host write to a param net (a waiter's write).
    d.instants.push_back(Outcome { boundary: vec![boundary()], host_param_write: true, ..Outcome::default() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    assert_eq!(calls(&log, |c| matches!(c, Call::Deliver(..))), [Call::Deliver(vec![boundary()], Restart::IntegratorSmall)]);
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
        matches!(c, Call::Attempt { .. } | Call::Propose(..) | Call::Accept | Call::Deliver(..))
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

/// The (attempted time, instant tick) pairs of a run, in order.
fn attempts_and_instants(log: &Log) -> Vec<Call> {
    calls(log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. } | Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, .. } => Call::RunInstant { at, samples: vec![], limit: horizon(0, 0), mode: InstantMode { is_static: false, settle: false } },
            other => other,
        })
        .collect()
}

#[test]
fn a_step_never_ends_a_tick_or_more_past_the_proposal_at_one_second() {
    // One femtosecond per tick (the default) at t ~ 1 s: the proposal lies 0.4
    // tick past a tick. A relative slack would round it up by ~100 ticks.
    let spt = 1.0e-15;
    for proposal in [1.0 + 1.0e-9 + 0.4e-15, 1.0 + 1.0e-9 + 0.9e-15, 0.999_999_999_999_9] {
        let (log, d, mut a) = sides();
        a.proposals.extend([proposal, 2.0]);
        let mut p = Protocol::new(d, a, ProtocolOptions::new(spt));
        p.transient(2.0).unwrap();
        let Call::Attempt { to, .. } = attempts(&log)[0].clone() else { unreachable!() };
        assert!(
            to < proposal + spt,
            "proposal {proposal:e}: the step ended {:e} s ({:.1} ticks) past it",
            to - proposal,
            (to - proposal) / spt
        );
    }
}

#[test]
fn an_off_grid_step_lands_on_the_tick_grid() {
    let run = |at: i64| Call::RunInstant { at, samples: vec![], limit: horizon(0, 0), mode: InstantMode { is_static: false, settle: false } };
    // One second per tick; the analog side proposes 1.6 s, then 2.4 s.
    let (log, d, mut a) = sides();
    a.proposals.extend([1.6, 2.4]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(1.0));
    p.transient(3.0).unwrap();
    assert_eq!(
        attempts_and_instants(&log),
        [
            Call::Advance(1),
            Call::Attempt { to: 1.0, exempt: false },
            run(1),
            Call::Advance(2),
            Call::Attempt { to: 2.0, exempt: false },
            run(2),
            Call::Advance(3),
            Call::Attempt { to: 3.0, exempt: false },
            run(3),
        ],
        "every step ends on a tick and the instant run is that tick's own"
    );
    assert_eq!(p.now(), 3.0);

    // A crossing located off the grid lands on its nearest tick inside the step.
    let (log, d, mut a) = sides();
    a.proposals.extend([2.6]);
    a.attempts.push_back(Attempt::LandedEarlier(1.4));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(1.0));
    p.transient(2.0).unwrap();
    let seq = attempts_and_instants(&log);
    assert_eq!(&seq[..5], [Call::Advance(2), Call::Attempt { to: 2.0, exempt: false }, Call::Advance(1), Call::Attempt { to: 1.0, exempt: false }, run(1)]);
}

#[test]
fn consecutive_rejections_beyond_the_cap_fail_loud() {
    for cap in [None, Some(5u32)] {
        let (log, d, mut a) = sides();
        a.attempts.extend((0..20_000).map(|_| Attempt::Rejected));
        let mut options = ProtocolOptions::new(SPT);
        if let Some(c) = cap {
            options.max_rejections = c;
        }
        let want = cap.unwrap_or(ProtocolOptions::MAX_REJECTIONS);
        let mut p = Protocol::new(d, a, options);
        let err = p.transient(1.0).expect_err("a step that never converges");
        let text = err.to_string();
        assert!(text.contains(&format!("{want} consecutive rejected attempts")), "{text}");
        assert!(text.contains("from t = 0e0 s"), "the error names the instant it could not leave: {text}");
        assert_eq!(attempts(&log).len(), want as usize);
    }
    // An acceptance resets the count.
    let (log, d, mut a) = sides();
    a.attempts.extend([Attempt::Rejected, Attempt::Rejected, Attempt::Converged, Attempt::Rejected, Attempt::Rejected]);
    a.proposals.extend([1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
    let mut options = ProtocolOptions::new(SPT);
    options.max_rejections = 3;
    let mut p = Protocol::new(d, a, options);
    assert_eq!(p.transient(2.0).unwrap(), Ended::StopTime);
    assert_eq!(attempts(&log).len(), 6);
}

#[test]
fn a_crossing_within_half_a_tick_of_the_target_does_not_loop() {
    // The analog side locates the same crossing at 0.9 s on every attempt; at
    // 0.5 s per tick it rounds to the target tick 2 (1.0 s).
    let (log, d, mut a) = sides();
    a.attempts.extend((0..5_000).map(|_| Attempt::LandedEarlier(0.9)));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(1.0).unwrap(), Ended::StopTime);
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Attempt { .. } | Call::Accept)),
        [
            Call::Attempt { to: 1.0, exempt: false },
            // Shrunk by one tick, never the identical step again.
            Call::Attempt { to: 0.5, exempt: false },
            // The crossing lies past this step's end: taken at the end.
            Call::Accept,
            // A one-tick step whose crossing rounds to its end: taken at the end.
            Call::Attempt { to: 1.0, exempt: false },
            Call::Accept,
        ]
    );
}

#[test]
fn a_crossing_at_or_after_the_target_is_taken_at_the_end() {
    let (log, d, mut a) = sides();
    a.attempts.extend((0..5_000).map(|_| Attempt::LandedEarlier(7.0)));
    a.proposals.push_back(1.0);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(2.0).unwrap(), Ended::StopTime);
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Attempt { .. } | Call::Accept)),
        [Call::Attempt { to: 1.0, exempt: false }, Call::Accept, Call::Attempt { to: 2.0, exempt: false }, Call::Accept]
    );
}

#[test]
fn crossing_retries_count_toward_the_rejection_cap() {
    // Each attempt to tick n locates a crossing 0.4 tick before it: one tick less each time.
    let (log, d, mut a) = sides();
    a.attempts.extend((0..20).map(|i| Attempt::LandedEarlier(99.6 - i as f64)));
    a.proposals.push_back(100.0);
    let mut options = ProtocolOptions::new(1.0);
    options.max_rejections = 5;
    let mut p = Protocol::new(d, a, options);
    let err = p.transient(100.0).expect_err("retries beyond the cap");
    assert!(err.to_string().contains("5 consecutive rejected attempts"), "{err}");
    assert_eq!(attempts(&log).len(), 5);
}

/// Re-entry (the PSS shooting seam): after `resume` the transient continues
/// from the resumed time on the sides as they were put back, with no
/// operating point; a host write between runs then runs its instant at once,
/// reading the samples given to `resume`.
#[test]
fn a_resumed_transient_continues_from_its_time_without_an_operating_point() {
    let (log, d, mut a) = sides();
    a.proposals.extend([3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume(2.0, vec![0.25]);
    assert_eq!(p.now(), 2.0);
    assert_eq!(p.set(7, 0, &[1], None).unwrap(), SetOutcome::Committed);
    let instants: Vec<(i64, Vec<f64>)> = calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, samples, .. } => (at, samples),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(instants, [(4, vec![0.25])], "the write's instant runs at the resumed time");
    assert_eq!(p.transient(4.0).unwrap(), Ended::StopTime);
    assert!(calls(&log, |c| matches!(c, Call::Start | Call::Equilibrium)).is_empty(), "no operating point");
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Propose(..) | Call::Advance(_) | Call::Attempt { .. })),
        [
            Call::Propose(2.0, 4.0),
            Call::Advance(6),
            Call::Attempt { to: 3.0, exempt: false },
            Call::Propose(3.0, 4.0),
            Call::Advance(8),
            Call::Attempt { to: 4.0, exempt: false },
        ]
    );
    assert_eq!(p.now(), 4.0);
}

/// A resume at a time off the grid steps on from the tick its discrete side
/// ran at last, the one `resume_at_tick` names: a stop inside the next tick is
/// reached by one step, and a stop after the time but inside that same tick
/// is an error, never a run that returns having taken no step.
#[test]
fn a_resume_off_the_grid_steps_on_from_its_tick() {
    // Resumed at 2.45 (tick 4.9), its instant at tick 4: a stop at 2.6 (tick 5.2) is one step to tick 5.
    let (log, d, mut a) = sides();
    a.proposals.extend([2.6]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume_at_tick(2.45, 4, vec![0.25]);
    assert_eq!(p.transient(2.6).unwrap(), Ended::StopTime);
    assert_eq!(
        calls(&log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. })),
        [Call::Advance(5), Call::Attempt { to: 2.5, exempt: false }]
    );

    // Resumed below its tick (an analog time 1.95, the instant at tick 4): a host write runs at tick 4.
    let (log, d, a) = sides();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume_at_tick(1.95, 4, vec![0.25]);
    assert_eq!(p.set(7, 0, &[1], None).unwrap(), SetOutcome::Committed);
    let at: Vec<i64> = calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, .. } => at,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(at, [4]);

    // A stop at 2.47 (tick 4.94) lies after 2.45 and inside tick 4: no step reaches it.
    let (log, d, a) = sides();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume_at_tick(2.45, 4, vec![0.25]);
    let err = p.transient(2.47).expect_err("a stop inside the resumed tick");
    assert!(err.to_string().contains("no step reaches it"), "{err}");
    assert!(attempts(&log).is_empty());
}
