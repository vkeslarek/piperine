//! LRM-COVERS: A.7.4
//! Specify path delays reach the solver's `DelayTable` at circuit build
//! (T19, LRMW3-06) and are observable as real event timing in a transient
//! run — not merely read back off the table. `Leaf`'s `specify (a => y) =
//! (rise, fall);` block declares the delay; `Top` wires it in as an
//! ordinary instance (the pattern every other digital codegen test in
//! this crate already uses to get a buildable circuit — see
//! `checkpoint_digital.rs`).

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{CircuitInstance, PathDelay};

fn build(src: &str) -> (CircuitInstance, HashMap<String, usize>) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies: HashMap<String, LoweredBody> = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit compiles");
    (circuit, info.digital_nets)
}

const LEAF_WITH_DELAY: &str = r#"
discipline Bit;
    domain discrete;
enddiscipline
module Leaf (input Bit a, output Bit y);
    specify
        (a => y) = (2e-9, 3e-9);
    endspecify
    always @(a)
        y = a;
endmodule
module Top (input Bit a, output Bit y);
    Leaf u1 (a, y);
endmodule
"#;

#[test]
fn path_delay_populates_the_delay_table_keyed_by_destination_net() {
    let (circuit, nets) = build(LEAF_WITH_DELAY);
    let y = DigitalNet(nets["y"]);
    let recorded = circuit.digital_state.delay_table.get(y);
    assert_eq!(recorded, PathDelay { rise: 2e-9, fall: 3e-9, turn_off: 2e-9 });
}

/// The Done-when criterion proper: rise/fall delays are observable as
/// *event timing* in a real transient run, not merely a table read-back.
#[test]
fn rise_and_fall_delays_are_observable_as_event_timing() {
    let (mut circuit, nets) = build(LEAF_WITH_DELAY);
    let a = DigitalNet(nets["a"]);
    let y = DigitalNet(nets["y"]);

    // Settle the initial state (a = 0, y = 0) at t = 0.
    circuit.digital_state.schedule(DigitalEvent { time: 0.0, net: a, value: LogicValue::Zero, source: 999, seq: 0, region: Region::Active, strength: None });
    circuit
        .digital_state
        .evaluate_until_stable(0.0, &mut circuit.devices, Default::default(), &[])
        .expect("settles");
    // `y`'s very first transition (X -> 0) is delayed by the declared
    // fall delay (3ns) too — the specify path delay applies uniformly,
    // including the initial settle. Advance to drain it.
    circuit
        .digital_state
        .evaluate_until_stable(3e-9, &mut circuit.devices, Default::default(), &[])
        .expect("settles the delayed initial edge");
    assert_eq!(circuit.digital_state.nets[y.0], LogicValue::Zero);

    // a: 0 -> 1 at t = 10ns. The comb block reacts immediately (its own
    // intrinsic delay is 0), so without the specify path delay `y` would
    // change at the same instant; with it, `y` must NOT change until
    // 10ns + 2ns (the declared rise delay).
    circuit.digital_state.schedule(DigitalEvent { time: 10e-9, net: a, value: LogicValue::One, source: 999, seq: 1, region: Region::Active, strength: None });
    circuit
        .digital_state
        .evaluate_until_stable(10e-9, &mut circuit.devices, Default::default(), &[])
        .expect("settles at the input edge");
    assert_eq!(circuit.digital_state.nets[y.0], LogicValue::Zero, "y must not move before its declared rise delay elapses");

    circuit
        .digital_state
        .evaluate_until_stable(10e-9 + 2e-9, &mut circuit.devices, Default::default(), &[])
        .expect("settles at the delayed output edge");
    assert_eq!(circuit.digital_state.nets[y.0], LogicValue::One, "y follows a's rise after exactly the declared 2ns rise delay");

    // a: 1 -> 0 at t = 20ns; y must follow only after the 3ns fall delay.
    circuit.digital_state.schedule(DigitalEvent { time: 20e-9, net: a, value: LogicValue::Zero, source: 999, seq: 2, region: Region::Active, strength: None });
    circuit
        .digital_state
        .evaluate_until_stable(20e-9, &mut circuit.devices, Default::default(), &[])
        .expect("settles at the second input edge");
    assert_eq!(circuit.digital_state.nets[y.0], LogicValue::One, "y must not move before its declared fall delay elapses");

    circuit
        .digital_state
        .evaluate_until_stable(20e-9 + 3e-9, &mut circuit.devices, Default::default(), &[])
        .expect("settles at the delayed fall edge");
    assert_eq!(circuit.digital_state.nets[y.0], LogicValue::Zero, "y follows a's fall after exactly the declared 3ns fall delay");
}

/// **Disclosed deviation pinning test** (design.md C5): `DelayTable` is
/// keyed per destination net, not per source→destination path — two
/// distinct paths that both target the same net collapse onto whichever
/// was written last (authored order), not an error and not a merge.
#[test]
fn two_paths_into_one_net_collapse_onto_the_last_written_record() {
    let src = r#"
discipline Bit;
    domain discrete;
enddiscipline
module Leaf (input Bit a, input Bit b, output Bit y);
    specify
        (a => y) = (1e-9, 1e-9);
        (b => y) = (5e-9, 5e-9);
    endspecify
    always @(a or b)
        y = a | b;
endmodule
module Top (input Bit a, input Bit b, output Bit y);
    Leaf u1 (a, b, y);
endmodule
"#;
    let (circuit, nets) = build(src);
    let y = DigitalNet(nets["y"]);
    let recorded = circuit.digital_state.delay_table.get(y);
    assert_eq!(
        recorded,
        PathDelay { rise: 5e-9, fall: 5e-9, turn_off: 5e-9 },
        "the second authored path (b => y) collapses onto the first (a => y)'s record — SPEC_DEVIATION design.md C5"
    );
}

/// A `` `timescale `` directive quantizes the recorded delay to its
/// precision grid (LRM §8.4.3.3, the same rule `DigitalKernel::
/// quantize_time` applies to runtime event scheduling).
#[test]
fn path_delay_quantizes_to_the_module_timescale_precision() {
    let src = r#"
`timescale 1ns/1ns
discipline Bit;
    domain discrete;
enddiscipline
module Leaf (input Bit a, output Bit y);
    specify
        (a => y) = (2.4e-9, 2.6e-9);
    endspecify
    always @(a)
        y = a;
endmodule
module Top (input Bit a, output Bit y);
    Leaf u1 (a, y);
endmodule
"#;
    let (circuit, nets) = build(src);
    let y = DigitalNet(nets["y"]);
    let recorded = circuit.digital_state.delay_table.get(y);
    // 1ns precision: 2.4ns rounds to 2ns, 2.6ns rounds to 3ns (epsilon
    // comparison: the round-trip through `/grid`/`*grid` is not bit-exact,
    // the same floating-point behavior `DigitalKernel::quantize_time`'s
    // own tests already accept).
    assert!((recorded.rise - 2e-9).abs() < 1e-15, "rise = {}", recorded.rise);
    assert!((recorded.fall - 3e-9).abs() < 1e-15, "fall = {}", recorded.fall);
}
