//! The steps of a transient that end on a landing or read a jump (feature `transient-performance`,
//! TP-04, TP-05, TP-09 AC 2). A 1 V, 1 kHz sine through 1 kOhm into 1 uF takes a first step of a
//! whole period, which the truncation test refuses; a digital reaction scheduled inside that step
//! makes its end a landing. The step that ends on that landing reads no change, so it is an
//! ordinary step and is tested like any other.

// The targets use the libraries selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::RefCell;
use std::rc::Rc;

use abi_devices::{add_capacitor, add_resistor, vsine};
use abi_mixed::{delay, recorder, reactions, sampler};
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions, TransientAnalysisResult};
use piperine_solver::protocol::{Observation, ObservationKind, Sink};

/// The time of the digital reaction, inside the first step of a whole period.
const LANDING: f64 = 8.0e-4;

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

/// A sink that keeps what it sees.
struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, observation: &Observation) {
        self.0.borrow_mut().push(observation.clone());
    }
}

/// The sine-driven RC low-pass; returns its output node.
fn sine_rc(builder: &mut CircuitBuilder) -> AnalogReference {
    let ground = builder.ground();
    let top = builder.node("top");
    let out = builder.node("out");
    builder.device(
        vsine(),
        "v1",
        &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))],
        &[Port::Node(top.clone()), Port::Node(ground.clone())],
    );
    add_resistor(builder, "r1", 1.0e3, &top, &out);
    add_capacitor(builder, "c1", 1.0e-6, &out, &ground);
    out
}

/// The run of the sine RC to one period with a reaction at [`LANDING`] that reads the output (a
/// landing under every rule) and a recorder on the same event; returns the result and the records.
fn run_with_a_landing_in_the_first_step() -> (TransientAnalysisResult, Vec<Vec<u8>>) {
    let mut builder = CircuitBuilder::new("landing inside the first step");
    let out = sine_rc(&mut builder);
    builder.device(delay(), "kick", &[("at", ParamValue::Real(LANDING))], &[net("go")]);
    builder.device(sampler(), "sample", &[], &[Port::Node(out), net("go"), net("sampled")]);
    builder.device(recorder(), "rec", &[], &[net("go"), net("log")]);
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

#[test]
fn a_landing_step_over_the_tolerance_is_rejected_and_retaken_smaller() {
    let (result, _) = run_with_a_landing_in_the_first_step();
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
    let (result, records) = run_with_a_landing_in_the_first_step();
    assert!(result.stats.rejected_truncation >= 1);
    assert!(reactions("recorder") >= 2, "the instant ran again after the rejection ({})", reactions("recorder"));
    let expected: Vec<Vec<u8>> = (1..=8u8).map(|i| vec![i, 1]).collect();
    assert_eq!(records, expected, "each record reaches the sink once");
}
