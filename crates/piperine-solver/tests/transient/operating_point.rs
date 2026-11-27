//! The operating point of a circuit of devices through the protocol (SAI-06,
//! SAI-09; abi.md §10): a comparator, a flip-flop and a DAC closed in a loop
//! settle at the consistent point with `SETTLE` fired once; the same loop
//! wired as an oscillator fails with `E0506` naming its nets; the session
//! globals hold the session's values inside a device.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{AnalogReference, Bound, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context};

#[test]
fn a_mixed_loop_settles_at_its_operating_point() {
    // Round 1 solves on the code bind seeded (no value: 0 V); the first instant
    // inverts q (LOW) to code HIGH. Round 2: `a` = 1 V crosses the reference, the
    // comparator goes HIGH, its edge clocks code (HIGH) into q, and the inverter
    // makes code LOW. Round 3: `a` = 0 V crosses back, the comparator goes LOW
    // (a falling edge: q holds). Nothing an analog function reads changes: settled.
    let (b, a, r) = OperatingPointCases::mixed_loop("q");
    let mut circuit = b.build().expect("build");
    let op = circuit.dc(Context::default()).expect("dc").solve().expect("the loop settles");

    assert!(op.get_node(&CircuitBuilding::node_id(&a)).unwrap().abs() < 1e-12, "v(a) = {:?}", op.get_node(&CircuitBuilding::node_id(&a)));
    assert!((op.get_node(&CircuitBuilding::node_id(&r)).unwrap() - 0.5).abs() < 1e-12);
    let bound = circuit.bound();
    assert_eq!(OperatingPointCases::bytes_of(bound, "q"), HIGH.to_vec(), "q");
    assert_eq!(OperatingPointCases::bytes_of(bound, "code"), LOW.to_vec(), "code = not q");
    assert_eq!(OperatingPointCases::bytes_of(bound, "cmp"), LOW.to_vec(), "v(a) < v(r)");
    assert_eq!(mixed::ReactionCounts::of("comparator"), 2, "up in round 2, down in round 3");
    assert_eq!(mixed::ReactionCounts::of("dff"), 1, "one rising edge");
    assert_eq!(mixed::ReactionCounts::of("settle"), 1, "SETTLE fires once");
}

#[test]
fn operating_point_rounds_beyond_the_bound_are_e0506() {
    // The inverter reads the comparator instead of the flip-flop: every round
    // flips the code and `a` crosses the reference again.
    let (b, _, _) = OperatingPointCases::mixed_loop("cmp");
    let mut circuit = b.build().expect("build");
    let err = circuit.dc(Context::default()).expect("dc").solve().expect_err("an oscillator does not settle");
    let text = format!("{err}");
    assert!(text.contains("E0506"), "{text}");
    assert!(text.contains("20 rounds"), "{text}");
    assert!(text.contains("code"), "the net still changing is named: {text}");
    assert_eq!(mixed::ReactionCounts::of("settle"), 0, "no SETTLE without an operating point");
}

#[test]
fn globals_hold_the_session_values() {
    let mut b = CircuitBuilder::new("globals");
    let gnd = b.ground();
    let n = b.node("n");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &n, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 1e3, &n, &gnd);
    b.resolution(1e-12);
    b.session_param("vdd", ParamValue::Real(1.8));
    b.device(mixed::Globals::module(), "probe", &[], &[]);
    let mut circuit = b.build().expect("build");
    circuit.dc(Context::default()).expect("dc").solve().expect("dc");

    let bound = circuit.bound();
    let read = |name: &str| -> f64 {
        let bytes = bound.analog.var("probe", name).unwrap_or_else(|| panic!("VAR {name}"));
        f64::from_le_bytes(bytes[..8].try_into().unwrap())
    };
    let temperature = piperine_solver::abi::Tolerances::default().temperature;
    assert_eq!(read("temp"), temperature, "TEMPERATURE");
    assert_eq!(read("res"), 1e-12, "RESOLUTION");
    assert_eq!(read("dc"), 1.0, "DC holds HIGH in an operating point");
    assert_eq!(read("vdd"), 1.8, "the NAMED global holds the session parameter");
}

/// The helpers of the operating_point tests.
pub struct OperatingPointCases;

impl OperatingPointCases {
    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The bytes of helper `bytes_of`.
    fn bytes_of(bound: &Bound, name: &str) -> Vec<u8> {
        let id = bound.names.nets[&format!("net:{name}")];
        bound.digital.read(id).to_vec()
    }

    /// The DAC drives `a` from `code`, the comparator compares `a` with a 0.5 V
    /// reference `r` and drives `cmp`; `code` comes from `code_from` through the
    /// inverter. The flip-flop is clocked by `cmp` and samples `code` into `q`.
    fn mixed_loop(code_from: &str) -> (CircuitBuilder, AnalogReference, AnalogReference) {
        let mut b = CircuitBuilder::new("mixed loop");
        let gnd = b.ground();
        let a = b.node("a");
        let r = b.node("r");
        CircuitBuilding::add_voltage_source(&mut b, "vr", 0.5, &r, &gnd);
        b.device(mixed::DigitalToAnalog::module(), "dac", &[("vref", ParamValue::Real(1.0))], &[OperatingPointCases::net("code"), Port::Node(a.clone())]);
        b.device(mixed::Comparator::module(), "cmp", &[], &[Port::Node(a.clone()), Port::Node(r.clone()), OperatingPointCases::net("cmp")]);
        b.device(mixed::DataFlipFlop::module(), "ff", &[], &[OperatingPointCases::net("cmp"), OperatingPointCases::net("code"), OperatingPointCases::net("q")]);
        b.device(mixed::Inverter::module(), "inv", &[], &[OperatingPointCases::net(code_from), OperatingPointCases::net("code")]);
        b.device(mixed::SettleCount::module(), "settle", &[], &[]);
        (b, a, r)
    }
}
