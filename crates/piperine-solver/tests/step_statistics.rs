//! The protocol's step statistics (feature `transient-performance`, TP-01): on scripted sides, one
//! accepted step of each end is counted under that end alone, a rejected attempt adds no end, and
//! a step retaken toward a crossing is counted once. The real analog side's counts are in the
//! tests further down this target.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;

use piperine_solver::protocol::{
    Advance, Attempt, Ended, LandingCause, ProposalBound, Protocol, ProtocolOptions, StepEnd, StepStatistics,
};
use stub_sides::{ScriptedAnalog, ScriptedDiscrete, new_log};

/// One second per tick: a time and its tick are the same number.
const SPT: f64 = 1.0;

fn sides() -> (ScriptedDiscrete, ScriptedAnalog) {
    let log = new_log();
    (ScriptedDiscrete::new(&log), ScriptedAnalog::new(&log))
}

/// The statistics after a transient to `stop` over the scripted sides.
fn statistics_of(discrete: ScriptedDiscrete, analog: ScriptedAnalog, stop: f64) -> StepStatistics {
    let mut protocol = Protocol::new(discrete, analog, ProtocolOptions::new(SPT));
    assert_eq!(protocol.transient(stop).unwrap(), Ended::StopTime);
    protocol.statistics().clone()
}

#[test]
fn the_protocol_counts_one_landing_of_each_cause() {
    let (mut discrete, mut analog) = sides();
    let causes = [
        LandingCause::AnalogReadChanged,
        LandingCause::ContinuousRead,
        LandingCause::DeclaredReach,
        LandingCause::Irreversible,
        LandingCause::HostWaiter,
    ];
    for (index, cause) in causes.iter().enumerate() {
        discrete.advances.push_back(Advance::Landing { tick: index as i64 + 1, cause: *cause });
    }
    // after the five landings (now = 5): a crossing step, a step at max_step, a capped step, the
    // controller's own and the stop
    discrete.advances.extend([Advance::Reached; 5]);
    analog.proposals.extend([10.0, 10.0, 10.0, 10.0, 10.0, 7.0, 7.0, 8.0, 9.0, 10.0]);
    analog.bounds.extend([ProposalBound::Controller; 6]);
    analog.bounds.extend([ProposalBound::MaxStep, ProposalBound::Cap, ProposalBound::Controller, ProposalBound::Stop]);
    // the sixth step is proposed to 7 and located at 6
    analog.attempts.extend([Attempt::Converged; 5]);
    analog.attempts.push_back(Attempt::LandedEarlier(6.0));

    let statistics = statistics_of(discrete, analog, 10.0);

    for cause in causes {
        assert_eq!(statistics.landings(cause), 1, "{cause:?}");
    }
    for end in [StepEnd::Crossing, StepEnd::MaxStep, StepEnd::Cap, StepEnd::Proposal, StepEnd::Stop] {
        assert_eq!(statistics.ended(end), 1, "{end:?}");
    }
    assert_eq!(statistics.accepted(), 10);
    assert_eq!(statistics.instants_run(), 10, "one instant per accepted step");
    assert_eq!(statistics.retaken(), 1);
}

#[test]
fn a_rejected_attempt_adds_no_end() {
    let (discrete, mut analog) = sides();
    analog.proposals.extend([4.0, 4.0]);
    analog.attempts.extend([Attempt::Rejected, Attempt::Converged]);
    analog.bounds.extend([ProposalBound::Stop, ProposalBound::Stop]);

    let statistics = statistics_of(discrete, analog, 4.0);

    assert_eq!(statistics.accepted(), 1, "the rejected attempt is no accepted step");
    assert_eq!(statistics.ended(StepEnd::Stop), 1);
    assert_eq!(statistics.retaken(), 0, "a rejection is not a retake toward a crossing");
}

#[test]
fn a_step_retaken_toward_a_crossing_is_counted_once() {
    let (discrete, mut analog) = sides();
    analog.proposals.push_back(8.0);
    analog.bounds.extend([ProposalBound::Stop; 2]);
    analog.attempts.push_back(Attempt::LandedEarlier(5.0));

    let statistics = statistics_of(discrete, analog, 8.0);

    assert_eq!(statistics.retaken(), 1);
    assert_eq!(statistics.ended(StepEnd::Crossing), 1, "the retaken step ends at the crossing");
    assert_eq!(statistics.ended(StepEnd::Stop), 1, "and the step after it reaches the stop");
    assert_eq!(statistics.accepted(), 2);
}
