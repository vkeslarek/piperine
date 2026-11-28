//! The DC driver on devices (SAI-11): the circuits of `helper_devices.rs`
//! built from `Device`s read the same analytic values with the same
//! tolerances, and the homotopy's continuation values reach a device through
//! the ambient inputs: the source scale through `SOURCE_SCALE`, the gmin step
//! through `GMIN`.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::solver_devices::abi_dc::{GminProbe};
use piperine_device_fixtures::circuits::Counting;
use std::sync::Arc;

use piperine_analog_solver::convergence::{ConvergencePlan, HomotopyDriver};
use piperine_solver::abi::Port;
use piperine_solver::prelude::{AnalogReference, CircuitBuilder, CircuitInstance, Context};

#[test]
fn divider_reads_the_analytic_ratio_on_devices() {
    let (vin, r1, r2) = (7.0, 1.5e3, 4.5e3);
    let (mut circuit, mid) = DividerOperatingPoint::divider(vin, r1, r2);

    let v = DividerOperatingPoint::dc_node(&mut circuit, &mid);
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
    CircuitBuilding::add_voltage_source(&mut b, "v1", vin, &top, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", r, &top, &mid);
    CircuitBuilding::add_diode(&mut b, "d1", is_sat, &mid, &gnd);
    let mut circuit = b.build().expect("build");

    let vd = DividerOperatingPoint::dc_node(&mut circuit, &mid);
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
    let (mut circuit, mid) = DividerOperatingPoint::divider(vin, r1, r2);
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

#[test]
fn gmin_stepping_reaches_a_device_through_the_ambient_input() {
    // The probe drives GMIN amperes into a 1 Ω load; the driver's own shunt of
    // the step (`g` on every node) parallels it: v = (gmin + g) / (1 + g).
    let mut b = CircuitBuilder::new("gmin probe");
    let gnd = b.ground();
    let a = b.node("a");
    b.device(GminProbe::module(), "probe", &[], &[Port::Node(a.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0, &a, &gnd);
    let mut circuit = b.build().expect("build");
    let at = a.idx().expect("a is solved");
    let gmin = Context::default().tolerances.gmin;

    let mut dc = circuit.dc(Context::default()).expect("dc");
    dc.set_gmin_extra(0.5);
    let stepped = dc.newton().expect("a solve on a gmin step");
    let expected = (gmin + 0.5) / 1.5;
    assert!((stepped[at] - expected).abs() < 1.0e-12, "v(a) = {} on the 0.5 S step, expected {expected}", stepped[at]);

    // At rest the input is the session's gmin again.
    dc.set_gmin_extra(0.0);
    let rest = dc.newton().expect("a solve at rest");
    assert!((rest[at] - gmin).abs() < 1.0e-15, "v(a) = {} at rest, expected {gmin}", rest[at]);
}

#[test]
fn the_dc_driver_iterates_with_the_plans_strategy() {
    // Newton of the first equilibrium (a diode needs several iterations) goes through the
    // strategy of the plan the driver was given: it is called once per iteration.
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut b = CircuitBuilder::new("diode");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
    CircuitBuilding::add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
    let mut circuit = b.build().expect("build");
    let plan = ConvergencePlan::default().with_newton(Box::new(Counting(Arc::clone(&calls))));
    let result = circuit.dc(Context::default()).expect("dc").with_plan(plan).solve().expect("solve");
    let seen = calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(result.stats.newton_iterations > 1, "a diode takes several iterations");
    assert_eq!(seen, result.stats.newton_iterations as usize, "one damp_update per iteration of the plan's strategy");
}

/// The divider the ABI operating-point tests solve, and the voltage at one of its nodes.
struct DividerOperatingPoint;

impl DividerOperatingPoint {
    /// The dc node helper `dc_node`.
    fn dc_node(circuit: &mut CircuitInstance, node: &AnalogReference) -> f64 {
        let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
        result.get_node(&CircuitBuilding::node_id(node)).expect("node solved")
    }

    /// The divider helper `divider`.
    fn divider(vin: f64, r1: f64, r2: f64) -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("divider");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", vin, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", r1, &top, &mid);
        CircuitBuilding::add_resistor(&mut b, "r2", r2, &mid, &gnd);
        (b.build().expect("build"), mid)
    }
}
