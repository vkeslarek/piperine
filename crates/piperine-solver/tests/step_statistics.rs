//! The protocol's step statistics (feature `transient-performance`, TP-01): on scripted sides, one
//! accepted step of each end is counted under that end alone, a rejected attempt adds no end, and
//! a step retaken toward a crossing is counted once. Further down, a real analog side: an RC
//! behind a digital switch reports one jump step per delivery, and a step forced over the
//! tolerance reports its rejection by cause.

#[allow(dead_code)]
#[path = "support/stub_sides.rs"]
mod stub_sides;
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use abi_devices::{add_capacitor, add_resistor, add_vsrc, vsine};
use abi_mixed::dac;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{
    CircuitBuilder, Context, SolverStats, TransientAnalysisOptions, TransientAnalysisResult,
};
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

/// An RC low-pass (1 kOhm, 100 nF) behind a DAC whose `code` the host raises at 2 us.
fn rc_behind_a_switch() -> TransientAnalysisResult {
    let mut builder = CircuitBuilder::new("switched rc");
    let ground = builder.ground();
    let source = builder.node("source");
    let output = builder.node("output");
    builder.device(
        dac(),
        "dac",
        &[("vref", ParamValue::Real(1.0))],
        &[Port::Net("code".to_string()), Port::Node(source.clone())],
    );
    add_resistor(&mut builder, "r1", 1.0e3, &source, &output);
    add_capacitor(&mut builder, "c1", 1.0e-7, &output, &ground);
    let mut circuit = builder.build().expect("build");
    let mut solver = circuit.transient(TransientAnalysisOptions::new(2.0e-4, 1.0e-6), Context::default()).expect("tran");
    solver.schedule_write(2.0e-6, "code", ParamValue::Bool(true));
    solver.solve().expect("solve")
}

/// A sine-driven RC low-pass with a first step far over the tolerance: Newton always converges.
fn rc_over_the_tolerance() -> TransientAnalysisResult {
    let mut builder = CircuitBuilder::new("rc over the tolerance");
    let ground = builder.ground();
    let top = builder.node("top");
    let mid = builder.node("mid");
    builder.device(
        vsine(),
        "vsin",
        &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1000.0))],
        &[Port::Node(top.clone()), Port::Node(ground.clone())],
    );
    add_resistor(&mut builder, "r1", 1.0e3, &top, &mid);
    add_capacitor(&mut builder, "c1", 1.0e-7, &mid, &ground);
    let mut circuit = builder.build().expect("build");
    circuit
        .transient(TransientAnalysisOptions::new(1.0e-4, 1.0e-5), Context::default())
        .expect("tran")
        .solve()
        .expect("solve")
}

/// The rejections of a run are the sum of their two causes.
fn assert_rejections_add_up(stats: &SolverStats) {
    assert_eq!(stats.steps_rejected, stats.rejected_truncation + stats.rejected_newton);
}

#[test]
fn the_analog_side_reports_rejections_by_cause_and_jump_steps() {
    let switched = rc_behind_a_switch();
    assert_eq!(switched.stats.jump_steps, 1, "one delivery, one step that reads it");
    assert_rejections_add_up(&switched.stats);

    let over = rc_over_the_tolerance();
    assert!(over.stats.rejected_truncation >= 1, "the first step fails the tolerance");
    assert_eq!(over.stats.rejected_newton, 0, "and Newton converges on every attempt");
    assert_eq!(over.stats.jump_steps, 0, "nothing was delivered under a restart");
    assert_rejections_add_up(&over.stats);
}

#[test]
fn the_result_returns_the_protocols_statistics() {
    let switched = rc_behind_a_switch();
    let statistics = switched.statistics();
    assert_eq!(statistics.accepted() as usize, switched.stats.steps_accepted, "one end per accepted step");
    assert!(statistics.instants_run() > 0);
}

#[test]
fn a_step_at_max_step_is_counted() {
    let mut builder = CircuitBuilder::new("steady");
    let ground = builder.ground();
    let top = builder.node("top");
    add_vsrc(&mut builder, "v1", 1.0, &top, &ground);
    add_resistor(&mut builder, "r1", 1.0e3, &top, &ground);
    let mut circuit = builder.build().expect("build");
    let options = TransientAnalysisOptions::new(1.0e-4, 1.0e-6).with_dt_max(1.0e-5);
    let result = circuit.transient(options, Context::default()).expect("tran").solve().expect("solve");
    assert!(result.stats.steps_at_max_step > 0, "a circuit with no dynamics grows to max_step");
    assert_eq!(result.stats.steps_at_max_step as u64, result.statistics().ended(StepEnd::MaxStep));
}
