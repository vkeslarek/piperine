//! Scope: the bounds a session can set (host-ii HII-02 AC 6 and 7): the rounds of an operating
//! point (`Context::max_rounds`, default 20) and the microsteps of an instant (`CircuitBuilder::
//! microsteps`, default 1000). Each bound is observed by the failure it causes when it is
//! reached, and its default by the value the failure names.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::digital;
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context};

#[test]
fn max_rounds_defaults_to_twenty() {
    // HII-02 AC 1: no option leaves the bound where it was, 20 rounds. A loop that never
    // settles fails naming the bound it reached; with a default context that is 20.
    assert_eq!(Context::default().max_rounds, 20);
    let (b, _) = BoundedLoops::mixed_loop("cmp");
    let mut circuit = b.build().expect("build");
    let error = circuit.dc(Context::default()).expect("dc").solve().expect_err("an oscillator does not settle");
    assert!(format!("{error}").contains("20 rounds"), "{error}");
}

#[test]
fn an_operating_point_past_max_rounds_fails_naming_the_bound() {
    // HII-02 AC 6: the loop that settles in three rounds, run with `max_rounds = 1`, fails with
    // `E0506` naming 1; with the default it settles (the control), so the bound is what failed it.
    let (b, _) = BoundedLoops::mixed_loop("q");
    let mut circuit = b.build().expect("build");
    let context = Context { max_rounds: 1, ..Context::default() };
    let error = circuit.dc(context).expect("dc").solve().expect_err("one round is not enough");
    let text = format!("{error}");
    assert!(text.contains("E0506"), "{text}");
    assert!(text.contains("1 rounds"), "{text}");

    let (b, a2) = BoundedLoops::mixed_loop("q");
    let mut circuit = b.build().expect("build");
    let op = circuit.dc(Context::default()).expect("dc").solve().expect("the default bound lets it settle");
    assert!(op.get_node(&CircuitBuilding::node_id(&a2)).unwrap().abs() < 1e-12);
}

#[test]
fn microsteps_default_to_a_thousand() {
    // HII-02 AC 1: with no `microsteps` call the engine keeps its 1000 microsteps, and an instant
    // that never goes quiet fails naming that bound.
    let mut circuit = BoundedLoops::zero_delay_clock(None).build().expect("build");
    let error = circuit.dc(Context::default()).expect("dc").solve().expect_err("the instant never goes quiet");
    let text = format!("{error}");
    assert!(text.contains("E0501"), "{text}");
    assert!(text.contains("1000 microsteps"), "{text}");
}

#[test]
fn an_instant_past_the_microstep_bound_fails_naming_it() {
    // HII-02 AC 7: a zero-delay toggle bound at 5 microsteps fails naming 5, not the default.
    let mut circuit = BoundedLoops::zero_delay_clock(Some(5)).build().expect("build");
    let error = circuit.dc(Context::default()).expect("dc").solve().expect_err("5 microsteps are not enough");
    let text = format!("{error}");
    assert!(text.contains("E0501"), "{text}");
    assert!(text.contains("5 microsteps"), "{text}");
    assert!(!text.contains("1000"), "{text}");
}

/// The mixed loop and the zero-delay clock the session bounds tests run.
struct BoundedLoops;

impl BoundedLoops {
    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// A DAC, a comparator, a flip-flop and an inverter in a loop: `code_from` is the net the
    /// inverter reads. From `q` it settles after three rounds; from `cmp` it never settles.
    fn mixed_loop(code_from: &str) -> (CircuitBuilder, AnalogReference) {
        let mut b = CircuitBuilder::new("mixed loop");
        let gnd = b.ground();
        let a = b.node("a");
        let r = b.node("r");
        CircuitBuilding::add_voltage_source(&mut b, "vr", 0.5, &r, &gnd);
        b.device(mixed::BitDigitalToAnalog::module(), "dac", &[("vref", ParamValue::Real(1.0))], &[BoundedLoops::net("code"), Port::Node(a.clone())]);
        b.device(mixed::Comparator::module(), "cmp", &[], &[Port::Node(a.clone()), Port::Node(r), BoundedLoops::net("cmp")]);
        b.device(digital::DataFlipFlop::module(), "ff", &[], &[BoundedLoops::net("cmp"), BoundedLoops::net("code"), BoundedLoops::net("q")]);
        b.device(digital::Inverter::module(), "inv", &[], &[BoundedLoops::net(code_from), BoundedLoops::net("code")]);
        b.device(mixed::SettleCount::module(), "settle", &[], &[]);
        (b, a)
    }

    /// A clock with a half period of zero: it toggles its own trigger at every microstep, so its
    /// instant never goes quiet (`E0501`).
    fn zero_delay_clock(bound: Option<usize>) -> CircuitBuilder {
        let mut b = CircuitBuilder::new("zero delay");
        b.device(digital::Timer::module(), "t", &[("half", ParamValue::Real(0.0))], &[BoundedLoops::net("clk")]);
        if let Some(bound) = bound {
            b.microsteps(bound);
        }
        b
    }
}
