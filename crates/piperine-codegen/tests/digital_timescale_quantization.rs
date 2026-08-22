//! Digital event times quantize to the module's `` `timescale `` precision
//! grid (LRM §8.4.3.3, T26/LRMW2-26) — a module declaring
//! `` `timescale 1ns/1ps `` rounds every scheduled digital event to the
//! nearest picosecond; a module with no `` `timescale `` directive at all
//! quantizes nothing (existing behavior unchanged).
//!
//! LRM-COVERS: 8.4.3.3

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::DigitalKernel;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::DigitalEvent;
use piperine_solver::prelude::{DigitalNet, LogicValue};

/// A trivial clocked register — enough digital structure for
/// `DigitalKernel` to compile and schedule an output-change event on a
/// clock edge (mirrors `checkpoint_digital.rs`'s own
/// `always @(posedge clk) Q = D;` fixture shape, proven to compile).
const REGISTER: &str = "
discipline Bit;
    domain discrete;
enddiscipline
module reg1 (input Bit clk, input Bit d, output Bit q);
    always @(posedge clk)
        q = d;
endmodule
";

fn with_timescale(ts: &str) -> String {
    format!("{ts}\n{REGISTER}")
}

fn compile(src: &str) -> std::sync::Arc<DigitalKernel> {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies: std::collections::HashMap<String, LoweredBody> =
        piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    std::sync::Arc::new(DigitalKernel::compile(&bodies["reg1"]).expect("compiles"))
}

/// Direct, exact proof (no solver round-trip involved): `quantize_time`
/// rounds a non-grid time to the nearest picosecond when the module
/// declares `` `timescale 1ns/1ps ``.
#[test]
fn quantize_time_rounds_to_the_declared_precision_grid() {
    let kernel = compile(&with_timescale("`timescale 1ns/1ps"));
    // 2.3456 ns is not a whole number of picoseconds; the nearest ps grid
    // point is 2.346 ns (2346 ps), not 2.3456 ns or 2.345 ns.
    let t = 2.3456e-9;
    let expected = 2.346e-9;
    let got = kernel.quantize_time(t);
    assert!(
        (got - expected).abs() < 1e-15,
        "quantize_time({t}) = {got}, want {expected} (nearest 1 ps)"
    );
}

/// Regression: a module with no `` `timescale `` directive quantizes
/// nothing — existing (pre-T26) transient waveforms must not move.
#[test]
fn quantize_time_is_a_no_op_without_a_timescale_directive() {
    let kernel = compile(REGISTER);
    let t = 2.34567891011e-9;
    assert_eq!(kernel.quantize_time(t), t, "no `timescale` directive means no quantization");
}

/// Drives the register through a rising clock edge at a deliberately
/// non-grid time and returns the resulting `q`-change event.
fn clock_edge_at(kernel: std::sync::Arc<DigitalKernel>, t: f64) -> DigitalEvent {
    let (clk, d, q) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance = DigitalInstance::new(kernel, 0, vec![clk, d], vec![q], vec![]).expect("instance");

    let mut queue: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();
    instance.init(&mut queue);
    queue.clear();

    // Seed the pre-edge state: clk low, d high (so the coming rising edge
    // captures a real 0 -> 1 change on q).
    instance.eval(0.0, &[LogicValue::Zero, LogicValue::One, LogicValue::X], &[], &mut queue);
    queue.clear();

    // The rising edge itself, at the non-grid time under test.
    instance.eval(t, &[LogicValue::One, LogicValue::One, LogicValue::Zero], &[], &mut queue);

    let Reverse(ev) = queue.pop().expect("q captures d on the rising edge");
    assert_eq!(ev.net, q);
    assert_eq!(ev.value, LogicValue::One);
    ev
}

/// Spec AC: an event scheduled at a non-grid time lands on the grid —
/// asserted at the actual recorded `DigitalEvent::time`, not just the
/// standalone `quantize_time` helper. Drives `DigitalInstance::eval`
/// directly (bypassing the solver's `EventSink`-delay round-trip, which
/// would reintroduce floating-point noise unrelated to this fix) with a
/// deliberately non-grid clock-edge time.
#[test]
fn an_event_scheduled_at_a_non_grid_time_lands_on_the_grid() {
    let kernel = compile(&with_timescale("`timescale 1ns/1ps"));
    let non_grid_t = 5.0000001e-9; // not a whole number of picoseconds
    let ev = clock_edge_at(kernel, non_grid_t);

    let expected = 5.0e-9; // nearest 1 ps grid point to 5.0000001 ns
    assert!(
        (ev.time - expected).abs() < 1e-15,
        "event scheduled at {non_grid_t} must land on the grid ({expected}), got {}",
        ev.time
    );
}

/// Regression counterpart: the identical off-grid clock-edge time, on a
/// module with no `` `timescale `` directive, reaches the event queue
/// unchanged.
#[test]
fn without_a_timescale_directive_the_event_time_is_unchanged() {
    let kernel = compile(REGISTER);
    let non_grid_t = 5.0000001e-9;
    let ev = clock_edge_at(kernel, non_grid_t);

    assert_eq!(
        ev.time, non_grid_t,
        "no `timescale` directive means the event time is not quantized at all"
    );
}
