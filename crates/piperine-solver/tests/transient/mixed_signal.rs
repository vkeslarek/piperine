//! The analog/digital boundary on devices: the mixed-signal sources of
//! `tests/helpers/devices.rs` as `Device`s (`helpers/abi_mixed.rs`). A
//! `ClockSrc` drives its waveform from a timer reaction and a discrete phase
//! pin; a comparator and a toggle flip-flop divide it.
//!
//! LRM-COVERS: 8.4, 8.4.3, 8.4.3.2, 8.4.5
//! §8.4.3 ("Mixed-signal transient analysis"), §8.4.3.2 and §8.4.5: analog and
//! digital processes communicating through events, an analog solution driving
//! a comparator's event and a discrete value reconfiguring an analog function —
//! `clock_src_reads_its_waveform` and `a_comparator_and_toggle_divide_the_clock`
//! here, with `operating_point.rs`, `protocol_real.rs` and `protocol_crossings.rs`.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, TransientAnalysisOptions};

#[test]
fn clock_src_reads_its_waveform() {
    let period = 1.0e-3;
    let mut b = CircuitBuilder::new("clock");
    let gnd = b.ground();
    let clk = b.node("clk");
    b.device(mixed::ClockSource::module(), "ck", &[("period", ParamValue::Real(period))], &[Port::Node(clk.clone()), Port::Node(gnd.clone())]);
    CircuitBuilding::add_resistor(&mut b, "rl", 1.0e3, &clk, &gnd);
    let mut circuit = b.build().expect("build");
    let result = circuit
        .transient(TransientAnalysisOptions::new(3.0e-3, 1.0e-5), Context::default())
        .expect("tran")
        .solve()
        .expect("solve");
    let edge = |t: f64| {
        let ph = t / period - (t / period).floor();
        (ph - 0.75).abs() < 1.0e-9 || (ph - 0.25).abs() < 1.0e-9
    };
    let mut falling = 0;
    for s in result.iter() {
        let v = s.get_node(&CircuitBuilding::node_id(&clk)).unwrap();
        if edge(s.time()) {
            // The landing on an edge is solved on the level before it.
            falling += usize::from(v == 1.0);
            continue;
        }
        assert_eq!(v, MixedSignalRead::at(period, s.time()), "v(clk) at t = {:e}", s.time());
    }
    assert_eq!(falling, 3, "each falling edge is a landing, read on the level before it");
}

#[test]
fn a_comparator_and_toggle_divide_the_clock() {
    let period = 1.0e-3;
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let clk = b.node("clk");
    let r = b.node("r");
    b.device(mixed::ClockSource::module(), "ck", &[("period", ParamValue::Real(period))], &[Port::Node(clk.clone()), Port::Node(gnd.clone())]);
    CircuitBuilding::add_voltage_source(&mut b, "vr", 0.5, &r, &gnd);
    b.device(mixed::Comparator::module(), "cmp", &[], &[Port::Node(clk), Port::Node(r), Port::Net("c".into())]);
    b.device(mixed::ToggleFlipFlop::module(), "ff", &[], &[Port::Net("c".into()), Port::Net("q".into())]);
    let mut circuit = b.build().expect("build");
    circuit
        .transient(TransientAnalysisOptions::new(2.9e-3, 1.0e-5), Context::default())
        .expect("tran")
        .solve()
        .expect("solve");
    // Rising clock edges at 0.25, 1.25, 2.25 ms: the comparator sees each at the
    // step after it, and q toggles three times.
    assert_eq!(mixed::ReactionCounts::of("toggle"), 3);
    let bound = circuit.bound();
    assert_eq!(bound.digital.read(bound.names.nets["net:q"]), HIGH);
    assert_eq!(bound.digital.read(bound.names.nets["net:c"]), LOW, "the clock is low at 2.9 ms");
}

/// A mixed-signal value read at a time.
struct MixedSignalRead;

impl MixedSignalRead {
    /// `ClockSrc::at` of `tests/helpers/devices.rs`.
    fn at(period: f64, t: f64) -> f64 {
        let ph = t - period * (t / period).floor();
        if ph > period * 0.25 && ph < period * 0.75 { 1.0 } else { 0.0 }
    }
}
