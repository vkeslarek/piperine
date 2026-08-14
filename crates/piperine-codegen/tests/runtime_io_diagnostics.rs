//! T5 (LRMC-C7-07 enabler): analog `$display` diagnostics reach the host
//! sink through the `pip_task_display` trampoline, and fire **exactly
//! once per accepted transient step — never per Newton iteration**
//! (design.md Risks table: naive wiring "prints thousands of duplicate
//! lines"). Exercised end-to-end through the real solver, the same
//! `parse_and_elaborate` -> `lower_bodies` -> `CircuitCompiler` ->
//! `transient(...).solve()` pipeline `tests/vams_vertical_slice.rs` uses.

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

/// `tests/vams_vertical_slice.rs`'s own RC-step circuit, with one
/// `$display` added to the resistor's analog body — the exact statement
/// shape `flatten/analog.rs`'s `PomStmt::TaskCall` arm collects into
/// `FlatDiagnostic`.
const RC_STEP_WITH_DISPLAY: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 1.0;
        analog begin
            V(p, n) <+ dc;
        end
    endmodule

    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
            $display("tick");
        end
    endmodule

    module capacitor (inout electrical p, inout electrical n);
        parameter real c = 1.0e-6;
        analog begin
            I(p, n) <+ c * ddt(V(p, n));
        end
    endmodule

    module top ();
        electrical gnd;
        electrical vin;
        electrical vout;
        vsrc #(.dc(1.0)) v1 (vin, gnd);
        resistor #(.r(1.0e3)) r1 (vin, vout);
        capacitor #(.c(1.0e-6)) c1 (vout, gnd);
    endmodule
"#;

#[test]
fn analog_display_fires_once_per_accepted_step_not_per_newton_iteration() {
    let design = parse_and_elaborate(RC_STEP_WITH_DISPLAY, &piperine_lang::SourceMap::dummy())
        .expect("a .vams RC circuit with $display parses and elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers to the resolved form");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) =
        compiler.build_circuit_mapped("top").expect("CircuitCompiler builds a device tree");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c;
    let stop = 5.0 * tau;
    let options = TransientAnalysisOptions::new(stop, tau / 50.0);
    let result = circuit
        .transient(options, ctx)
        .expect("transient analysis starts")
        .solve()
        .expect("the RC circuit converges");

    assert!(result.len() > 10, "the run recorded a meaningful number of accepted steps");

    // The transient's very first recorded point (t=0) is the DC operating
    // point used to seed the run — a DC solve, not a stepped transient
    // point, so it never reaches `Element::accept_timestep` (that hook is
    // transient-stepping-specific). Every subsequent recorded point IS an
    // accepted transient step, so `tick_count == result.len() - 1` is the
    // exact expected relationship — not an approximation. If diagnostics
    // fired per Newton iteration instead, `tick_count` would be many times
    // `result.len()` (TrBdf2 evaluates the residual multiple times per
    // accepted step), which this equality rules out precisely.
    let tick_count = sink.stdout().matches("tick\n").count();
    assert_eq!(
        tick_count,
        result.len() - 1,
        "each accepted transient step (all but the t=0 DC-seed point) must print exactly \
         one \"tick\" line — never one per Newton iteration — got {tick_count} lines for \
         {} accepted points",
        result.len()
    );

    // A negative control: the resistor's diagnostic must not leak into
    // stderr, and the capacitor/source (which never call $display) must
    // not somehow contribute extra lines.
    assert_eq!(sink.stderr(), "", "no $fatal/$error path was exercised");
    assert_eq!(
        sink.stdout().lines().count(),
        tick_count,
        "every captured line must be exactly one \"tick\" — no stray output"
    );
}
