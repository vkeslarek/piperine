//! The transient loop of a circuit of devices through the protocol over the
//! real sub-solvers (SAI-02, SAI-03, SAI-04; abi.md §5): a clock nothing
//! analog reads adds no step; a boundary change is solved on the left value
//! at its landing and delivered after the instant, restarting the next step;
//! a rejected step leaves no trace and replays its instants; a design with no
//! continuous pin runs its instants with no solve; a commit due after the stop
//! time stays pending; `SETTLE` fires at the stop time.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::protocol_real::{FAULTED, FlakyG};
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::digital;

use piperine_abi::layout::HIGH;
use piperine_digital_solver::tick::Tick;
use piperine_solver::abi::{AnalogReference, Bound, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions, TransientAnalysisResult};

#[test]
fn a_clock_nothing_analog_reads_adds_no_step() {
    let plain = {
        let mut b = CircuitBuilder::new("rc");
        ProtocolRealCases::rc(&mut b);
        ProtocolRealCases::run(&mut b.build().expect("build"), 1.0e-3, 1.0e-5)
    };
    let clocked = {
        let mut b = CircuitBuilder::new("rc + clock");
        ProtocolRealCases::rc(&mut b);
        b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(1.0e-5))], &[ProtocolRealCases::net("clk")]);
        b.device(digital::Counter::module(), "cnt", &[], &[ProtocolRealCases::net("clk"), ProtocolRealCases::net("n")]);
        let mut circuit = b.build().expect("build");
        let result = ProtocolRealCases::run(&mut circuit, 1.0e-3, 1.0e-5);
        // The clock did run: 50 rising edges in 1 ms of 10 µs half periods.
        assert_eq!(mixed::MixedParts::count_of(&ProtocolRealCases::bytes_of(circuit.bound(), "n")), 50);
        result
    };
    assert_eq!(clocked.stats.steps_accepted, plain.stats.steps_accepted);
    assert_eq!(clocked.len(), plain.len());
    for (a, b) in clocked.iter().zip(plain.iter()) {
        assert_eq!(a.time(), b.time(), "the clock moved an accepted time");
    }
}

#[test]
fn a_boundary_change_is_delivered_after_the_landing_and_restarts_the_next() {
    // The timer drives the DAC's code HIGH at 1 µs; the DAC drives `o`, which
    // charges `c` through 1 kΩ into 1 nF. The user's order (2026-11-15): the step
    // arriving at 1 µs is solved on the value before the instant (code LOW), the
    // instant runs, and the change reaches the analog side for the next step.
    let edge = 1.0e-6;
    let mut b = CircuitBuilder::new("timer -> dac");
    let gnd = b.ground();
    let o = b.node("o");
    let c = b.node("c");
    b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(edge))], &[ProtocolRealCases::net("code")]);
    b.device(mixed::BitDigitalToAnalog::module(), "dac", &[("vref", ParamValue::Real(1.0))], &[ProtocolRealCases::net("code"), Port::Node(o.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &o, &c);
    CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-9, &c, &gnd);
    let mut circuit = b.build().expect("build");
    let result = ProtocolRealCases::run(&mut circuit, 1.5e-6, 1.0e-8);

    // The last accepted time within a tick of the edge is the landing (the
    // controller's own grid may put a step a few ulps short of it before).
    let at = result.iter().rposition(|s| (s.time() - edge).abs() < 1.0e-15).expect("a step lands on the edge");
    let landing = result.get(at).unwrap();
    let next = result.get(at + 1).expect("a step after the edge");
    assert_eq!(landing.get_node(&CircuitBuilding::node_id(&o)).unwrap(), 0.0, "the landing is solved on the left value");
    assert!((next.get_node(&CircuitBuilding::node_id(&o)).unwrap() - 1.0).abs() < 1.0e-12, "the next step reads the new code");
    // The jump lies inside the step after the landing: that step is tested by
    // the first-order jump estimate, which it passes, so nothing is rejected and no step falls to the floor.
    assert_eq!(result.stats.steps_rejected, 0, "{:?}", result.stats);
    assert_eq!(result.stats.dt_min_floor_hits, 0);
    assert_eq!(ProtocolRealCases::bytes_of(circuit.bound(), "code"), HIGH.to_vec());
}

#[test]
fn a_rejected_step_leaves_no_trace() {
    // A first step of a whole sine period is rejected by the truncation test; the
    // conductance faults once between 0.3 and 0.4 ms, a Newton failure (the first
    // attempt evaluates at 0.59 and 1 ms, outside it). A 10 µs clock runs ahead of
    // every attempt; what a rejection undoes is replayed.
    let (mut circuit, out) = ProtocolRealCases::flaky_circuit(true);
    let result = ProtocolRealCases::run(&mut circuit, 1.0e-3, 1.0e-3);
    assert!(FAULTED.with(|f| f.get()), "the Newton failure happened");
    // The store holds every edge once, although the replays ran reactions again.
    assert_eq!(mixed::MixedParts::count_of(&ProtocolRealCases::bytes_of(circuit.bound(), "n")), 50, "the discrete store left no trace");
    assert!(mixed::ReactionCounts::of("counter") > 50, "the undone instants were replayed ({})", mixed::ReactionCounts::of("counter"));
    // The term histories were restored: the run reads what a run with no fault reads.
    let (mut clean, _) = ProtocolRealCases::flaky_circuit(false);
    let reference = ProtocolRealCases::run(&mut clean, 1.0e-3, 1.0e-3);
    let (v, v_ref) = (result.last().unwrap().get_node(&CircuitBuilding::node_id(&out)).unwrap(), reference.last().unwrap().get_node(&CircuitBuilding::node_id(&out)).unwrap());
    assert!((v - v_ref).abs() < 1.0e-4, "v(out) = {v} vs {v_ref} without the fault");
    assert!(reference.stats.steps_rejected >= 1, "the truncation test rejects: {:?}", reference.stats);
    assert!(result.stats.steps_rejected > reference.stats.steps_rejected, "the Newton failure rejects too: {:?}", result.stats);
}

#[test]
fn the_default_run_copies_the_digital_state_once_per_accepted_step() {
    // HYG-19 AC 2: with every step keeping its re-entry state (the default), the full copy of the
    // digital state is taken once per accepted step: a rejected attempt (here a truncation
    // rejection and a Newton failure) is retried from the state its rewind restored, which the
    // first attempt's copy already holds. The run is the one `a_rejected_step_leaves_no_trace`
    // reads: the results are the ones of a run that copied on every attempt.
    let (mut circuit, _) = ProtocolRealCases::flaky_circuit(true);
    let result = ProtocolRealCases::run(&mut circuit, 1.0e-3, 1.0e-3);
    let rejected = result.stats.steps_rejected;
    assert!(rejected >= 2, "the run rejects attempts: {:?}", result.stats);
    // One copy per accepted step, and the two that bracket the run: the start's state and the last
    // step's, which a later run re-enters from. Before the change each rejected attempt took one more.
    let copies = circuit.bound().digital.checkpoints_taken() as usize;
    assert_eq!(copies, result.stats.steps_accepted + 2, "{copies} copies for {} accepted steps and {rejected} rejected attempts", result.stats.steps_accepted);
}

#[test]
fn a_design_without_analog_runs_every_instant_with_no_solve() {
    let mut b = CircuitBuilder::new("clock only");
    b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(1.0e-6))], &[ProtocolRealCases::net("clk")]);
    b.device(digital::Counter::module(), "cnt", &[], &[ProtocolRealCases::net("clk"), ProtocolRealCases::net("n")]);
    let mut circuit = b.build().expect("build");
    let result = ProtocolRealCases::run(&mut circuit, 1.0e-5, 1.0e-7);
    // Rising edges at 1, 3, 5, 7, 9 µs; the edge at 10 µs (falling) is the stop time.
    assert_eq!(mixed::MixedParts::count_of(&ProtocolRealCases::bytes_of(circuit.bound(), "n")), 5);
    assert_eq!(result.stats.newton_iterations, 0, "no solve");
    assert_eq!(result.stats.steps_accepted, 0);
}

#[test]
fn a_commit_due_after_the_stop_time_stays_pending() {
    let mut b = CircuitBuilder::new("rc + slow clock");
    ProtocolRealCases::rc(&mut b);
    b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(3.0e-4))], &[ProtocolRealCases::net("clk")]);
    let mut circuit = b.build().expect("build");
    ProtocolRealCases::run(&mut circuit, 5.0e-4, 1.0e-5);
    let bound = circuit.bound();
    assert_eq!(ProtocolRealCases::bytes_of(bound, "clk"), HIGH.to_vec(), "the edge at 0.3 ms applied");
    let spt = bound.digital.plan().resolution.seconds_per_tick;
    assert_eq!(bound.digital.next_due(), Some(Tick((6.0e-4 / spt).round() as i64)), "the edge at 0.6 ms waits");
}

#[test]
fn settle_fires_at_the_stop_time() {
    let mut b = CircuitBuilder::new("rc + settle");
    ProtocolRealCases::rc(&mut b);
    b.device(mixed::SettleTime::module(), "st", &[], &[]);
    let mut circuit = b.build().expect("build");
    let stop = 1.0e-3;
    let result = ProtocolRealCases::run(&mut circuit, stop, 1.0e-5);
    let last = *mixed::SettleTime::times().last().expect("SETTLE fired");
    assert!((last - stop).abs() < 1.0e-15, "the last SETTLE is at the stop time: {last}");
    assert!((result.last().unwrap().time() - stop).abs() < 1.0e-15);
}

/// The helpers of the protocol_real tests.
pub struct ProtocolRealCases;

impl ProtocolRealCases {
    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The bytes of helper `bytes_of`.
    fn bytes_of(bound: &Bound, name: &str) -> Vec<u8> {
        bound.digital.read(bound.names.nets[&format!("net:{name}")]).to_vec()
    }

    /// The run helper `run`.
    fn run(circuit: &mut CircuitInstance, stop: f64, dt: f64) -> TransientAnalysisResult {
        circuit
            .transient(TransientAnalysisOptions::new(stop, dt), Context::default())
            .expect("tran")
            .solve()
            .expect("solve")
    }

    /// A 1 V source charging 1 µF through 1 kΩ (τ = 1 ms).
    fn rc(b: &mut CircuitBuilder) -> AnalogReference {
        let gnd = b.ground();
        let top = b.node("top");
        let out = b.node("out");
        CircuitBuilding::add_voltage_source(b, "v1", 1.0, &top, &gnd);
        CircuitBuilding::add_resistor(b, "r1", 1.0e3, &top, &out);
        CircuitBuilding::add_capacitor(b, "c1", 1.0e-6, &out, &gnd);
        out
    }

    /// A 1 V, 1 kHz sine through 1 kΩ into 1 µF beside the flaky 1 mS conductance
    /// at `out`, with a 10 µs clock counted in `n`. `fault` arms the conductance.
    fn flaky_circuit(fault: bool) -> (CircuitInstance, AnalogReference) {
        FAULTED.with(|f| f.set(!fault));
        let mut b = CircuitBuilder::new("rejections");
        let gnd = b.ground();
        let top = b.node("top");
        let out = b.node("out");
        b.device(circuit::SineSource::module(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
            Port::Node(top.clone()),
            Port::Node(gnd.clone()),
        ]);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &out);
        CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-6, &out, &gnd);
        b.device(FlakyG::module(), "g1", &[], &[Port::Node(out.clone()), Port::Node(gnd)]);
        b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(1.0e-5))], &[ProtocolRealCases::net("clk")]);
        b.device(digital::Counter::module(), "cnt", &[], &[ProtocolRealCases::net("clk"), ProtocolRealCases::net("n")]);
        (b.build().expect("build"), out)
    }
}
