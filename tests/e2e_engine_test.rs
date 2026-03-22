//! End-to-end test using the high-level NgspiceEngine API.

use piperine_core::prelude::*;
use piperine_core::engine::SimulationEngine;
use piperine_pool::NgspiceEngine;

fn make_engine() -> NgspiceEngine {
    let exe = env!("CARGO_BIN_EXE_piperine");
    NgspiceEngine::with_exe(exe, 1).expect("failed to create engine")
}

#[test]
fn engine_op_analysis() {
    let engine = make_engine();

    let ckt = Circuit::new("OP Test")
        .vdc("1", "in", GND, 10.0)
        .resistor("1", "in", "out", "10k")
        .resistor("2", "out", GND, "10k");

    let op = OpAnalysis::new();

    let result = engine.run(&ckt, &op).expect("simulation failed");

    // Should have at least one plot
    assert!(!result.plots.is_empty(), "no plots returned");

    // Print what we got
    for (name, plot) in &result.plots {
        println!("Plot '{}' ({:?}): {} vectors", name, plot.plot_type, plot.vectors.len());
        for (vname, _) in &plot.vectors {
            println!("  {}", vname);
        }
    }
}

#[test]
fn engine_dc_sweep() {
    let engine = make_engine();

    let ckt = Circuit::new("DC Sweep")
        .vdc("in", "in", GND, 0.0)
        .resistor("1", "in", "out", "1k")
        .resistor("2", "out", GND, "1k");

    let dc = DcAnalysis::new("Vin", 0.0, 10.0, 0.5)
        .save("v(out)");

    let result = engine.run(&ckt, &dc).expect("DC sweep failed");
    assert!(!result.plots.is_empty());
}

#[test]
fn engine_tran_analysis() {
    let engine = make_engine();

    let ckt = Circuit::new("Transient RC")
        .vsource("in", "in", GND,
            Waveform::Pulse(Pulse::new(0.0, 5.0)
                .delay(1e-6)
                .rise(1e-9)
                .fall(1e-9)
                .width(50e-6)
                .period(100e-6)))
        .resistor("1", "in", "out", "1k")
        .capacitor("1", "out", GND, "100n");

    let tran = TranAnalysis::new(1e-7, 200e-6)
        .save("v(out)")
        .save("v(in)");

    let result = engine.run(&ckt, &tran).expect("tran failed");
    assert!(!result.plots.is_empty());
}

#[test]
fn engine_ac_analysis() {
    let engine = make_engine();

    let ckt = Circuit::new("AC RC Filter")
        .vac("in", "in", GND, 1.0)
        .resistor("1", "in", "out", "1k")
        .capacitor("1", "out", GND, "1u");

    let ac = AcAnalysis::new(Variation::Dec, 10, 1.0, 1e6)
        .save("v(out)");

    let result = engine.run(&ckt, &ac).expect("AC analysis failed");
    assert!(!result.plots.is_empty());
}
