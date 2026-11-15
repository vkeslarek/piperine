//! Crossings, samples and step caps on the transient of a circuit of devices
//! (SAI-11, SAI-02; abi.md C6, R2, §4.3): a `CROSS` watch that fired is a
//! commit to its event pin at microstep 0 of the landing; a reaction reads the
//! accepted solution; the smallest of several `CAP` watches bounds the next
//! step, whatever their order.

// The targets use the libraries selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::RefCell;
use std::rc::Rc;

use abi_devices::{add_capacitor, add_resistor, add_vsrc, node_id, vsine};
use abi_mixed::{capped, comparator, delay, reactions, sampler};
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

/// A 1 V, 1 kHz sine at `top`, loaded by 1 kΩ and 1 µF.
fn sine(b: &mut CircuitBuilder) -> AnalogReference {
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    b.device(vsine(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
        Port::Node(top.clone()),
        Port::Node(gnd.clone()),
    ]);
    add_resistor(b, "r1", 1.0e3, &top, &out);
    add_capacitor(b, "c1", 1.0e-6, &out, &gnd);
    top
}

#[test]
fn a_fired_crossing_is_a_commit_at_the_landing() {
    let mut b = CircuitBuilder::new("crossing");
    let top = sine(&mut b);
    let gnd = b.ground();
    let r = b.node("r");
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(top.clone()), Port::Node(r), net("out")]);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut solver = Solver::new(b.build().expect("build"))
        .with_tran_opts(TransientAnalysisOptions::new(2.0e-4, 1.0e-5))
        .with_sink(Box::new(Collect(Rc::clone(&seen))))
        .build();
    let result = solver.tran().expect("tran").solve().expect("solve");
    let seen = seen.borrow();

    let crossings: Vec<(usize, &Observation)> =
        seen.iter().enumerate().filter(|(_, o)| o.kind == ObservationKind::Crossing && o.path == "cmp.up").collect();
    assert_eq!(crossings.len(), 1, "the sine crosses 0.5 V upward once before 0.2 ms");
    let (ci, crossing) = crossings[0];
    assert_eq!(crossing.analysis, AnalysisKind::Transient);
    // solver-crossings will locate the crossing inside the step; today it fires at
    // the first accepted solution after the sign change, which this asserts.
    // The instant is the accepted time's tick (the step ends on the controller's
    // own grid, within one tick of it).
    let k = result
        .iter()
        .position(|s| (s.time() - crossing.time).abs() < 1.0e-15)
        .expect("the crossing is at an accepted time");
    let v = |i: usize| result.get(i).unwrap().get_node(&node_id(&top)).unwrap();
    assert!(v(k - 1) <= 0.5 && v(k) > 0.5, "the first accepted time after the sign change: {} {}", v(k - 1), v(k));
    // `out` changes one microstep later, in the same instant.
    let (oi, out) = seen
        .iter()
        .enumerate()
        .find(|(_, o)| o.path == "out" && o.time > 0.0 && matches!(o.kind, ObservationKind::Change { .. }))
        .expect("out changed");
    assert_eq!(out.time, crossing.time);
    assert!(oi > ci, "after the firing");
    assert_eq!(reactions("comparator"), 1);
}

#[test]
fn a_reaction_reads_the_accepted_solution() {
    let at = 2.0e-4;
    let mut b = CircuitBuilder::new("sampler");
    let top = sine(&mut b);
    b.device(delay(), "kick", &[("at", ParamValue::Real(at))], &[net("go")]);
    b.device(sampler(), "smp", &[], &[Port::Node(top.clone()), net("go"), net("s")]);
    let mut solver = Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(5.0e-4, 1.0e-5)).build();
    let result = solver.tran().expect("tran").solve().expect("solve");

    let landing = result.iter().find(|s| (s.time() - at).abs() < 1.0e-15).expect("the probing instant is a landing");
    let bound = solver.circuit().bound().unwrap();
    let s = bound.digital.read(bound.names.nets["net:s"]).to_vec();
    assert_eq!(s[8], 1, "the sampler ran");
    let sample = f64::from_le_bytes(s[..8].try_into().unwrap());
    assert_eq!(sample, landing.get_node(&node_id(&top)).unwrap(), "the accepted solution at its instant");
    assert!(sample > 0.5, "sin(2π·0.2) = 0.95: {sample}");
}

#[test]
fn a_cap_watch_bounds_the_next_step() {
    // Two CAP watches, 4 ns and 1 ns, placed in both orders: the smallest bounds
    // every step, and the controller reaches it (a 10 ns dt asks for more).
    for caps in [[4.0e-9, 1.0e-9], [1.0e-9, 4.0e-9]] {
        let mut b = CircuitBuilder::new("capped sine");
        let top = sine(&mut b);
        for (i, cap) in caps.into_iter().enumerate() {
            b.device(capped(), &format!("cap{i}"), &[("cap", ParamValue::Real(cap))], &[Port::Node(top.clone())]);
        }
        let mut solver = Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8)).build();
        let result = solver.tran().expect("tran").solve().expect("solve");
        assert!(result.len() > 200, "at least 200 steps of 1 ns: {}", result.len());
        let mut widest = 0.0_f64;
        for pair in result.iter().collect::<Vec<_>>().windows(2) {
            let dt = pair[1].time() - pair[0].time();
            assert!(dt <= 1.0e-9 * (1.0 + 1.0e-9), "caps {caps:?}: a step of {dt:e} s past the 1 ns cap at t = {:e}", pair[0].time());
            widest = widest.max(dt);
        }
        assert!(widest > 0.99e-9, "caps {caps:?}: the steps reach the smallest cap, widest {widest:e} s");
    }
}
