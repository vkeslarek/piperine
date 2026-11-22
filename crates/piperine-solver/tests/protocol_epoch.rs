//! The epoch of the transient protocol (feature `transient-performance`, T25): past 2^52 ticks an
//! `f64` count of seconds cannot tell neighbouring ticks apart, so the owner counts the analog
//! side's times from an epoch it moves to every accepted tick, on scripted sides: every time the
//! analog side is given is a small, exact number of seconds since the epoch, a step of one tick is
//! one tick long, and a run below 2^52 ticks never moves its epoch.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use piperine_solver::protocol::{Attempt, Ended, Origin, Protocol, ProtocolOptions};
use stub_sides::{Call, ScriptedAnalog, ScriptedDiscrete, calls, new_log};

/// One second per tick: a time and its tick are the same number.
const SPT: f64 = 1.0;

/// The first tick an `f64` second cannot tell from its neighbour.
const BEYOND: i64 = 1 << 52;

/// A protocol resumed at tick `start` over scripted sides, and the log they write.
fn resumed_at(start: i64, analog: impl FnOnce(&mut ScriptedAnalog)) -> (Protocol<ScriptedDiscrete, ScriptedAnalog>, stub_sides::Log) {
    let log = new_log();
    let mut scripted = ScriptedAnalog::new(&log);
    analog(&mut scripted);
    let mut protocol = Protocol::new(ScriptedDiscrete::new(&log), scripted, ProtocolOptions::new(SPT));
    protocol.resume(start as f64, Vec::new());
    (protocol, log)
}

/// The `Rebase` calls of a log.
fn rebases(log: &stub_sides::Log) -> Vec<Origin> {
    calls(log, |call| matches!(call, Call::Rebase(_))).into_iter().map(|call| match call {
        Call::Rebase(origin) => origin,
        other => unreachable!("{other:?}"),
    }).collect()
}

#[test]
fn past_2_52_ticks_the_analog_side_is_told_small_times_since_the_epoch() {
    let start = BEYOND + 7;
    let (mut protocol, log) = resumed_at(start, |analog| analog.proposals.extend([10.0, 10.0, 10.0]));

    let ended = protocol.transient((start + 1000) as f64).expect("the run");

    assert_eq!(ended, Ended::StopTime);
    let proposed = calls(&log, |call| matches!(call, Call::Propose(..)));
    assert_eq!(proposed[0], Call::Propose(0.0, 1000.0), "the stop is 1000 ticks after the accepted tick");
    assert_eq!(proposed[1], Call::Propose(0.0, 990.0), "after a step of 10 ticks the origin moved there");
    let attempts = calls(&log, |call| matches!(call, Call::Attempt { .. }));
    assert_eq!(attempts[..3], vec![Call::Attempt { to: 10.0, exempt: false }; 3], "every step is 10 ticks from its own origin");
    let moves = rebases(&log);
    // the resume floors the start to a tick, which at 2^52 can round one tick up
    assert!((moves[0].moved - start as f64).abs() <= 1.0, "the first move brings the origin from tick 0 to the start: {}", moves[0].moved);
    assert_eq!(moves[1..4].iter().map(|origin| origin.moved).collect::<Vec<_>>(), [10.0; 3]);
    assert_eq!(moves[1].stop, 990.0);
}

#[test]
fn a_step_of_one_tick_is_one_tick_long_past_2_52_ticks() {
    let (mut protocol, log) = resumed_at(BEYOND + 7, |analog| {
        analog.proposals.push_back(10.0);
        analog.attempts.push_back(Attempt::LandedEarlier(1.0));
    });

    protocol.transient((BEYOND + 20) as f64).expect("the run");

    let attempts = calls(&log, |call| matches!(call, Call::Attempt { .. }));
    assert_eq!(attempts[0], Call::Attempt { to: 10.0, exempt: false });
    assert_eq!(attempts[1], Call::Attempt { to: 1.0, exempt: false }, "the retake lands one tick after the accepted tick");
}

#[test]
fn a_run_under_2_52_ticks_never_moves_its_epoch() {
    let (mut protocol, log) = resumed_at(BEYOND - 2000, |analog| analog.proposals.extend([10.0; 5]));

    protocol.transient((BEYOND - 1000) as f64).expect("the run");

    assert!(rebases(&log).is_empty(), "no move below 2^52 ticks: {:?}", rebases(&log));
    let proposed = calls(&log, |call| matches!(call, Call::Propose(..)));
    assert_eq!(proposed[0], Call::Propose((BEYOND - 2000) as f64, (BEYOND - 1000) as f64), "absolute times, as before");
}
