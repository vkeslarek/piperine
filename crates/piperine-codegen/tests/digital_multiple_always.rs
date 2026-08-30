//! A module may declare more than one digital behavior. Two `always`
//! blocks are independent concurrent processes in Verilog and **both**
//! run — `resolve::pom::body` used to assign `bodies[i].digital`
//! unconditionally inside its per-behavior loop, so only the last
//! behavior survived and every earlier one was dropped with no
//! diagnostic at all: the module elaborated clean, compiled clean, and
//! silently simulated missing logic.
//!
//! Proven through the host output sink rather than by inspecting the
//! lowered body: each behavior carries its own `$display`, so a dropped
//! behavior is a missing line in the captured stdout — an end-to-end
//! observation a shape-only assertion could not make.

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

const EDGES: u32 = 3;

/// Drives `clk_net` for `EDGES` rising edges and returns everything the
/// design wrote to stdout.
fn run_and_capture(src: &str) -> String {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let clk = DigitalNet(*info.digital_nets.get("clk_net").expect("net `clk_net`"));
    for i in 0..EDGES {
        let base = (2 * i) as f64;
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 0.5,
            net: clk,
            value: LogicValue::Zero,
            source: usize::MAX,
            seq: (2 * i) as u64,
            region: Region::Active, strength: None });
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 1.0,
            net: clk,
            value: LogicValue::One,
            source: usize::MAX,
            seq: (2 * i + 1) as u64,
            region: Region::Active, strength: None });
    }

    let mut tran = circuit.transient(TransientAnalysisOptions::new(6.0, 0.1), ctx).unwrap();
    tran.solve().expect("transient run completes");
    sink.stdout()
}

/// Two clocked behaviors in one module: both fire on every edge.
#[test]
fn two_clocked_always_blocks_both_run() {
    let out = run_and_capture(
        r#"
        discipline logic;
            domain discrete;
        enddiscipline

        module counter (input logic clk, output logic q);
            integer a, b;
            always @(posedge clk) begin
                a <= a + 1;
                $display("first");
            end
            always @(posedge clk) begin
                b <= b + 1;
                $display("second");
            end
        endmodule

        module top ();
            logic clk_net, q_net;
            counter c1 (clk_net, q_net);
        endmodule
        "#,
    );

    assert_eq!(
        out.matches("first\n").count(),
        EDGES as usize,
        "the FIRST always block must fire on every edge — dropping it is the \
         regression this test exists for. Captured stdout: {out:?}"
    );
    assert_eq!(
        out.matches("second\n").count(),
        EDGES as usize,
        "the second always block must fire on every edge too: {out:?}"
    );
}

/// A combinational behavior and a clocked one coexist: the per-statement
/// comb/seq split in `kernel::digital::compile` routes each to its own
/// list, so concatenating the two behaviors' statements is equivalent to
/// compiling them separately.
#[test]
fn a_combinational_and_a_clocked_always_block_coexist() {
    let out = run_and_capture(
        r#"
        discipline logic;
            domain discrete;
        enddiscipline

        module mixed (input logic clk, output logic q);
            integer n;
            always @(posedge clk) begin
                n <= n + 1;
                $display("seq");
            end
            always @(*) begin
                $display("comb");
            end
        endmodule

        module top ();
            logic clk_net, q_net;
            mixed m1 (clk_net, q_net);
        endmodule
        "#,
    );

    assert_eq!(
        out.matches("seq\n").count(),
        EDGES as usize,
        "the clocked block fires once per edge: {out:?}"
    );
    assert!(
        out.contains("comb\n"),
        "the combinational block must run as well — it is re-evaluated every \
         digital pass, so it must appear at least once: {out:?}"
    );
}
