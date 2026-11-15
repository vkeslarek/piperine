//! The device library of `tests/helpers/abi_devices.rs` against
//! analytic values: a resistor divider, an RC step, a diode fed through a
//! resistor. (The partials of every
//! function are verified in `abi_devices.rs`.) This is the proof that the library is a faithful
//! stand-in before the five formerly codegen-driven targets are ported to it.

// Each target includes the library by path: it uses a different subset of it.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{add_capacitor, add_diode, add_resistor, add_vsrc, node_id};
use piperine_solver::abi::InitialValue;
use piperine_solver::prelude::{
    AnalogReference, CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions,
};

fn dc_node(circuit: &mut CircuitInstance, node: &AnalogReference) -> f64 {
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    result.get_node(&node_id(node)).expect("node solved")
}

#[test]
fn divider_reads_the_analytic_ratio() {
    let (vin, r1, r2) = (7.0, 1.5e3, 4.5e3);
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", vin, &top, &gnd);
    add_resistor(&mut b, "r1", r1, &top, &mid);
    add_resistor(&mut b, "r2", r2, &mid, &gnd);
    let mut circuit = b.build().expect("build");

    let v = dc_node(&mut circuit, &mid);
    let analytic = vin * r2 / (r1 + r2);
    assert!((v - analytic).abs() < 1.0e-9, "v(mid) = {v} vs analytic {analytic}");
}

#[test]
fn rc_step_reads_one_minus_exp() {
    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c;
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", r, &top, &out);
    add_capacitor(&mut b, "c1", c, &out, &gnd);
    let mut circuit = b.build().expect("build");

    let mut solver = circuit
        .transient(TransientAnalysisOptions::new(tau, tau / 100.0), Context::default())
        .expect("tran");
    solver.apply_initial_conditions(vec![InitialValue { reference: out.clone(), value: 0.0 }]);
    let result = solver.solve().expect("solve");
    let last = result.last().expect("steps");

    let v = last.get_node(&node_id(&out)).expect("v(out)");
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((last.time() - tau).abs() < 1.0e-12, "stopped at t = {}", last.time());
    assert!((v - analytic).abs() < 1.0e-3, "v(rc) = {v} vs 1 - 1/e = {analytic}");
}

#[test]
fn diode_satisfies_its_own_equation() {
    let (vin, r, is_sat, vt) = (1.0, 1.0e3, 1.0e-14, 0.02585);
    let mut b = CircuitBuilder::new("diode");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", vin, &top, &gnd);
    add_resistor(&mut b, "r1", r, &top, &mid);
    add_diode(&mut b, "d1", is_sat, &mid, &gnd);
    let mut circuit = b.build().expect("build");

    let vd = dc_node(&mut circuit, &mid);
    let i_resistor = (vin - vd) / r;
    let i_diode = is_sat * ((vd / vt).exp() - 1.0);
    assert!(
        (i_resistor - i_diode).abs() < 1.0e-9,
        "i(r) = {i_resistor} A vs i(d) = {i_diode} A at vd = {vd}"
    );
}
