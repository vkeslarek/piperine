//! LRM-COVERS: A.2.2.2
//!
//! Gap-fix batch (LRMW3-09, `lrm-conformance-wave3` follow-up): spec.md's
//! P2 axis-A AC5 — "WHEN a continuous assignment carries drive strengths
//! (`assign (strong1, weak0) w = r;`) THEN the strengths SHALL reach the
//! `DriverTable` and participate in Table 7-3 resolution." Before this
//! fix, `assign (strong1, weak0) w = r;` did not even parse (no grammar
//! production); `crates/piperine-lang/tests/t43_final_coverage_gaps.rs`
//! pinned that as a declared limitation. This file proves the real,
//! end-to-end mechanism: two module instances drive a shared top-level
//! net with different, explicit drive strengths, and the net's resolved
//! value follows LRM Table 7-3 (the stronger driver wins), not "whichever
//! event happened to apply last" or "both default to Strong and contend
//! to X" (which is what would happen if the parsed strength never
//! actually reached `DriverTable::set_driver`).
//!
//! Uses `fuse_digital_cones(false)` (`CircuitCompiler`'s documented
//! per-device reference path) — the fused whole-cone JIT network
//! (`kernel::digital::network::DigitalNetwork`) does not thread strength
//! at all yet (same disclosed gap as gate-primitive strength,
//! `piperine-lang/tests/annex_a_grammar.rs`'s A.3.2 row); this task's
//! scope is the per-device path strength reaches through
//! `DigitalInstance::push_event` → `EventSink::emit_with_strength` →
//! `DriverTable`.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::LogicValue;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// Builds and runs a `top` module instantiating `drv1`/`drv2` (each
/// driving shared net `bus` through its own strength-carrying continuous
/// assign), settles the digital network, and returns `bus`'s resolved
/// value.
fn run(drv1_strength: &str, drv1_value: &str, drv2_strength: &str, drv2_value: &str) -> LogicValue {
    let src = format!(
        r#"
        discipline logic;
            domain discrete;
        enddiscipline

        module drv1 (output logic w);
            assign {drv1_strength} w = {drv1_value};
        endmodule

        module drv2 (output logic w);
            assign {drv2_strength} w = {drv2_value};
        endmodule

        module top ();
            logic bus;
            drv1 u1 (bus);
            drv2 u2 (bus);
        endmodule
        "#
    );
    let mut design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    // Reference (unfused) per-device path — see module doc.
    compiler.fuse_digital_cones = false;
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));

    let opts = TransientAnalysisOptions::new(1.0, 0.1);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    tran.solve().expect("transient run completes");

    let bus = *info.digital_nets.get("bus").expect("net `bus`");
    circuit.digital_state.nets[bus]
}

/// `u1` drives `1` at `strong1` (Strong), `u2` drives `0` at `weak0`
/// (Weak) — Table 7-3: the stronger driver wins outright, so `bus`
/// resolves to `u1`'s value (`One`), not `X` (which is what a same-
/// strength contention — or a "strength never reaches the scheduler,
/// everything defaults to Strong" bug — would produce).
#[test]
fn stronger_driver_wins_when_it_drives_one() {
    let bus = run("(strong1, weak0)", "1'b1", "(weak1, weak0)", "1'b0");
    assert_eq!(
        bus,
        LogicValue::One,
        "u1's strong1 must beat u2's weak0 per Table 7-3, resolving `bus` to u1's driven value (1)"
    );
}

/// The mirror case, with the strong/weak roles swapped between the two
/// drivers and which value each side drives also swapped: `u1` now
/// drives `1` at `weak1` (Weak), `u2` drives `0` at `strong0` (Strong).
/// Table 7-3 resolution must follow the *strength*, not "first driver
/// wins" or "last driver wins" or a fixed value — `bus` resolves to
/// `Zero` this time, discriminating against those wrong implementations.
#[test]
fn stronger_driver_wins_when_it_drives_zero() {
    let bus = run("(weak1, weak0)", "1'b1", "(strong1, strong0)", "1'b0");
    assert_eq!(
        bus,
        LogicValue::Zero,
        "u2's strong0 must beat u1's weak1 per Table 7-3, resolving `bus` to u2's driven value (0)"
    );
}
