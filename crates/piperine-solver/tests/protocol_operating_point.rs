//! The operating point of the protocol (feature `solver-abi-core`, SAC-16
//! AC 10, 11; abi.md §10): on scripted stubs it starts the discrete side,
//! solves the equilibrium, runs the instant, solves again while the instant
//! changes a value the analog side reads, then settles once; rounds beyond the
//! bound fail with `E0506` naming the nets; with no unknown only the instant runs.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use piperine_solver::protocol::{Accepted, Boundary, Ended, InstantMode, Outcome, Protocol, ProtocolOptions, Restart};
use stub_sides::{Call, horizon, Log, ScriptedAnalog, ScriptedDiscrete, calls, new_log};

const QUIET: InstantMode = InstantMode { is_static: true, settle: false };
const SETTLE: InstantMode = InstantMode { is_static: true, settle: true };

fn changing() -> Outcome {
    Outcome { boundary: vec![Boundary { slot: 7, bytes: vec![1] }], ..Outcome::default() }
}

fn sequence(log: &Log) -> Vec<Call> {
    calls(log, |c| {
        matches!(c, Call::Start | Call::Equilibrium | Call::RunInstant { .. } | Call::Deliver(..) | Call::Fire(..))
    })
}

#[test]
fn the_operating_point_follows_the_call_sequence() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    d.instants.push_back(changing());
    a.equilibria.push_back(Accepted { fired: vec![], samples: vec![0.5] });
    a.equilibria.push_back(Accepted { fired: vec![9], samples: vec![0.75] });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(1e-12));
    p.operating_point().unwrap();
    assert_eq!(
        sequence(&log),
        [
            Call::Start,
            Call::Equilibrium,
            Call::RunInstant { at: 0, samples: vec![0.5], limit: horizon(0, 0), mode: QUIET },
            Call::Deliver(vec![Boundary { slot: 7, bytes: vec![1] }], Restart::None),
            Call::Equilibrium,
            // A crossing between rounds is due at the next microstep.
            Call::Fire(9, 0),
            Call::RunInstant { at: 0, samples: vec![0.75], limit: horizon(0, 0), mode: QUIET },
            Call::RunInstant { at: 0, samples: vec![0.75], limit: horizon(0, 0), mode: SETTLE },
        ]
    );
}

#[test]
fn rounds_beyond_the_bound_are_e0506_naming_the_nets() {
    for bound in [None, Some(3u32)] {
        let log = new_log();
        let mut d = ScriptedDiscrete::new(&log);
        let a = ScriptedAnalog::new(&log);
        d.instants.extend((0..40).map(|_| changing()));
        d.names.push((7, "top.cmp.out".into()));
        let mut options = ProtocolOptions::new(1e-12);
        if let Some(b) = bound {
            options.max_rounds = b;
        }
        let want = bound.unwrap_or(20);
        let mut p = Protocol::new(d, a, options);
        let err = p.operating_point().expect_err("an instant that always changes never settles");
        let text = err.to_string();
        assert!(text.contains("E0506"), "{text}");
        assert!(text.contains(&format!("{want} rounds")), "{text}");
        assert!(text.contains("nets still changing: top.cmp.out"), "the error names the nets still changing: {text}");
        assert_eq!(calls(&log, |c| matches!(c, Call::Equilibrium)).len(), want as usize, "one solve per round");
        assert!(
            calls(&log, |c| matches!(c, Call::RunInstant { mode, .. } if mode.settle)).is_empty(),
            "no settle after a failed operating point"
        );
    }
    assert_eq!(ProtocolOptions::new(1e-12).max_rounds, 20, "the default bound");
}

#[test]
fn an_operating_point_without_unknowns_runs_the_instant_only() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    a.unknowns = false;
    d.instants.push_back(changing());
    let mut p = Protocol::new(d, a, ProtocolOptions::new(1e-12));
    p.operating_point().unwrap();
    assert_eq!(
        sequence(&log),
        [
            Call::Start,
            Call::RunInstant { at: 0, samples: vec![], limit: horizon(0, 0), mode: QUIET },
            Call::RunInstant { at: 0, samples: vec![], limit: horizon(0, 0), mode: SETTLE },
        ]
    );
}

#[test]
fn finish_at_the_operating_point_ends_the_run() {
    let log = new_log();
    let mut d = ScriptedDiscrete::new(&log);
    let a = ScriptedAnalog::new(&log);
    d.instants.push_back(Outcome { finish: true, ..Outcome::default() });
    let mut p = Protocol::new(d, a, ProtocolOptions::new(0.5));
    assert_eq!(p.operating_point().unwrap(), Ended::Finish(0.0));
    assert_eq!(p.transient(2.0).unwrap(), Ended::Finish(0.0), "the run already ended");
    let after = calls(&log, |c| matches!(c, Call::Advance(_) | Call::Propose(..) | Call::Attempt { .. }));
    assert!(after.is_empty(), "nothing runs after a FINISH: {after:?}");

    // Without a FINISH the operating point completes and the transient runs.
    let log = new_log();
    let mut p = Protocol::new(ScriptedDiscrete::new(&log), ScriptedAnalog::new(&log), ProtocolOptions::new(0.5));
    assert_eq!(p.operating_point().unwrap(), Ended::StopTime);
    assert_eq!(p.transient(1.0).unwrap(), Ended::StopTime);
}
