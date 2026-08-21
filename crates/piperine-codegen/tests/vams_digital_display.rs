//! LRM-COVERS: 9.2
//! LRMC-C7-07 (T43): digital `$display`/`$write` reach the host output
//! path — a real Cranelift-emitted call to the `pip_task_display`/
//! `pip_task_write` trampolines (`emit::stmt.rs`'s `S::TaskCall` arm),
//! captured through a `BufferSink` end-to-end. Mirrors
//! `runtime_io_diagnostics.rs`'s analog proof, on the digital side.

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

const COUNTER_WITH_DISPLAY: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module counter (input logic clk, output logic q);
        integer count;
        always @(posedge clk) begin
            count <= count + 1;
            $display("tick");
            $write("w");
        end
    endmodule

    module top ();
        logic clk_net, q_net;
        counter c1 (clk_net, q_net);
    endmodule
"#;

#[test]
fn digital_display_and_write_reach_the_host_sink() {
    let design = parse_and_elaborate(COUNTER_WITH_DISPLAY, &piperine_lang::SourceMap::dummy())
        .expect("counter with $display/$write elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let clk = DigitalNet(*info.digital_nets.get("clk_net").expect("net `clk`"));

    const EDGES: u32 = 3;
    for i in 0..EDGES {
        let base = (2 * i) as f64;
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 0.5,
            net: clk,
            value: LogicValue::Zero,
            source: usize::MAX,
            seq: (2 * i) as u64,
            region: Region::Active,
        });
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 1.0,
            net: clk,
            value: LogicValue::One,
            source: usize::MAX,
            seq: (2 * i + 1) as u64,
            region: Region::Active,
        });
    }

    let opts = TransientAnalysisOptions::new(6.0, 0.1);
    let mut tran = circuit.transient(opts, ctx).unwrap();
    tran.solve().expect("transient run completes");

    let tick_count = sink.stdout().matches("tick\n").count();
    assert_eq!(tick_count, EDGES as usize, "one \"tick\\n\" per posedge, got: {:?}", sink.stdout());

    let write_count = sink.stdout().matches('w').count();
    assert_eq!(write_count, EDGES as usize, "$write has no trailing newline, one per posedge");
    assert!(!sink.stdout().contains("w\n"), "$write must not add a newline: {:?}", sink.stdout());
}
