//! The DC driver on devices (SAI-11): the circuits of `helper_devices.rs`
//! built from `Device`s read the same analytic values with the same
//! tolerances, and the homotopy's continuation values reach a device through
//! the ambient inputs: the source scale through `SOURCE_SCALE`, the gmin step
//! through `GMIN`.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::result::Result;
use std::sync::{Arc, OnceLock};

use abi_devices::{Counting, add_diode, add_resistor, add_vsrc, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_analog_solver::convergence::{ConvergencePlan, HomotopyDriver};
use piperine_solver::abi::Port;
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

// ---------------------------------------------------------------------------
// The gmin-stepping continuation reaches a device through the `GMIN` input.
// ---------------------------------------------------------------------------

/// Drives the ambient `GMIN` (in amperes per siemens) into its node.
/// pins: a=0 (PORT); terms: T0 FLOW{a, GROUND} "i" = -GMIN
/// fn 0: reads [GMIN] writes [T0] (no partial: the term reads no unknown)
struct GminProbe;
static GMIN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GMIN_PROBE: OnceLock<Module> = OnceLock::new();

fn gmin_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: "abi_dc.rs", op: op::FLOW, plus: a, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        origin: "abi_dc.rs",
        reads: vec![Cell { kind: cell::AMBIENT, index: ambient::GMIN, part: part::VALUE }],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for GminProbe {
    fn prototype() -> &'static Shape<'static> {
        GMIN_SHAPE.get_or_init(gmin_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((GminProbe, Arc::clone(GMIN_SHAPE.get_or_init(gmin_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let gmin = cx.f64(0);
        cx.set(0, -gmin);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn gmin_stepping_reaches_a_device_through_the_ambient_input() {
    // The probe drives GMIN amperes into a 1 Ω load; the driver's own shunt of
    // the step (`g` on every node) parallels it: v = (gmin + g) / (1 + g).
    let mut b = CircuitBuilder::new("gmin probe");
    let gnd = b.ground();
    let a = b.node("a");
    b.device(GMIN_PROBE.get_or_init(|| module::<GminProbe>("GminProbe")), "probe", &[], &[Port::Node(a.clone())]);
    add_resistor(&mut b, "r1", 1.0, &a, &gnd);
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

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("GminProbe", GMIN_PROBE.get_or_init(|| module::<GminProbe>("GminProbe")));
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
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
    add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
    let mut circuit = b.build().expect("build");
    let plan = ConvergencePlan::default().with_newton(Box::new(Counting(Arc::clone(&calls))));
    let result = circuit.dc(Context::default()).expect("dc").with_plan(plan).solve().expect("solve");
    let seen = calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(result.stats.newton_iterations > 1, "a diode takes several iterations");
    assert_eq!(seen, result.stats.newton_iterations as usize, "one damp_update per iteration of the plan's strategy");
}
