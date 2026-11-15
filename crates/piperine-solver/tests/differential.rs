//! The differential oracle (SAI-13, SAI-02): each circuit is built twice, from
//! the `Element` doubles of `helpers/devices.rs` and from their `Device` ports
//! of `helpers/abi_devices.rs`, and the two are compared unknown by unknown at
//! 1e-12 relative, with equal accepted step counts and Newton iteration
//! counts. The target exists while both contracts do.

// Each library is used selectively.
#[allow(dead_code)]
#[path = "helpers/devices.rs"]
mod elements;
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod devices;

use std::collections::HashMap;
use std::sync::Arc;

use piperine_solver::abi::{AnalogReference, AnalogVariable, InitialValue, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions};

const REL: f64 = 1.0e-12;

/// `|a - b| <= REL * max(|a|, |b|)`, the floor being a number no circuit here can produce.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= REL * a.abs().max(b.abs()) + 1.0e-30
}

fn assert_close(what: &str, a: f64, b: f64) {
    assert!(close(a, b), "{what}: element {a:e} vs device {b:e}");
}

/// A solution by unknown index: both contracts allocate nodes, then branches, in the same order.
fn by_index(circuit: &CircuitInstance, values: &HashMap<Arc<AnalogVariable>, f64>) -> Vec<f64> {
    let mut refs: Vec<&AnalogReference> = circuit.netlist().all_references();
    refs.retain(|r| r.idx().is_some());
    refs.sort_by_key(|r| r.idx());
    refs.iter().map(|r| values[r.variable()]).collect()
}

/// The circuits of the oracle, each on both contracts. Nodes: `top`, `mid`.
enum Fixture {
    Divider,
    Diode,
    DiodeSeries,
    Rc,
    SineRc,
}

fn element_circuit(f: &Fixture) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("elements");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    match f {
        Fixture::Divider => {
            b.element(Box::new(elements::Vsrc::new("v1", 7.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.5e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::R::new("r2", 4.5e3, mid.clone(), gnd.clone())));
        }
        Fixture::Diode => {
            b.element(Box::new(elements::Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.0e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::D::new("d1", 1.0e-14, mid.clone(), gnd.clone())));
        }
        Fixture::DiodeSeries => {
            let low = b.node("low");
            b.element(Box::new(elements::Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 470.0, top.clone(), mid.clone())));
            b.element(Box::new(elements::D::new("d1", 1.0e-14, mid.clone(), low.clone())));
            b.element(Box::new(elements::R::new("r2", 22.0, low, gnd.clone())));
        }
        Fixture::Rc => {
            b.element(Box::new(elements::Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.0e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::C::new("c1", 1.0e-6, mid.clone(), gnd.clone())));
        }
        Fixture::SineRc => {
            b.element(Box::new(elements::Vsine::new("v1", 5.0, 1.0e3, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.0e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::C::new("c1", 1.0e-6, mid.clone(), gnd.clone())));
        }
    }
    (b.build().expect("build"), mid)
}

fn device_circuit(f: &Fixture) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("devices");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    match f {
        Fixture::Divider => {
            devices::add_vsrc(&mut b, "v1", 7.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 1.5e3, &top, &mid);
            devices::add_resistor(&mut b, "r2", 4.5e3, &mid, &gnd);
        }
        Fixture::Diode => {
            devices::add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
            devices::add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
        }
        Fixture::DiodeSeries => {
            let low = b.node("low");
            devices::add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 470.0, &top, &mid);
            devices::add_diode(&mut b, "d1", 1.0e-14, &mid, &low);
            devices::add_resistor(&mut b, "r2", 22.0, &low, &gnd);
        }
        Fixture::Rc => {
            devices::add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
            devices::add_capacitor(&mut b, "c1", 1.0e-6, &mid, &gnd);
        }
        Fixture::SineRc => {
            b.device(
                devices::vsine(),
                "v1",
                &[("amp", ParamValue::Real(5.0)), ("freq", ParamValue::Real(1.0e3))],
                &[Port::Node(top.clone()), Port::Node(gnd.clone())],
            );
            devices::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
            devices::add_capacitor(&mut b, "c1", 1.0e-6, &mid, &gnd);
        }
    }
    (b.build().expect("build"), mid)
}

fn dc(circuit: &mut CircuitInstance) -> (Vec<f64>, usize) {
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    (by_index(circuit, result.values()), result.stats.newton_iterations)
}

#[test]
fn dc_agrees_on_both_contracts() {
    for (name, f) in [("divider", Fixture::Divider), ("diode", Fixture::Diode), ("diode with series resistance", Fixture::DiodeSeries)] {
        let (mut e, _) = element_circuit(&f);
        let (mut d, _) = device_circuit(&f);
        let (xe, xd) = (dc(&mut e).0, dc(&mut d).0);
        assert_eq!(xe.len(), xd.len(), "{name}: unknown count");
        for (i, (a, b)) in xe.iter().zip(&xd).enumerate() {
            assert_close(&format!("{name}, unknown {i}"), *a, *b);
        }
    }
}

/// Every accepted step of an RC transient: (time, unknowns by index).
fn tran(circuit: &mut CircuitInstance, mid: &AnalogReference, seed: bool, stop: f64, dt: f64) -> (Vec<(f64, Vec<f64>)>, usize) {
    let mut solver = circuit.transient(TransientAnalysisOptions::new(stop, dt), Context::default()).expect("tran");
    if seed {
        solver.apply_initial_conditions(vec![InitialValue { reference: mid.clone(), value: 0.0 }]);
    }
    let result = solver.solve().expect("solve");
    let newton = result.stats.newton_iterations;
    drop(solver);
    let steps = result.iter().map(|s| (s.time(), by_index(circuit, s.values()))).collect();
    (steps, newton)
}

#[test]
fn transient_takes_the_same_steps_on_both_contracts() {
    for (name, f, seed, stop, dt) in [
        ("RC step", Fixture::Rc, true, 1.0e-3, 1.0e-5),
        ("sine-driven RC", Fixture::SineRc, false, 2.0e-3, 2.0e-5),
    ] {
        let (mut e, me) = element_circuit(&f);
        let (mut d, md) = device_circuit(&f);
        let (se, _) = tran(&mut e, &me, seed, stop, dt);
        let (sd, _) = tran(&mut d, &md, seed, stop, dt);
        assert_eq!(se.len(), sd.len(), "{name}: accepted step count");
        for (k, ((te, xe), (td, xd))) in se.iter().zip(&sd).enumerate() {
            assert!((te - td).abs() <= 1.0e-15 * te.abs().max(td.abs()), "{name}, step {k}: time {te:e} vs {td:e}");
            for (i, (a, b)) in xe.iter().zip(xd).enumerate() {
                assert_close(&format!("{name}, step {k}, unknown {i}"), *a, *b);
            }
        }
    }
}

#[test]
fn newton_takes_the_same_iterations_on_both_contracts() {
    let (mut e, _) = element_circuit(&Fixture::Diode);
    let (mut d, _) = device_circuit(&Fixture::Diode);
    assert_eq!(dc(&mut e).1, dc(&mut d).1, "Newton iterations of the diode circuit");
}
