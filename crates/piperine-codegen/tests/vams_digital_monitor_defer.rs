//! LRM-COVERS: 8.5.1, 9.4
//! T62 (LRMW2-59): `$strobe`/`$monitor` defer to the Monitor region
//! (§8.5.1) instead of calling out immediately like `$display` — `emit/
//! stmt.rs`'s `S::TaskCall` arm routes them through
//! `pip_task_defer_monitor` (pushes onto `SimCtx::monitor_queue`) instead
//! of `pip_task_display`; `device/digital.rs` drains that queue only
//! after the enclosing `seq`/`comb` call *and* its NBA commit have both
//! finished. Mirrors `vams_digital_display.rs`'s real-transient-run
//! infrastructure (`BufferSink`, `CircuitCompiler`, `circuit.transient`).

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

/// `$strobe("B")` appears *before* `$display("A")` in source, inside the
/// same clocked block that also does a `<=` (NBA) write — deliberately
/// adversarial ordering: an immediate-call implementation would print
/// "B" before "A" (source order); a correctly Monitor-deferred `$strobe`
/// must print "A" first regardless, since `$display` (Active region)
/// always lands before `$strobe` (Monitor region, drained only after
/// this evaluation's NBA commit).
const COUNTER_WITH_STROBE_BEFORE_DISPLAY: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module counter (input logic clk, output logic q);
        integer count;
        always @(posedge clk) begin
            $strobe("B");
            $display("A");
            count <= count + 1;
        end
    endmodule

    module top ();
        logic clk_net, q_net;
        counter c1 (clk_net, q_net);
    endmodule
"#;

fn run_counter(edges: u32) -> Rc<BufferSink> {
    let design = parse_and_elaborate(COUNTER_WITH_STROBE_BEFORE_DISPLAY, &piperine_lang::SourceMap::dummy())
        .expect("counter with $strobe/$display elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let clk = DigitalNet(*info.digital_nets.get("clk_net").expect("net `clk`"));

    for i in 0..edges {
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

    let opts = TransientAnalysisOptions::new(2.0 * edges as f64, 0.1);
    let mut tran = circuit.transient(opts, ctx).unwrap();
    tran.solve().expect("transient run completes");
    sink
}

/// T62 Done-when #1 (discriminating): `$strobe` reports after `$display`
/// within the same evaluation, even though it appears *earlier* in
/// source — proving the call is genuinely deferred to the Monitor
/// region, not just reordered by chance.
#[test]
fn strobe_lands_after_display_from_the_same_evaluation_despite_source_order() {
    let sink = run_counter(3);
    let out = sink.stdout();
    assert_eq!(out.matches("A\n").count(), 3, "one $display per posedge: {out:?}");
    assert_eq!(out.matches("B\n").count(), 3, "one $strobe per posedge: {out:?}");
    // Every "A\n" must precede the "B\n" from the *same* evaluation — with
    // one strobe/display pair per posedge, the strict per-instance
    // ordering reduces to: no "B\n" occurs before the *first* "A\n", and
    // the interleaving is exactly A,B,A,B,A,B (never B,A anywhere).
    assert_eq!(out, "A\nB\nA\nB\nA\nB\n", "$strobe must land after $display every time, despite source order: {out:?}");
}

/// T62 Done-when #2: `$display` itself is unaffected — still Active
/// region, still prints inline. Reusing the fixture and sink from the
/// discriminating test above: the "A\n" count already proves every
/// `$display` fired (immediate, one per posedge); this test isolates
/// that specific claim rather than only inferring it from the combined
/// interleaving assertion above.
#[test]
fn display_stays_immediate_active_region() {
    let sink = run_counter(1);
    let out = sink.stdout();
    assert_eq!(out, "A\nB\n", "single edge: $display (\"A\") immediate, $strobe (\"B\") deferred but not lost: {out:?}");
}
