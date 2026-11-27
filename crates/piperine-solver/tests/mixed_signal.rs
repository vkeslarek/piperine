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

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use abi_devices::{add_resistor, add_vsrc, node_id};
use abi_mixed::{clock_src, comparator, reactions, toggle_dff};
use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, TransientAnalysisOptions};

/// `ClockSrc::at` of `tests/helpers/devices.rs`.
fn at(period: f64, t: f64) -> f64 {
    let ph = t - period * (t / period).floor();
    if ph > period * 0.25 && ph < period * 0.75 { 1.0 } else { 0.0 }
}

#[test]
fn clock_src_reads_its_waveform() {
    let period = 1.0e-3;
    let mut b = CircuitBuilder::new("clock");
    let gnd = b.ground();
    let clk = b.node("clk");
    b.device(clock_src(), "ck", &[("period", ParamValue::Real(period))], &[Port::Node(clk.clone()), Port::Node(gnd.clone())]);
    add_resistor(&mut b, "rl", 1.0e3, &clk, &gnd);
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
        let v = s.get_node(&node_id(&clk)).unwrap();
        if edge(s.time()) {
            // The landing on an edge is solved on the level before it.
            falling += usize::from(v == 1.0);
            continue;
        }
        assert_eq!(v, at(period, s.time()), "v(clk) at t = {:e}", s.time());
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
    b.device(clock_src(), "ck", &[("period", ParamValue::Real(period))], &[Port::Node(clk.clone()), Port::Node(gnd.clone())]);
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(clk), Port::Node(r), Port::Net("c".into())]);
    b.device(toggle_dff(), "ff", &[], &[Port::Net("c".into()), Port::Net("q".into())]);
    let mut circuit = b.build().expect("build");
    circuit
        .transient(TransientAnalysisOptions::new(2.9e-3, 1.0e-5), Context::default())
        .expect("tran")
        .solve()
        .expect("solve");
    // Rising clock edges at 0.25, 1.25, 2.25 ms: the comparator sees each at the
    // step after it, and q toggles three times.
    assert_eq!(reactions("toggle"), 3);
    let bound = circuit.bound();
    assert_eq!(bound.digital.read(bound.names.nets["net:q"]), HIGH);
    assert_eq!(bound.digital.read(bound.names.nets["net:c"]), LOW, "the clock is low at 2.9 ms");
}
