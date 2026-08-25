//! T5 (LRMC-C7-07 enabler): analog `$display` diagnostics reach the host
//! sink through the `pip_task_display` trampoline, and fire **exactly
//! once per accepted transient step — never per Newton iteration**
//! (design.md Risks table: naive wiring "prints thousands of duplicate
//! lines"). Exercised end-to-end through the real solver, the same
//! `parse_and_elaborate` -> `lower_bodies` -> `CircuitCompiler` ->
//! `transient(...).solve()` pipeline `tests/vams_vertical_slice.rs` uses.
//!
//! T27/LRMW2-27 (§9.7.3): `$fatal` terminates the run as soon as its guard
//! is true, without waiting for `accept_timestep` the way `$error` (and
//! `$display` above) does — proven end to end on the same RC-step circuit.
//!
//! LRM-COVERS: 9.3, 9.4.1, 9.4.6, 9.7.3
//! §9.3 ("System tasks/functions executing in the context of the Analog
//! Simulation Cycle"): "a call to such a system task... in an analog
//! block during an iteration that is rejected should cause no
//! side-effects on the next iteration" — exactly what "fires once per
//! accepted step, never per Newton iteration" proves for `$display`.
//! §9.4.1 ("Behavior of the display tasks in the analog context") and
//! §9.4.6 ("Behavior of the display tasks in the analog block during
//! iterative solving" — "shall not display output unless an iteration
//! has been accepted"): `analog_display_fires_once_per_accepted_step_
//! not_per_newton_iteration` proves exactly this — `$display` inside an
//! analog block fires once per accepted transient step, never once per
//! Newton iteration, through a real solver run.

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

/// T27/LRMW2-27: end-to-end proof that `$fatal` reaches a real solver run
/// and actually terminates it (not merely a codegen-level flag check) —
/// the same RC-step circuit `$display`'s own sibling test above uses, with
/// `$fatal` in place of `$display`. `fatal_immediate.rs` (same crate)
/// proves the finer-grained claim this test can't reach through a real
/// solver path (a linear RC circuit's Newton iteration never visits an
/// intermediate guess a `$fatal` guard would react to but the accepted
/// point wouldn't): that the check runs on every `load_dc` call, before
/// any accept/reject decision is even made.
const RC_STEP_WITH_FATAL: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 1.0;
        analog begin
            V(p, n) <+ dc;
        end
    endmodule

    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog begin
            $fatal("overvoltage across the resistor");
            I(p, n) <+ V(p, n) / r;
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
fn fatal_terminates_a_real_solver_run() {
    let design = parse_and_elaborate(RC_STEP_WITH_FATAL, &piperine_lang::SourceMap::dummy())
        .expect("a .vams RC circuit with $fatal parses and elaborates");
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

    // The default panic hook would print a backtrace to the real stderr
    // during this call — harmless test noise, same tolerated pattern
    // `tests/ngspice_temp.rs` already uses for its own `catch_unwind`.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        circuit.transient(options, ctx).expect("transient starts").solve()
    }));

    assert!(result.is_err(), "a $fatal call site must terminate the run, not solve it to completion");
}

/// Negative control / regression (Done-when): `$error` on the identical
/// circuit shape (same guard-free unconditional call site as `$display`'s
/// own sibling test above) still defers to `accept_timestep` — fires
/// exactly once per *accepted* step, never mid-iteration, unaffected by
/// `$fatal`'s new immediate-check path.
const RC_STEP_WITH_ERROR: &str = r#"
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
            $error("resistor tick");
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
fn error_still_fires_once_per_accepted_step_unaffected_by_the_fatal_routing_change() {
    let design = parse_and_elaborate(RC_STEP_WITH_ERROR, &piperine_lang::SourceMap::dummy())
        .expect("a .vams RC circuit with $error parses and elaborates");
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
        .expect("the RC circuit converges (no $fatal call site exists here)");

    let tick_count = sink.stdout().matches("resistor tick\n").count();
    assert_eq!(
        tick_count,
        result.len() - 1,
        "$error must still fire exactly once per accepted step (same relationship $display's \
         own sibling test proves) — never per Newton iteration, and never zero (it must not \
         have been silently dropped by the $fatal routing change)"
    );
}
