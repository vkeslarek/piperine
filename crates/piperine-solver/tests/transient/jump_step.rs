//! The steps of a transient that end on a landing or read a jump (feature `transient-performance`,
//! TP-04, TP-05, TP-09 AC 2). A 1 V, 1 kHz sine through 1 kOhm into 1 uF takes a first step of a
//! whole period, which the truncation test refuses; a digital reaction scheduled inside that step
//! makes its end a landing. The step that ends on that landing reads no change, so it is an
//! ordinary step and is tested like any other. Further down, the step that reads a jump (a delivery
//! under a restart): an ideal 1 V step read by an RC, and the three ways that step's own estimate
//! can go (within the tolerance, over it, over it at the floor).

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions, TransientAnalysisResult};
use piperine_solver::protocol::{Observation, ObservationKind, Sink};

/// The time of the digital reaction, inside the first step of a whole period.
const LANDING: f64 = 8.0e-4;

/// A sink that keeps what it sees.
struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, observation: &Observation) {
        self.0.borrow_mut().push(observation.clone());
    }
}

/// The circuits and runs of this target.
struct Fixture;

impl Fixture {
    /// The port of the net called `name`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The sine-driven RC low-pass; returns its output node.
    fn sine_rc(builder: &mut CircuitBuilder) -> AnalogReference {
        let ground = builder.ground();
        let top = builder.node("top");
        let out = builder.node("out");
        builder.device(
            circuit::SineSource::module(),
            "v1",
            &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))],
            &[Port::Node(top.clone()), Port::Node(ground.clone())],
        );
        CircuitBuilding::add_resistor(builder, "r1", 1.0e3, &top, &out);
        CircuitBuilding::add_capacitor(builder, "c1", 1.0e-6, &out, &ground);
        out
    }

    /// The run of the sine RC to one period with a reaction at [`LANDING`] that reads the output (a
    /// landing under every rule) and a recorder on the same event; returns the result and the records.
    fn run_with_a_landing_in_the_first_step() -> (TransientAnalysisResult, Vec<Vec<u8>>) {
        let mut builder = CircuitBuilder::new("landing inside the first step");
        let out = Fixture::sine_rc(&mut builder);
        builder.device(mixed::Delay::module(), "kick", &[("at", ParamValue::Real(LANDING))], &[Fixture::net("go")]);
        builder.device(mixed::Sampler::module(), "sample", &[], &[Port::Node(out), Fixture::net("go"), Fixture::net("sampled")]);
        builder.device(mixed::Recorder::module(), "rec", &[], &[Fixture::net("go"), Fixture::net("log")]);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut solver = Solver::new(builder.build().expect("build"))
            .with_tran_opts(TransientAnalysisOptions::new(1.0e-3, 1.0e-3))
            .with_sink(Box::new(Collect(Rc::clone(&seen))))
            .build();
        let result = solver.tran().expect("tran").solve().expect("solve");
        let records = seen
            .borrow()
            .iter()
            .filter_map(|observation| match &observation.kind {
                ObservationKind::Record { bytes, .. } => Some(bytes.clone()),
                _ => None,
            })
            .collect();
        (result, records)
    }

    /// An RC low-pass of `resistance` and `capacitance` behind a 1 V DAC whose code the host raises at
    /// [`JUMP_AT`] when `raised` is set: an ideal 1 V step. Returns the result and the output node.
    fn rc_behind_a_step(
        resistance: f64,
        capacitance: f64,
        raised: bool,
        options: TransientAnalysisOptions,
    ) -> (TransientAnalysisResult, AnalogReference) {
        let mut builder = CircuitBuilder::new("ideal step into an rc");
        let ground = builder.ground();
        let source = builder.node("source");
        let output = builder.node("output");
        builder.device(
            mixed::DigitalToAnalog::module(),
            "dac",
            &[("vref", ParamValue::Real(1.0))],
            &[Fixture::net("code"), Port::Node(source.clone())],
        );
        CircuitBuilding::add_resistor(&mut builder, "r1", resistance, &source, &output);
        CircuitBuilding::add_capacitor(&mut builder, "c1", capacitance, &output, &ground);
        let mut circuit = builder.build().expect("build");
        let mut solver = circuit.transient(options, Context::default()).expect("tran");
        if raised {
            solver.schedule_write(JUMP_AT, "code", ParamValue::Bool(true));
        }
        (solver.solve().expect("solve"), output)
    }

    /// The output voltage at `time`, linearly interpolated between the two accepted steps around it.
    fn output_at(result: &TransientAnalysisResult, output: &AnalogReference, time: f64) -> f64 {
        let node = CircuitBuilding::node_id(output);
        let after = (0..result.len()).find(|&index| result.get(index).unwrap().time() >= time).expect("the run reaches it");
        let (late, early) = (result.get(after).unwrap(), result.get(after - 1).unwrap());
        let (v_late, v_early) = (late.get_node(&node).unwrap(), early.get_node(&node).unwrap());
        v_early + (v_late - v_early) * (time - early.time()) / (late.time() - early.time())
    }
}

#[test]
fn a_landing_step_over_the_tolerance_is_rejected_and_retaken_smaller() {
    let (result, _) = Fixture::run_with_a_landing_in_the_first_step();
    assert!(result.stats.rejected_truncation >= 1, "the step to the landing fails the truncation test");
    let first = result.get(1).expect("a first accepted step").time();
    assert!(first < LANDING, "the retaken step is smaller than the one to the landing: {first:e}");
    assert!(
        (0..result.len()).any(|index| (result.get(index).unwrap().time() - LANDING).abs() < 1.0e-12),
        "the landing is still a time point of the run"
    );
}

#[test]
fn the_instants_run_ahead_of_a_rejected_step_leave_no_record() {
    let (result, records) = Fixture::run_with_a_landing_in_the_first_step();
    assert!(result.stats.rejected_truncation >= 1);
    assert!(mixed::ReactionCounts::of("recorder") >= 2, "the instant ran again after the rejection ({})", mixed::ReactionCounts::of("recorder"));
    let expected: Vec<Vec<u8>> = (1..=8u8).map(|i| vec![i]).collect();
    assert_eq!(records, expected, "each record reaches the sink once");
}

/// The time the host raises the DAC's code in the jump fixtures.
const JUMP_AT: f64 = 2.0e-5;

#[test]
fn an_ideal_step_is_read_by_an_rc_to_one_minus_e_to_the_minus_one() {
    // 1 kOhm and 1 nF: the time constant is 1 us.
    let tolerance = Context::default().tolerances.reltol;
    let options = TransientAnalysisOptions::new(JUMP_AT + 3.0e-6, 1.0e-7);
    let (result, output) = Fixture::rc_behind_a_step(1.0e3, 1.0e-9, true, options);
    let expected = 1.0 - (-1.0_f64).exp();
    let measured = Fixture::output_at(&result, &output, JUMP_AT + 1.0e-6);
    assert!((measured - expected).abs() <= tolerance, "{measured} against {expected} (reltol {tolerance})");
    assert_eq!(result.stats.jump_steps, 1, "one delivery, one step that reads it");
}

#[test]
fn a_jump_step_over_the_tolerance_is_retaken_smaller() {
    // The circuit settles, so the last step before the jump is long; a tenth of it is still many
    // time constants of a 1 ns RC, and the first-order estimate refuses it.
    let options = TransientAnalysisOptions::new(JUMP_AT + 1.0e-6, 1.0e-7).with_dt_min(1.0e-15);
    let (result, _) = Fixture::rc_behind_a_step(1.0e3, 1.0e-12, true, options);
    assert_eq!(result.stats.jump_steps, 1);
    assert!(result.stats.jump_retakes >= 1, "the jump step was retaken ({})", result.stats.jump_retakes);
    assert_eq!(result.stats.floor_accepts, 0, "and a smaller step passed above the floor");
}

#[test]
fn a_jump_step_at_the_floor_is_accepted_and_counted() {
    // The floor is far above the 1 ns time constant: the step cannot be smaller, so it is accepted.
    let options = TransientAnalysisOptions::new(JUMP_AT + 1.0e-6, 1.0e-7).with_dt_min(1.0e-7);
    let (result, _) = Fixture::rc_behind_a_step(1.0e3, 1.0e-12, true, options.clone());
    let (quiet, _) = Fixture::rc_behind_a_step(1.0e3, 1.0e-12, false, options);
    assert_eq!(result.stats.jump_steps, 1);
    assert_eq!(result.stats.jump_retakes, 0, "nothing smaller than the floor is tried");
    assert_eq!(quiet.stats.floor_accepts, 0, "nothing fails the estimate without the jump");
    assert!(result.stats.floor_accepts >= 1, "the jump step failed the estimate at the floor and was accepted");
}

#[test]
fn a_jump_step_within_the_tolerance_is_not_retaken() {
    // A cap of 1 ns on every step: the jump step is a tenth of that against a 1 us time constant.
    let options = TransientAnalysisOptions::new(JUMP_AT + 1.0e-6, 1.0e-9).with_dt_max(1.0e-9);
    let (result, _) = Fixture::rc_behind_a_step(1.0e3, 1.0e-9, true, options);
    assert_eq!(result.stats.jump_steps, 1);
    assert_eq!(result.stats.jump_retakes, 0);
    assert_eq!(result.stats.floor_accepts, 0);
}
