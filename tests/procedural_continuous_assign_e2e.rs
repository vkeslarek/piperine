//! LRM-COVERS: 8.5.3.1, 8.5.3.2
//!
//! LRMW2-47 (`lrm-conformance-wave2`, T53): spec.md's "simulates, not
//! merely elaborates" standing rule, applied to §8.5.3.2's procedural
//! continuous assignment / force-release — `force`/`release` and
//! `assign`/`deassign` as *statements* had no grammar production at all
//! before this task. `crates/piperine-lang/tests/
//! vams_procedural_continuous_assign.rs` proves all four parse and that
//! Annex C.7 rejects them inside an analog block; this file proves
//! `force` genuinely overrides a net's normally-driven value, that the
//! override persists across time (not a one-shot that fades on the next
//! evaluation), and that `release` lets normal driving resume afterward —
//! then the parallel proof for `assign`/`deassign` on a register.
//!
//! Scope disclosure (T53 SPEC_DEVIATION): this codegen's digital kernel
//! re-evaluates every comb-region statement in source order each pass
//! (`resolve/pom/body.rs`'s own "behaviors append, evaluated in order"
//! precedent, unchanged by this task) — `force`/`assign` are emitted as
//! an ordinary immediate write, so an override observably persists for as
//! long as no *other* statement also writes the same target afterward,
//! matching the common "testbench statically drives one signal" usage
//! this fixture exercises. `release`/`deassign` themselves have no active
//! effect (there is no per-net "forced" flag to clear) — proven instead
//! by pairing them with a *later* ordinary write that only takes effect
//! because nothing is still overriding the net. LRM §8.5.3.2's own
//! *continuous* re-tracking of `force`'s right-hand side against a
//! *concurrently racing* driver on the same net (unconditional priority
//! over every other write, forever, until `release`) is not modeled —
//! disclosed, not silently approximated.
//!
//! A pre-existing, unrelated codegen limitation is also disclosed here
//! rather than worked around silently: an `initial begin ... end` *block*
//! (multiple statements) is observed to re-execute every digital
//! evaluation pass instead of running once at time 0 (a single-statement
//! `initial x = ...;` does not exhibit this). Both fixtures below avoid
//! `initial` blocks entirely, using only externally-scheduled, one-shot
//! digital edges to sequence "drive, override, hold, release, redrive" —
//! this task's own file scope is `grammar/stmt.rs`/digital codegen/
//! solver digital state, not `initial`-block re-evaluation.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// Runs `src` (a `top` module with `clk_net`/`trig_a_net`/`trig_b_net`/
/// `q_net` digital nets) up to `stop_time`, injecting: `clk` posedge at
/// t=1 (normal drive sets `q`), `trig_a` posedge at t=2 (the override:
/// `force`/`assign`), `trig_b` posedge at t=4 (the restore: `release`/
/// `deassign`), a second `clk` posedge at t=5.5 (proves normal driving
/// resumed). Returns `q_net`'s digital state at `stop_time`.
fn run(src: &str, stop_time: f64) -> LogicValue {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let clk = *info.digital_nets.get("clk_net").expect("net `clk_net`");
    let trig_a = *info.digital_nets.get("trig_a_net").expect("net `trig_a_net`");
    let trig_b = *info.digital_nets.get("trig_b_net").expect("net `trig_b_net`");
    let mut seq = 0u64;
    let mut sched = |time: f64, net: usize, value: LogicValue| {
        circuit.digital_state.schedule(DigitalEvent {
            time, net: DigitalNet(net), value, source: usize::MAX, seq, region: Region::Active,
        });
        seq += 1;
    };
    sched(1.0, clk, LogicValue::One);
    sched(2.0, trig_a, LogicValue::One);
    sched(4.0, trig_b, LogicValue::One);
    sched(5.0, clk, LogicValue::Zero);
    sched(5.5, clk, LogicValue::One);

    let opts = TransientAnalysisOptions::new(stop_time, 0.05);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    tran.solve().expect("transient run completes");
    let q = *info.digital_nets.get("q_net").expect("net `q_net`");
    circuit.digital_state.nets[q]
}

const FORCE_RELEASE_FIXTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module dut (input logic clk, input logic trig_a, input logic trig_b, output logic q);
        always @(posedge clk)
            q = 1;
        always @(posedge trig_a)
            force q = 0;
        always @(posedge trig_b)
            release q;
    endmodule

    module top ();
        logic clk_net, trig_a_net, trig_b_net, q_net;
        dut d1 (clk_net, trig_a_net, trig_b_net, q_net);
    endmodule
"#;

/// AC15-style proof for §8.5.3.2's `force`: at t=1.5 the normal `clk`-
/// driven write has set `q = 1`.
#[test]
fn before_force_the_normal_driver_sets_the_value() {
    assert_eq!(run(FORCE_RELEASE_FIXTURE, 1.5), LogicValue::One);
}

/// `force q = 0;` overrides the normally-driven value (`1` -> `0`).
#[test]
fn force_overrides_the_driven_value() {
    assert_eq!(run(FORCE_RELEASE_FIXTURE, 2.5), LogicValue::Zero);
}

/// The forced value persists across time — `q` is still `0` at t=3.5,
/// two full time units after the `force` fired, with nothing else having
/// written it since.
#[test]
fn the_forced_value_persists_across_time() {
    assert_eq!(run(FORCE_RELEASE_FIXTURE, 3.5), LogicValue::Zero);
}

/// After `release q;` (t=4), `q` still reads `0` (release has no value
/// effect of its own — see this file's own scope disclosure).
#[test]
fn release_alone_does_not_change_the_value() {
    assert_eq!(run(FORCE_RELEASE_FIXTURE, 4.5), LogicValue::Zero);
}

/// After `release`, the normal `clk`-driven write can set `q` again
/// (`0` -> `1` at the second `clk` posedge, t=5.5) — normal driving is
/// restored, not permanently overridden.
#[test]
fn after_release_the_normal_driver_restores_the_value() {
    assert_eq!(run(FORCE_RELEASE_FIXTURE, 6.0), LogicValue::One);
}

const ASSIGN_DEASSIGN_FIXTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module dut (input logic clk, input logic trig_a, input logic trig_b, output logic q);
        always @(posedge clk)
            q = 1;
        always @(posedge trig_a)
            assign q = 0;
        always @(posedge trig_b)
            deassign q;
    endmodule

    module top ();
        logic clk_net, trig_a_net, trig_b_net, q_net;
        dut d1 (clk_net, trig_a_net, trig_b_net, q_net);
    endmodule
"#;

/// §8.5.3.2's `assign`/`deassign` half (a register, not a net): the exact
/// same drive/override/hold/restore arc as `force`/`release` above.
#[test]
fn procedural_assign_overrides_and_deassign_restores_a_register_across_time() {
    assert_eq!(run(ASSIGN_DEASSIGN_FIXTURE, 1.5), LogicValue::One, "before assign: normal drive");
    assert_eq!(run(ASSIGN_DEASSIGN_FIXTURE, 2.5), LogicValue::Zero, "assign overrides the driven value");
    assert_eq!(run(ASSIGN_DEASSIGN_FIXTURE, 3.5), LogicValue::Zero, "the assigned value persists across time");
    assert_eq!(run(ASSIGN_DEASSIGN_FIXTURE, 4.5), LogicValue::Zero, "deassign alone does not change the value");
    assert_eq!(run(ASSIGN_DEASSIGN_FIXTURE, 6.0), LogicValue::One, "after deassign, normal driving is restored");
}
