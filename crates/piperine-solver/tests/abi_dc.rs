//! The DC driver on devices (SAI-11): the circuits of `helper_devices.rs`
//! built from `Device`s read the same analytic values with the same
//! tolerances, and the homotopy's source scale reaches a source through the
//! `SOURCE_SCALE` ambient input.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{add_diode, add_resistor, add_vsrc, node_id};
use piperine_analog_solver::convergence::HomotopyDriver;
use piperine_solver::prelude::{AnalogReference, CircuitBuilder, CircuitInstance, Context};

fn dc_node(circuit: &mut CircuitInstance, node: &AnalogReference) -> f64 {
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    result.get_node(&node_id(node)).expect("node solved")
}

fn divider(vin: f64, r1: f64, r2: f64) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", vin, &top, &gnd);
    add_resistor(&mut b, "r1", r1, &top, &mid);
    add_resistor(&mut b, "r2", r2, &mid, &gnd);
    (b.build().expect("build"), mid)
}

#[test]
fn divider_reads_the_analytic_ratio_on_devices() {
    let (vin, r1, r2) = (7.0, 1.5e3, 4.5e3);
    let (mut circuit, mid) = divider(vin, r1, r2);

    let v = dc_node(&mut circuit, &mid);
    let analytic = vin * r2 / (r1 + r2);
    assert!((v - analytic).abs() < 1.0e-9, "v(mid) = {v} vs analytic {analytic}");
}

#[test]
fn diode_satisfies_its_own_equation_on_devices() {
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

#[test]
fn source_stepping_scales_sources_through_the_ambient_input() {
    let (vin, r1, r2) = (8.0, 1.0e3, 3.0e3);
    let (mut circuit, mid) = divider(vin, r1, r2);
    let at = mid.idx().expect("mid is solved");
    let full = vin * r2 / (r1 + r2);

    let mut dc = circuit.dc(Context::default()).expect("dc");
    dc.set_src_scale(0.25);
    let scaled = dc.newton().expect("a solve at a quarter of every source");
    assert!((scaled[at] - 0.25 * full).abs() < 1.0e-9, "v(mid) = {} at scale 0.25", scaled[at]);

    // At rest the source is whole again: the scale is an input of each solve, not a state.
    dc.set_src_scale(1.0);
    let whole = dc.newton().expect("a solve at rest");
    assert!((whole[at] - full).abs() < 1.0e-9, "v(mid) = {} at scale 1", whole[at]);
}
