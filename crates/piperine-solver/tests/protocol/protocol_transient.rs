//! The transient protocol (feature `solver-abi-core`, SAC-14, SAC-16 AC 9;
//! abi.md §5): on scripted stubs, a step with no landing is one attempt to the
//! proposed time; a landing is solved on the value before its instant (a landing),
//! then the instant runs and what it changed is delivered with a restart (the
//! user's order, superseding the boundary preview of AC 3); a rejected
//! or shortened attempt restores both sides and the sink mark and advances
//! again; acceptance fires the crossings, runs the instant with the samples
//! and asks the next acceptance for a restart (the small one after a host
//! param write); with no unknown no attempt is made; settle runs at time
//! points and the stop time only; a resume off the grid steps on from the
//! tick its discrete side ran at, and a stop inside that tick is an error; on a real discrete
//! side, a loop that never settles stops the run with `E0511` (spec §8.2.3 step 4).

use piperine_device_fixtures::partials::PartialsCheck;
use piperine_device_fixtures::scripted::Call;
use piperine_device_fixtures::scripted::Log;
use piperine_device_fixtures::scripted::ScriptedAnalog;
use piperine_device_fixtures::scripted::ScriptedDiscrete;
use piperine_device_fixtures::scripted::ScriptedLog;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::BIT;
use piperine_abi::raw::{Cell, Module, Shape, cell, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::Port;
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};

use piperine_solver::protocol::{
    Accepted, Advance, LandingCause, AnalysisKind, Attempt, Boundary, Ended, InstantMode, Observation, ObservationKind, Outcome,
    Protocol, ProtocolOptions, Restart, SetOutcome, Sink, ZeroWidthCause,
};

/// Half a second per tick: every time below is exact in binary.
const SPT: f64 = 0.5;

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

#[test]
fn no_landing_means_one_attempt_to_the_proposed_time() {
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([2.0, 5.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(5.0).unwrap(), Ended::StopTime);
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. })),
        [
            Call::Advance(4),
            Call::Attempt { to: 2.0, landing: false },
            Call::Advance(10),
            Call::Attempt { to: 5.0, landing: false },
        ]
    );
    assert_eq!(p.now(), 5.0);
}

#[test]
fn a_proposal_a_few_ulps_short_of_a_tick_is_that_tick() {
    let (log, d, mut a) = ProtocolTransientCases::sides();
    // 5e9 ticks, less two ulps: the rounding error of a sum of controller steps.
    let stop: f64 = 2.5e9;
    let short = f64::from_bits(stop.to_bits() - 2);
    a.proposals.push_back(short);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(stop).unwrap(), Ended::StopTime);
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. })),
        [Call::Advance(5_000_000_000), Call::Attempt { to: stop, landing: false }],
        "one step to the tick, not a step a tick short and a sliver after it"
    );
}

#[test]
fn a_landing_is_solved_on_the_left_value_then_its_instant_runs() {
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    // An R4 landing at 1 s whose instant changes a value the analog side reads.
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::AnalogReadChanged });
    d.instants.push_back(Outcome { boundary: vec![ProtocolTransientCases::boundary()], ..Outcome::default() });
    a.accepts.push_back(Accepted { fired: vec![], samples: vec![0.5] });
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = ScriptedLog::calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Deliver(..) | Call::Attempt { .. } | Call::Accept | Call::RunInstant { .. })
    });
    let mode = InstantMode { is_static: false, settle: true };
    assert_eq!(
        seq,
        [
            Call::Advance(6),
            // Solved at 1 s on the value before the instant: nothing delivered yet;
            // a landing, as a step that lands on a scheduled instant.
            Call::Attempt { to: 1.0, landing: true },
            Call::Accept,
            // The instant runs after the solve; its reactions read the solution at 1 s.
            Call::RunInstant { at: 2, samples: vec![0.5], limit: ScriptedLog::horizon(6, 2), mode },
            // What it changed (microstep 0 included) reaches the analog side after
            // that solve, and the next step restarts from the jump.
            Call::Deliver(vec![ProtocolTransientCases::boundary()], Restart::Integrator),
            Call::Advance(6),
            Call::Attempt { to: 3.0, landing: false },
            Call::Accept,
            Call::RunInstant { at: 6, samples: vec![], limit: ScriptedLog::horizon(6, 6), mode },
        ]
    );
}

#[test]
fn a_rejected_attempt_restores_both_sides_and_replays() {
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    let seen = Rc::new(RefCell::new(Vec::new()));
    // The first advance observes something; that instant is undone.
    d.advance_observations.push_back(vec![ProtocolTransientCases::change("top.undone", 0.5)]);
    a.proposals.extend([2.0, 1.0]);
    a.attempts.push_back(Attempt::Rejected);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT)).with_sink(Box::new(Collect(Rc::clone(&seen))));
    p.transient(2.0).unwrap();
    let mut seq = ScriptedLog::calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Attempt { .. } | Call::DiscreteRestore(_) | Call::AnalogRestore(_) | Call::DiscreteMark(_) | Call::AnalogMark(_))
    });
    seq.truncate(10);
    assert_eq!(
        seq,
        [
            Call::DiscreteMark(0),
            Call::AnalogMark(0),
            Call::Advance(4),
            Call::Attempt { to: 2.0, landing: false },
            Call::DiscreteRestore(0),
            Call::AnalogRestore(0),
            Call::DiscreteMark(1),
            Call::AnalogMark(1),
            Call::Advance(2),
            Call::Attempt { to: 1.0, landing: false },
        ]
    );
    assert!(seen.borrow().is_empty(), "the undone instant reached the sink: {:?}", seen.borrow());
    assert_eq!(p.buffered(), 0);
}

#[test]
fn an_earlier_landing_restores_and_replays() {
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([3.0, 3.0]);
    a.attempts.push_back(Attempt::LandedEarlier(1.5));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = ScriptedLog::calls(&log, |c| {
        matches!(c, Call::Advance(_) | Call::Attempt { .. } | Call::DiscreteRestore(_) | Call::AnalogRestore(_) | Call::Propose(..))
    });
    assert_eq!(
        seq,
        [
            Call::Propose(0.0, 3.0),
            Call::Advance(6),
            Call::Attempt { to: 3.0, landing: false },
            Call::DiscreteRestore(0),
            Call::AnalogRestore(0),
            // No new proposal: the discrete side advances again to the landing.
            Call::Advance(3),
            Call::Attempt { to: 1.5, landing: false },
            Call::Propose(1.5, 3.0),
            Call::Advance(6),
            Call::Attempt { to: 3.0, landing: false },
        ]
    );
}

#[test]
fn acceptance_fires_crossings_runs_the_instant_and_asks_for_restart() {
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.0, 2.0]);
    a.accepts.push_back(Accepted { fired: vec![5], samples: vec![0.25] });
    d.instants.push_back(Outcome { boundary: vec![ProtocolTransientCases::boundary()], ..Outcome::default() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(2.0).unwrap();
    let seq = ScriptedLog::calls(&log, |c| matches!(c, Call::Accept | Call::Fire(..) | Call::RunInstant { .. } | Call::Deliver(..)));
    let mode = InstantMode { is_static: false, settle: true };
    assert_eq!(
        seq,
        [
            Call::Accept,
            Call::Fire(5, 2),
            Call::RunInstant { at: 2, samples: vec![0.25], limit: ScriptedLog::horizon(4, 2), mode },
            Call::Deliver(vec![ProtocolTransientCases::boundary()], Restart::Integrator),
            Call::Accept,
            Call::RunInstant { at: 4, samples: vec![], limit: ScriptedLog::horizon(4, 4), mode },
        ]
    );
}

#[test]
fn a_host_param_write_asks_for_the_small_restart() {
    for (param, restart) in [(true, Restart::IntegratorSmall), (false, Restart::Integrator)] {
        let (log, mut d, mut a) = ProtocolTransientCases::sides();
        // The operating point (two instants) and one step to 1 s, then the instant
        // a host `set` at 1 s runs: it changes a value the analog side reads.
        d.instants.extend([Outcome::default(), Outcome::default(), Outcome::default()]);
        d.instants.push_back(Outcome { boundary: vec![ProtocolTransientCases::boundary()], host_param_write: param, ..Outcome::default() });
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
                Call::RunInstant { at: 2, samples: vec![0.25], limit: ScriptedLog::horizon(2, 2), mode: quiet },
                Call::Deliver(vec![ProtocolTransientCases::boundary()], restart),
                // No zero-length step: the next attempt goes forward.
                Call::Attempt { to: 2.0, landing: false },
                Call::RunInstant { at: 4, samples: vec![], limit: ScriptedLog::horizon(4, 4), mode: step },
            ],
            "param write: {param}"
        );
    }
}

#[test]
fn a_waiter_param_write_asks_for_the_small_restart() {
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.0, 2.0, 3.0]);
    // The instant at 1 s applied a host write to a param net (a waiter's write).
    d.instants.push_back(Outcome { boundary: vec![ProtocolTransientCases::boundary()], host_param_write: true, ..Outcome::default() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    assert_eq!(ScriptedLog::calls(&log, |c| matches!(c, Call::Deliver(..))), [Call::Deliver(vec![ProtocolTransientCases::boundary()], Restart::IntegratorSmall)]);
}

#[test]
fn no_unknowns_means_no_attempt() {
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.unknowns = false;
    d.advances.extend([
        Advance::Landing { tick: 2, cause: LandingCause::Irreversible },
        Advance::Landing { tick: 4, cause: LandingCause::Irreversible },
        Advance::Landing { tick: 6, cause: LandingCause::Irreversible },
    ]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(5.0).unwrap(), Ended::StopTime);
    let ats: Vec<i64> = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, .. } => at,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(ats, [2, 4, 6, 10], "three instants, then the stop time");
    let analog = ScriptedLog::calls(&log, |c| {
        matches!(c, Call::Attempt { .. } | Call::Propose(..) | Call::Accept | Call::Deliver(..))
    });
    assert!(analog.is_empty(), "no analog work without unknowns: {analog:?}");
    assert_eq!(ProtocolTransientCases::attempts(&log).len(), 0);
}

#[test]
fn settle_runs_at_time_points_and_at_the_stop_time_only() {
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    // An irreversible landing at 1 s (not a time point), an R4 landing at 2 s,
    // an ordinary accepted step at 3 s, the stop time at 4 s.
    d.advances.extend([Advance::Landing { tick: 2, cause: LandingCause::Irreversible }, Advance::Landing { tick: 4, cause: LandingCause::AnalogReadChanged }]);
    a.proposals.extend([3.0, 3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(4.0).unwrap();
    let instants: Vec<(i64, bool, bool)> = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, mode, .. } => (at, mode.settle, mode.is_static),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(instants, [(2, false, false), (4, true, false), (6, true, false), (8, true, false)]);
}

#[test]
fn a_step_never_ends_a_tick_or_more_past_the_proposal_at_one_second() {
    // One femtosecond per tick (the default) at t ~ 1 s: the proposal lies 0.4
    // tick past a tick. A relative slack would round it up by ~100 ticks.
    let spt = 1.0e-15;
    for proposal in [1.0 + 1.0e-9 + 0.4e-15, 1.0 + 1.0e-9 + 0.9e-15, 0.999_999_999_999_9] {
        let (log, d, mut a) = ProtocolTransientCases::sides();
        a.proposals.extend([proposal, 2.0]);
        let mut p = Protocol::new(d, a, ProtocolOptions::new(spt));
        p.transient(2.0).unwrap();
        let Call::Attempt { to, .. } = ProtocolTransientCases::attempts(&log)[0].clone() else { unreachable!() };
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
    let run = |at: i64| Call::RunInstant { at, samples: vec![], limit: ScriptedLog::horizon(0, 0), mode: InstantMode { is_static: false, settle: false } };
    // One second per tick; the analog side proposes 1.6 s, then 2.4 s.
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.6, 2.4]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(1.0));
    p.transient(3.0).unwrap();
    assert_eq!(
        ProtocolTransientCases::attempts_and_instants(&log),
        [
            Call::Advance(1),
            Call::Attempt { to: 1.0, landing: false },
            run(1),
            Call::Advance(2),
            Call::Attempt { to: 2.0, landing: false },
            run(2),
            Call::Advance(3),
            Call::Attempt { to: 3.0, landing: false },
            run(3),
        ],
        "every step ends on a tick and the instant run is that tick's own"
    );
    assert_eq!(p.now(), 3.0);

    // A crossing located off the grid lands on its nearest tick inside the step.
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([2.6]);
    a.attempts.push_back(Attempt::LandedEarlier(1.4));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(1.0));
    p.transient(2.0).unwrap();
    let seq = ProtocolTransientCases::attempts_and_instants(&log);
    assert_eq!(&seq[..5], [Call::Advance(2), Call::Attempt { to: 2.0, landing: false }, Call::Advance(1), Call::Attempt { to: 1.0, landing: false }, run(1)]);
}

#[test]
fn consecutive_rejections_beyond_the_cap_fail_loud() {
    for cap in [None, Some(5u32)] {
        let (log, d, mut a) = ProtocolTransientCases::sides();
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
        assert_eq!(ProtocolTransientCases::attempts(&log).len(), want as usize);
    }
    // An acceptance resets the count.
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.attempts.extend([Attempt::Rejected, Attempt::Rejected, Attempt::Converged, Attempt::Rejected, Attempt::Rejected]);
    a.proposals.extend([1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
    let mut options = ProtocolOptions::new(SPT);
    options.max_rejections = 3;
    let mut p = Protocol::new(d, a, options);
    assert_eq!(p.transient(2.0).unwrap(), Ended::StopTime);
    assert_eq!(ProtocolTransientCases::attempts(&log).len(), 6);
}

#[test]
fn a_crossing_within_half_a_tick_of_the_target_does_not_loop() {
    // The analog side locates the same crossing at 0.9 s on every attempt; at
    // 0.5 s per tick it rounds to the target tick 2 (1.0 s).
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.attempts.extend((0..5_000).map(|_| Attempt::LandedEarlier(0.9)));
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(1.0).unwrap(), Ended::StopTime);
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Attempt { .. } | Call::Accept)),
        [
            Call::Attempt { to: 1.0, landing: false },
            // Shrunk by one tick, never the identical step again.
            Call::Attempt { to: 0.5, landing: false },
            // The crossing lies past this step's end: taken at the end.
            Call::Accept,
            // A one-tick step whose crossing rounds to its end: taken at the end.
            Call::Attempt { to: 1.0, landing: false },
            Call::Accept,
        ]
    );
}

#[test]
fn a_crossing_at_or_after_the_target_is_taken_at_the_end() {
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.attempts.extend((0..5_000).map(|_| Attempt::LandedEarlier(7.0)));
    a.proposals.push_back(1.0);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(2.0).unwrap(), Ended::StopTime);
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Attempt { .. } | Call::Accept)),
        [Call::Attempt { to: 1.0, landing: false }, Call::Accept, Call::Attempt { to: 2.0, landing: false }, Call::Accept]
    );
}

#[test]
fn crossing_retries_count_toward_the_rejection_cap() {
    // Each attempt to tick n locates a crossing 0.4 tick before it: one tick less each time.
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.attempts.extend((0..20).map(|i| Attempt::LandedEarlier(99.6 - i as f64)));
    a.proposals.push_back(100.0);
    let mut options = ProtocolOptions::new(1.0);
    options.max_rejections = 5;
    let mut p = Protocol::new(d, a, options);
    let err = p.transient(100.0).expect_err("retries beyond the cap");
    assert!(err.to_string().contains("5 consecutive rejected attempts"), "{err}");
    assert_eq!(ProtocolTransientCases::attempts(&log).len(), 5);
}

/// Re-entry (the PSS shooting seam): after `resume` the transient continues
/// from the resumed time on the sides as they were put back, with no
/// operating point; a host write between runs then runs its instant at once,
/// reading the samples given to `resume`.
#[test]
fn a_resumed_transient_continues_from_its_time_without_an_operating_point() {
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume(2.0, vec![0.25]);
    assert_eq!(p.now(), 2.0);
    assert_eq!(p.set(7, 0, &[1], None).unwrap(), SetOutcome::Committed);
    let instants: Vec<(i64, Vec<f64>)> = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, samples, .. } => (at, samples),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(instants, [(4, vec![0.25])], "the write's instant runs at the resumed time");
    assert_eq!(p.transient(4.0).unwrap(), Ended::StopTime);
    assert!(ScriptedLog::calls(&log, |c| matches!(c, Call::Start | Call::Equilibrium)).is_empty(), "no operating point");
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Propose(..) | Call::Advance(_) | Call::Attempt { .. })),
        [
            Call::Propose(2.0, 4.0),
            Call::Advance(6),
            Call::Attempt { to: 3.0, landing: false },
            Call::Propose(3.0, 4.0),
            Call::Advance(8),
            Call::Attempt { to: 4.0, landing: false },
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
    let (log, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([2.6]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume_at_tick(2.45, 4, vec![0.25]);
    assert_eq!(p.transient(2.6).unwrap(), Ended::StopTime);
    assert_eq!(
        ScriptedLog::calls(&log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. })),
        [Call::Advance(5), Call::Attempt { to: 2.5, landing: false }]
    );

    // Resumed below its tick (an analog time 1.95, the instant at tick 4): a host write runs at tick 4.
    let (log, d, a) = ProtocolTransientCases::sides();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume_at_tick(1.95, 4, vec![0.25]);
    assert_eq!(p.set(7, 0, &[1], None).unwrap(), SetOutcome::Committed);
    let at: Vec<i64> = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { .. }))
        .into_iter()
        .map(|c| match c {
            Call::RunInstant { at, .. } => at,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(at, [4]);

    // A stop at 2.47 (tick 4.94) lies after 2.45 and inside tick 4: no step reaches it.
    let (log, d, a) = ProtocolTransientCases::sides();
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.resume_at_tick(2.45, 4, vec![0.25]);
    let err = p.transient(2.47).expect_err("a stop inside the resumed tick");
    assert!(err.to_string().contains("no step reaches it"), "{err}");
    assert!(ProtocolTransientCases::attempts(&log).is_empty());
}

#[test]
fn the_proposal_sees_the_next_due_time() {
    // The discrete side's next queued commit is at tick 3, 1.5 s at half a second per tick: the
    // proposal is handed that time in seconds. With nothing queued it is handed none.
    let (_, mut d, mut a) = ProtocolTransientCases::sides();
    d.next = Some(3);
    a.proposals.extend([2.0, 5.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(5.0).unwrap(), Ended::StopTime);
    let (_, a) = p.into_sides();
    assert_eq!(a.next_dues, [Some(1.5), Some(1.5)]);

    let (_, d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([2.0, 5.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(5.0).unwrap();
    let (_, a) = p.into_sides();
    assert_eq!(a.next_dues, [None, None], "nothing queued: no next time point");
}

#[test]
fn a_settle_commit_lands_at_the_next_time_point() {
    // §15 S1: the instant at tick 2 makes a settle commit (it lands nothing: none was held); the next accepted time point, tick 4,
    // lands it before it runs; the first instant lands nothing it did not hold
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.0, 2.0]);
    d.instants.push_back(ProtocolTransientCases::settle_outcome());
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(2.0).unwrap(), Ended::StopTime);
    let at = |tick| Call::RunInstant { at: tick, samples: vec![], limit: ScriptedLog::horizon(4, tick), mode: InstantMode { is_static: false, settle: true } };
    assert_eq!(
        ProtocolTransientCases::settle_calls(&log),
        [
            Call::Attempt { to: 1.0, landing: false },
            Call::LandSettleCommits(2),
            at(2),
            Call::Attempt { to: 2.0, landing: true },
            Call::LandSettleCommits(4),
            at(4),
            Call::DropSettleCommits,
        ],
        "each time point lands what the one before held, before its instant runs"
    );
}

#[test]
fn the_step_after_a_settle_commit_is_a_landing_and_restarts() {
    // §15 S2, S3: the commit reaches the analog side with a restart; the step that arrives at the
    // next time point is a landing, and the one after it is not
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.0, 2.0, 3.0]);
    d.instants.push_back(ProtocolTransientCases::settle_outcome());
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    assert_eq!(
        ProtocolTransientCases::attempts(&log),
        [
            Call::Attempt { to: 1.0, landing: false },
            Call::Attempt { to: 2.0, landing: true },
            Call::Attempt { to: 3.0, landing: false },
        ]
    );
    assert_eq!(ScriptedLog::calls(&log, |c| matches!(c, Call::Deliver(..))), [Call::Deliver(vec![ProtocolTransientCases::boundary()], Restart::Integrator)]);
}

#[test]
fn an_event_time_before_the_next_time_point_reads_the_old_value() {
    // §15 S4: a forced landing that is not a time point (an `IRREVERSIBLE` reaction) lands nothing:
    // the commit stays held until the time point
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.0, 2.0]);
    d.instants.push_back(ProtocolTransientCases::settle_outcome());
    d.advances.extend([Advance::Reached, Advance::Landing { tick: 3, cause: LandingCause::Irreversible }]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(2.0).unwrap();
    let lands = ScriptedLog::calls(&log, |c| matches!(c, Call::LandSettleCommits(_)));
    // tick 2 (a time point: it lands what was held before it, nothing), tick 3 (not a time point) lands nothing, tick 4 lands
    assert_eq!(lands, [Call::LandSettleCommits(2), Call::LandSettleCommits(4)], "{:?}", ProtocolTransientCases::settle_calls(&log));
}

#[test]
fn a_rejected_step_moves_the_settle_commit() {
    // §15 S5: the step to the next time point is rejected: both sides restore, nothing lands, and
    // the retried step (still a landing) lands it at the time point it finally reaches
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.extend([1.0, 2.0, 2.0]);
    a.attempts.extend([Attempt::Converged, Attempt::Rejected, Attempt::Converged]);
    d.instants.push_back(ProtocolTransientCases::settle_outcome());
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(2.0).unwrap();
    assert_eq!(
        ProtocolTransientCases::attempts(&log),
        [
            Call::Attempt { to: 1.0, landing: false },
            Call::Attempt { to: 2.0, landing: true },
            Call::Attempt { to: 2.0, landing: true },
        ]
    );
    let order = ProtocolTransientCases::settle_calls(&log);
    let restore = order.iter().position(|c| matches!(c, Call::DiscreteRestore(_))).expect("the rejection restored");
    let land = order.iter().rposition(|c| matches!(c, Call::LandSettleCommits(4))).expect("landed at 4");
    assert!(restore < land, "nothing lands before the retried step is accepted: {order:?}");
    assert_eq!(order.iter().filter(|c| matches!(c, Call::LandSettleCommits(4))).count(), 1, "{order:?}");
}

#[test]
fn a_settle_commit_at_the_stop_time_is_dropped() {
    // §15 S6: the commit made at the stop time has no time point to land at: it is dropped
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    a.proposals.push_back(1.0);
    d.instants.push_back(ProtocolTransientCases::settle_outcome());
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    assert_eq!(p.transient(1.0).unwrap(), Ended::StopTime);
    let order = ProtocolTransientCases::settle_calls(&log);
    assert_eq!(order.last(), Some(&Call::DropSettleCommits), "{order:?}");
    assert_eq!(order.iter().filter(|c| matches!(c, Call::LandSettleCommits(_))).count(), 1, "only the stop's own landing: {order:?}");
}

/// A function that never settles on its own net: it writes the inverse of the level it reads.
/// Bound with `a` and `y` on one `RELAX` net it relaxes forever.
///
/// pins: a=0 (PORT BIT); y=1 (PORT BIT, RELAX).  fn 0 "flip": reads [a] writes [y]
struct Flipper;
static FLIPPER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLIPPER: OnceLock<Module> = OnceLock::new();

impl Flipper {
    /// The shape every `Flipper` shares.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &BIT) });
        s.function(FunctionSpec {
            name: "flip",
            origin: "protocol_transient.rs",
            reads: vec![Cell { kind: cell::PIN, index: a, part: part::VALUE }],
            writes: vec![Cell { kind: cell::PIN, index: y, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Flipper {
    fn prototype() -> &'static Shape<'static> {
        FLIPPER_SHAPE.get_or_init(Flipper::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Flipper, Arc::clone(FLIPPER_SHAPE.get_or_init(Flipper::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = cx.bytes(0)[0] == 0;
        cx.out_bytes(0).copy_from_slice(&[level as u8]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn an_unsettled_loop_is_e0511_naming_its_drives() {
    // abi.md §9 D1: a loop that still moves after the bounded passes stops the run with `E0511`,
    // naming the loop's drives; the bytes of a layout hold only values, so there is nothing to
    // zero
    let mut builder = CircuitBuilder::new("unsettled");
    builder.device(
        FLIPPER.get_or_init(|| module::<Flipper>("Flipper")),
        "flip",
        &[],
        &[Port::Net("x".to_string()), Port::Net("x".to_string())],
    );
    let mut solver = Solver::new(builder.build().expect("build"))
        .with_tran_opts(TransientAnalysisOptions::new(1.0e-7, 1.0e-8))
        .with_sink(Box::new(Collect(Rc::new(RefCell::new(Vec::new())))))
        .build();
    let error = solver.tran().expect("transient").solve().expect_err("a loop that does not settle fails the run");
    assert!(error.to_string().contains("E0511") && error.to_string().contains("did not settle; its drives: x"), "{error}");
    let (_, drives) = error.unsettled().expect("the fault carries the loop's drives as data");
    assert_eq!(drives, ["x"], "the drives are named without reading the message");
}

/// SAI-15: the target's own device passes the partials verifier (the guard of `abi_devices.rs`).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("Flipper", FLIPPER.get_or_init(|| module::<Flipper>("Flipper")));
}

#[test]
fn a_value_passed_within_an_instant_is_read_by_a_zero_width_solve_and_settle_follows() {
    // docs/spec §15.2 rule 4: the landing's instant passes slot 3 through a high it does not keep
    // (a pulse) and leaves slot 4 changed. The step to 1 s is not retaken: a zero-width solve at
    // 1 s reads those values (its solution replaces the first, and the settle phase reads it),
    // the settle phase runs once after it, and the first step after 1 s reads what the instant
    // left: slot 3 back low
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    let kept = Boundary { slot: 4, bytes: vec![2] };
    let high = Boundary { slot: 3, bytes: vec![1] };
    let low = Boundary { slot: 3, bytes: vec![0] };
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::AnalogReadChanged });
    d.instants.push_back(Outcome {
        boundary: vec![kept.clone(), low.clone()],
        passing: vec![high.clone(), kept.clone()],
        ..Outcome::default()
    });
    a.accepts.extend([Accepted { fired: vec![], samples: vec![0.5] }, Accepted { fired: vec![], samples: vec![0.25] }]);
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = ScriptedLog::calls(&log, |c| {
        matches!(
            c,
            Call::Deliver(..) | Call::Attempt { .. } | Call::Accept | Call::ZeroWidth | Call::RunInstant { .. } | Call::AnalogRestore(_)
        )
    });
    let settling = InstantMode { is_static: false, settle: true };
    assert_eq!(
        seq,
        [
            Call::Attempt { to: 1.0, landing: true },
            Call::Accept,
            Call::RunInstant { at: 2, samples: vec![0.5], limit: ScriptedLog::horizon(6, 2), mode: settling },
            // the zero-width solve at 1 s, on the values passed through: nothing is restored
            Call::Deliver(vec![high, kept.clone()], Restart::Integrator),
            Call::ZeroWidth,
            // the settle phase follows it and reads its solution
            Call::RunInstant { at: 2, samples: vec![0.25], limit: ScriptedLog::horizon(6, 2), mode: settling },
            // what the instant left behind reaches the next step, with the restart of a jump
            Call::Deliver(vec![kept, low], Restart::Integrator),
            Call::Attempt { to: 3.0, landing: false },
            Call::Accept,
            Call::RunInstant { at: 6, samples: vec![], limit: ScriptedLog::horizon(6, 6), mode: settling },
        ]
    );
    assert_eq!(p.statistics().zero_width_for(ZeroWidthCause::PassedValue), 1, "counted for its cause");
}

#[test]
fn a_crossing_the_zero_width_solve_makes_is_read_by_another() {
    // docs/spec §15.2 rule 4: the zero-width solve at 1 s makes crossing 7; it fires at the next
    // microstep and its reaction changes slot 5 once. That change is read at 1 s by a second
    // zero-width solve (never by a retake of the step, which would undo the first), and the
    // first step after 1 s reads every change of the instant
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    let kept = Boundary { slot: 4, bytes: vec![2] };
    let high = Boundary { slot: 3, bytes: vec![1] };
    let low = Boundary { slot: 3, bytes: vec![0] };
    let made = Boundary { slot: 5, bytes: vec![1] };
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::AnalogReadChanged });
    d.instants.push_back(Outcome {
        boundary: vec![kept.clone(), low.clone()],
        passing: vec![high.clone(), kept.clone()],
        ..Outcome::default()
    });
    d.instants.push_back(Outcome { boundary: vec![made.clone()], ..Outcome::default() });
    a.accepts.extend([
        Accepted { fired: vec![], samples: vec![0.5] },
        Accepted { fired: vec![7], samples: vec![0.25] },
        Accepted { fired: vec![7], samples: vec![0.125] },
    ]);
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.transient(3.0).unwrap();
    let seq = ScriptedLog::calls(&log, |c| {
        matches!(c, Call::Deliver(..) | Call::ZeroWidth | Call::Fire(..) | Call::RunInstant { .. } | Call::AnalogRestore(_))
    });
    let settling = InstantMode { is_static: false, settle: true };
    let continued = InstantMode { is_static: false, settle: false };
    assert_eq!(
        seq[..8],
        [
            Call::RunInstant { at: 2, samples: vec![0.5], limit: ScriptedLog::horizon(6, 2), mode: settling },
            Call::Deliver(vec![high, kept.clone()], Restart::Integrator),
            Call::ZeroWidth,
            // the crossing it made fires at the next microstep, and the instant goes on
            Call::Fire(7, 2),
            Call::RunInstant { at: 2, samples: vec![0.25], limit: ScriptedLog::horizon(6, 2), mode: continued },
            // the change of its reaction, read by a second zero-width solve
            Call::Deliver(vec![made.clone()], Restart::Integrator),
            Call::ZeroWidth,
            Call::RunInstant { at: 2, samples: vec![0.125], limit: ScriptedLog::horizon(6, 2), mode: settling },
        ]
    );
    assert_eq!(seq[8], Call::Deliver(vec![kept, low, made], Restart::Integrator), "every change of the instant");
    assert_eq!(
        (p.statistics().zero_width_for(ZeroWidthCause::PassedValue), p.statistics().zero_width_for(ZeroWidthCause::CrossingChange)),
        (1, 1)
    );
}

#[test]
fn a_pulse_across_a_host_suspension_is_read_once_with_one_settle_phase() {
    // docs/spec §15.2 rule 4: a host waiter suspends the landing's instant after its first part;
    // each part reports what the whole instant passed through so far, so the owner reads the last
    // part's record (not the parts joined), and the settle phase runs once, after the reading
    let (log, mut d, mut a) = ProtocolTransientCases::sides();
    let kept = Boundary { slot: 4, bytes: vec![2] };
    let high = Boundary { slot: 3, bytes: vec![1] };
    let low = Boundary { slot: 3, bytes: vec![0] };
    d.advances.push_back(Advance::Landing { tick: 2, cause: LandingCause::AnalogReadChanged });
    d.instants.push_back(Outcome { host_events: vec![5], passing: vec![high.clone()], ..Outcome::default() });
    d.instants.push_back(Outcome {
        boundary: vec![kept.clone(), low.clone()],
        passing: vec![high.clone(), kept.clone()],
        ..Outcome::default()
    });
    a.accepts.extend([Accepted { fired: vec![], samples: vec![0.5] }, Accepted { fired: vec![], samples: vec![0.25] }]);
    a.proposals.extend([3.0, 3.0]);
    let mut p = Protocol::new(d, a, ProtocolOptions::new(SPT));
    p.on_event(5, Box::new(|_| {}));
    p.transient(3.0).unwrap();
    let delivered = ScriptedLog::calls(&log, |c| matches!(c, Call::Deliver(..)));
    assert_eq!(
        delivered,
        [Call::Deliver(vec![high, kept.clone()], Restart::Integrator), Call::Deliver(vec![kept, low], Restart::Integrator)],
        "the whole instant's record is read once, then what the instant left"
    );
    let settling = InstantMode { is_static: false, settle: true };
    let settles = ScriptedLog::calls(&log, |c| matches!(c, Call::RunInstant { at: 2, mode, .. } if *mode == settling));
    assert_eq!(settles.len(), 3, "the two parts, run with the settle phase the side leaves to the owner, and the phase itself once");
}

/// The helpers of the protocol_transient tests.
pub struct ProtocolTransientCases;

impl ProtocolTransientCases {
    /// The sides helper `sides`.
    fn sides() -> (Log, ScriptedDiscrete, ScriptedAnalog) {
        let log = ScriptedLog::new();
        let d = ScriptedDiscrete::new(&log);
        let a = ScriptedAnalog::new(&log);
        (log, d, a)
    }

    /// The change helper `change`.
    fn change(path: &str, time: f64) -> Observation {
        Observation { path: path.into(), analysis: AnalysisKind::OperatingPoint, time, kind: ObservationKind::Change { bytes: vec![1] } }
    }

    /// The attempts helper `attempts`.
    fn attempts(log: &Log) -> Vec<Call> {
        ScriptedLog::calls(log, |c| matches!(c, Call::Attempt { .. }))
    }

    /// The boundary helper `boundary`.
    fn boundary() -> Boundary {
        Boundary { slot: 3, bytes: vec![1] }
    }

    /// The (attempted time, instant tick) pairs of a run, in order.
    fn attempts_and_instants(log: &Log) -> Vec<Call> {
        ScriptedLog::calls(log, |c| matches!(c, Call::Advance(_) | Call::Attempt { .. } | Call::RunInstant { .. }))
            .into_iter()
            .map(|c| match c {
                Call::RunInstant { at, .. } => Call::RunInstant { at, samples: vec![], limit: ScriptedLog::horizon(0, 0), mode: InstantMode { is_static: false, settle: false } },
                other => other,
            })
            .collect()
    }

    /// The settle boundary of the commits an instant held: slot 3 will read `[1]` (§15 S2).
    fn settle_outcome() -> Outcome {
        Outcome { settle_boundary: vec![ProtocolTransientCases::boundary()], ..Outcome::default() }
    }

    /// The calls of the settle commit protocol and of the instants, in order.
    fn settle_calls(log: &Log) -> Vec<Call> {
        ScriptedLog::calls(log, |c| {
            matches!(c, Call::LandSettleCommits(_) | Call::DropSettleCommits | Call::RunInstant { .. } | Call::Attempt { .. } | Call::DiscreteRestore(_))
        })
    }
}
