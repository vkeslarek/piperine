//! The solver's host surface on a circuit of devices (SAI-05, SAI-07; abi.md
//! C5, A5, §5 R6 R7): `Solver::with_sink` receives every observation of a
//! real mixed run in instant order and none of an undone instant; an
//! irreversible reaction and a host waiter (`Solver::on_event`) run once at
//! an accepted time; `FINISH` ends the analysis at its instant.

// The targets use the libraries selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use abi_devices::{add_capacitor, add_resistor, add_vsrc, vsine};
use abi_mixed::{
    arm_fault, comparator, dac, delay, dff, faulted, finisher, flaky_g, inverter, reactions, recorder, stub, tracer,
};
use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{AnalogReference, Bound, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Solver, TransientAnalysisOptions, TransientAnalysisResult};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

fn bytes_of(bound: &Bound, name: &str) -> Vec<u8> {
    bound.digital.read(bound.names.nets[&format!("net:{name}")]).to_vec()
}

/// A sink that keeps what it sees.
struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

/// A solver over `circuit` whose sink collects into the returned list.
fn with_sink(circuit: CircuitInstance, stop: f64, dt: f64) -> (Solver, Rc<RefCell<Vec<Observation>>>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let solver = Solver::new(circuit)
        .with_tran_opts(TransientAnalysisOptions::new(stop, dt))
        .with_sink(Box::new(Collect(Rc::clone(&seen))))
        .build();
    (solver, seen)
}

fn tran(solver: &mut Solver) -> TransientAnalysisResult {
    solver.tran().expect("tran").solve().expect("solve")
}

/// A 1 V, 1 kHz sine through 1 kΩ into 1 µF beside a 1 mS conductance that
/// faults once when armed: its first step (a whole period) is rejected by the
/// truncation test.
fn sine_rc(b: &mut CircuitBuilder) -> AnalogReference {
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    b.device(vsine(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
        Port::Node(top.clone()),
        Port::Node(gnd.clone()),
    ]);
    add_resistor(b, "r1", 1.0e3, &top, &out);
    add_capacitor(b, "c1", 1.0e-6, &out, &gnd);
    b.device(flaky_g(), "g1", &[], &[Port::Node(out.clone()), Port::Node(gnd)]);
    out
}

fn records(seen: &[Observation]) -> Vec<Vec<u8>> {
    seen.iter()
        .filter_map(|o| match &o.kind {
            ObservationKind::Record { bytes } => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_sink_receives_everything_in_instant_order() {
    // The comparator, flip-flop and DAC loop of `operating_point.rs` (a crossing
    // between the rounds of the operating point), a recorder fired at 2 µs and a
    // conductance writing a TRACE record at every evaluation.
    let mut b = CircuitBuilder::new("mixed");
    let gnd = b.ground();
    let a = b.node("a");
    let r = b.node("r");
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(dac(), "dac", &[("vref", ParamValue::Real(1.0))], &[net("code"), Port::Node(a.clone())]);
    b.device(comparator(), "cmp", &[], &[Port::Node(a.clone()), Port::Node(r.clone()), net("cmp")]);
    b.device(dff(), "ff", &[], &[net("cmp"), net("code"), net("q")]);
    b.device(inverter(), "inv", &[], &[net("q"), net("code")]);
    b.device(tracer(), "tr", &[], &[Port::Node(r)]);
    b.device(delay(), "kick", &[("at", ParamValue::Real(2.0e-6))], &[net("go")]);
    b.device(recorder(), "rec", &[], &[net("go"), net("log")]);
    let (mut solver, seen) = with_sink(b.build().expect("build"), 5.0e-6, 1.0e-7);
    tran(&mut solver);
    let seen = seen.borrow();

    let has = |f: &dyn Fn(&Observation) -> bool| seen.iter().any(f);
    assert!(seen.iter().all(|o| !o.path.is_empty()), "every observation names its path");
    assert!(has(&|o| matches!(o.kind, ObservationKind::Change { .. }) && o.path == "q"), "q changed");
    assert!(has(&|o| matches!(o.kind, ObservationKind::Commit { .. }) && o.path == "cmp"), "the comparator committed");
    assert!(
        has(&|o| o.kind == ObservationKind::Crossing && o.analysis == AnalysisKind::OperatingPoint && o.time == 0.0),
        "a crossing between the rounds of the operating point"
    );
    let recs: Vec<&Observation> = seen.iter().filter(|o| matches!(o.kind, ObservationKind::Record { .. })).collect();
    assert_eq!(recs.len(), 8, "the recorder's eight records");
    assert!(recs.iter().all(|o| o.analysis == AnalysisKind::Transient && (o.time - 2.0e-6).abs() < 1.0e-15));
    let traces: Vec<u32> = seen
        .iter()
        .filter_map(|o| match &o.kind {
            ObservationKind::Trace { iteration, bytes } => {
                assert_eq!(o.path, "tr.seen");
                assert_eq!(bytes, &vec![1, 1]);
                Some(*iteration)
            }
            _ => None,
        })
        .collect();
    assert!(traces.iter().any(|i| *i >= 1), "a TRACE record carries its Newton iteration: {traces:?}");
    for pair in seen.windows(2) {
        assert!(pair[0].time <= pair[1].time, "instant order: {:?} before {:?}", pair[0], pair[1]);
    }
}

#[test]
fn records_of_an_undone_instant_do_not_reach_the_sink() {
    // The recorder fires at 0.2 ms, inside the first attempt (to 1 ms), which the
    // truncation test rejects: that instant is undone and replayed.
    let mut b = CircuitBuilder::new("undone");
    sine_rc(&mut b);
    b.device(delay(), "kick", &[("at", ParamValue::Real(2.0e-4))], &[net("go")]);
    b.device(recorder(), "rec", &[], &[net("go"), net("log")]);
    let (mut solver, seen) = with_sink(b.build().expect("build"), 1.0e-3, 1.0e-3);
    let result = tran(&mut solver);
    assert!(result.stats.steps_rejected >= 1);
    assert!(reactions("recorder") >= 2, "the instant ran again after the rejection ({})", reactions("recorder"));
    let expected: Vec<Vec<u8>> = (1..=8u8).map(|i| vec![i, 1]).collect();
    assert_eq!(records(&seen.borrow()), expected, "each record reaches the sink once");
}

#[test]
fn an_irreversible_reaction_runs_once_at_an_accepted_time() {
    // The stub fires at 0.2 ms. The first attempt that reaches it is shortened to
    // land there, and its Newton solve fails (the conductance faults at 0.117 ms,
    // inside that attempt): the stub must not run until the landing is accepted.
    arm_fault((1.1e-4, 1.2e-4));
    let at = 2.0e-4;
    let mut b = CircuitBuilder::new("irreversible");
    sine_rc(&mut b);
    b.device(delay(), "kick", &[("at", ParamValue::Real(at))], &[net("go")]);
    b.device(stub(), "emu", &[], &[net("go"), net("out")]);
    let (mut solver, _) = with_sink(b.build().expect("build"), 1.0e-3, 1.0e-3);
    let result = tran(&mut solver);
    assert!(faulted(), "the Newton failure happened");
    assert!(result.stats.steps_rejected >= 1);
    assert_eq!(reactions("stub"), 1, "the irreversible reaction ran once");
    assert!(result.iter().any(|s| (s.time() - at).abs() < 1.0e-15), "its instant is an accepted time");
    assert_eq!(bytes_of(solver.circuit().bound().unwrap(), "out"), HIGH.to_vec());
}

#[test]
fn finish_ends_the_analysis_at_an_accepted_time() {
    let at = 3.0e-4;
    let mut b = CircuitBuilder::new("finish");
    sine_rc(&mut b);
    b.device(delay(), "kick", &[("at", ParamValue::Real(at))], &[net("go")]);
    b.device(finisher(), "end", &[], &[net("go")]);
    let (mut solver, _) = with_sink(b.build().expect("build"), 1.0e-3, 1.0e-5);
    let result = tran(&mut solver);
    let last = result.last().unwrap().time();
    assert!((last - at).abs() < 1.0e-15, "the last recorded time is the FINISH instant: {last}");
    assert!(reactions("finisher") >= 1);
}

#[test]
fn a_host_waiter_is_called_once_and_its_write_is_due_next_microstep() {
    let at = 2.5e-4;
    let mut b = CircuitBuilder::new("waiter");
    sine_rc(&mut b);
    b.device(delay(), "kick", &[("at", ParamValue::Real(at))], &[net("go")]);
    b.device(inverter(), "inv", &[], &[net("h"), net("hy")]);
    let circuit = b.build().expect("build");
    let h = circuit.bound().unwrap().names.nets["net:h"].0;
    let (mut solver, seen) = with_sink(circuit, 1.0e-3, 1.0e-5);
    let calls = Rc::new(Cell::new(0u32));
    let times = Rc::new(RefCell::new(Vec::new()));
    let (c, t) = (Rc::clone(&calls), Rc::clone(&times));
    solver.on_event(
        "go",
        Box::new(move |cx| {
            c.set(c.get() + 1);
            t.borrow_mut().push(cx.time());
            cx.set(h, 0, &HIGH).expect("the host write");
        }),
    );
    let result = tran(&mut solver);

    assert_eq!(calls.get(), 1, "called once");
    assert!((times.borrow()[0] - at).abs() < 1.0e-15, "at its instant");
    assert!(result.iter().any(|s| (s.time() - at).abs() < 1.0e-15), "an accepted time");
    let bound = solver.circuit().bound().unwrap();
    assert_eq!(bytes_of(bound, "h"), HIGH.to_vec());
    assert_eq!(bytes_of(bound, "hy"), LOW.to_vec(), "the write ran its readers");
    let h_changes: Vec<f64> = seen
        .borrow()
        .iter()
        .filter(|o| o.path == "h" && matches!(o.kind, ObservationKind::Change { .. }))
        .map(|o| o.time)
        .collect();
    assert_eq!(h_changes.len(), 1);
    assert!((h_changes[0] - at).abs() < 1.0e-15, "applied within the instant");
}
