//! Transient margin evaluation points (dv-core T19, DVC-11): margins are
//! evaluated on **accepted steps only** — a rejected candidate (LTE or
//! Newton) is never an evaluation point and emits no finding — and the
//! t = 0 state is skipped when it came from UIC/`@initial` (a forced
//! initial condition may legitimately sit outside SOA, design §C3), while
//! a computed operating point at t = 0 *is* evaluated. The same fixture
//! run with and without UIC differs only in that first point.

//! **Fully migrated** (constraints-on-vams + the UIC-seed idiom): the
//! fixtures seed via `@(initial_step) V(p, n) <+ value;` — the guarded
//! potential contribution the flattener routes to the t=0 solve + UIC
//! clamps (T44 closed).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::CheckPosture;
use piperine_solver::prelude::{CircuitInstance, Context, TransientAnalysisOptions};

const DISCIPLINES: &str = "`include \"disciplines.vams\"";
/// The still-red UIC fixtures below keep PHDL source (they are blocked on
/// the T44 UIC-producer gap, not on constraint syntax), so they keep the
/// PHDL discipline declaration too.
const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// The `SourceMap` reaching `piperine-lang`'s bundled headers from this
/// crate (`SourceMap::dummy`'s relative paths only resolve from that
/// crate's dir).
fn source_map() -> piperine_lang::SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn build(src: &str) -> CircuitInstance {
    let design = parse_and_elaborate(src, &source_map()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit("Top").expect("circuit builds")
}

fn collect_tran(circuit: &mut CircuitInstance, stop: f64, dt: f64) -> piperine_solver::prelude::TransientAnalysisResult {
    let ctx = Context { checks: CheckPosture::Collect, ..Default::default() };
    circuit
        .transient(TransientAnalysisOptions::new(stop, dt), ctx)
        .unwrap()
        .solve()
        .expect("collect completes")
}

/// The LTE-rejection fixture (mirrors SC-09's kink): a `@timer`-stepped
/// source through `transition` drives an RC; the exponential settle after
/// the ramp-start kink is real curvature, so the Milne gate rejects and
/// halves there. The capacitor's `require lo : V(p, n) >= 2.0` is
/// violated at *every* point (V ≤ 1), so any evaluation — accepted or
/// not — produces a finding, and a rejected candidate's finding would
/// carry a time no accepted step has.
fn kink_circuit() -> CircuitInstance {
    build(&format!(
        "{DISCIPLINES}
module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Capacitor (inout electrical p, inout electrical n);
    parameter real c = 1e-8;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
    end
    constraint begin
        require lo : V(p, n) >= 2.0;
    end
endmodule
module Step (inout electrical p, inout electrical n);
    parameter real tr = 5e-4;
    real st;
    analog begin
        @(timer(1e-3)) st = 1.0;
        V(p, n) <+ transition(st, 0.0, tr, tr);
    end
endmodule
module Top ();
    electrical gnd, step, out;
    Step s1 (.p(step), .n(gnd));
    Resistor r1 (.p(step), .n(out));
    Capacitor c1 (.p(out), .n(gnd));
endmodule
"
    ))
}

/// DVC-11 AC5: a rejected step evaluates no margin and emits no finding.
/// The run provokes LTE rejections; with the require violated everywhere,
/// every finding's time must be an accepted (recorded) step's time — a
/// rejected candidate's evaluation would leave a finding at a time no
/// accepted step ever landed on.
#[test]
fn rejected_steps_evaluate_no_margin_and_emit_no_finding() {
    let mut circuit = kink_circuit();
    let result = collect_tran(&mut circuit, 2e-3, 1e-5);
    assert!(result.stats.steps_rejected > 0, "the fixture must provoke LTE rejections");
    assert!(!result.stats.validation.is_empty(), "the always-violated require produces findings");

    let accepted: Vec<f64> = (0..result.len()).filter_map(|i| result.get(i)).map(|s| s.time()).collect();
    assert!(accepted.len() > 1, "accepted steps recorded");
    for finding in &result.stats.validation {
        assert!(
            accepted.contains(&finding.time),
            "finding at t = {} belongs to no accepted step — a rejected candidate was evaluated",
            finding.time
        );
    }

    let margins = &result.stats.margins;
    assert_eq!(margins.len(), 1, "one declared margin on the capacitor");
    let argmin = margins[0].argmin.expect("the margin was evaluated");
    assert!(
        accepted.contains(&argmin),
        "the worst margin's argmin (t = {argmin}) is an accepted step's time"
    );
}

/// The UIC fixture: a capacitor across a 1 V source, `@initial`-seeded to
/// exactly the source's 1 V — a *consistent* seed, so the ±UIC runs share
/// every accepted step and differ only in the t = 0 state. The
/// `require hi : V(p, n) <= 2.0` has the constant margin 2 − 1 = 1.0 at
/// every point, so the argmin marks the *first evaluated point*: 0.0 when
/// the t = 0 OP is evaluated, the first accepted step when it is skipped.
fn uic_circuit(initial: bool) -> CircuitInstance {
    let seed = if initial {
        "@(initial_step) V(p, n) <+ 1.0;\n        "
    } else {
        ""
    };
    build(&format!(
        "{DISCIPLINES}
module Vsrc (inout Electrical p, inout Electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module Cap (inout Electrical p, inout Electrical n);
    parameter real c = 1e-9;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
        {seed}
    end
    constraint begin
        require hi : V(p, n) <= 2.0;
    end
endmodule
module Top ();
    Electrical gnd, in;
    Vsrc v1 (.p(in), .n(gnd));
    Cap c1 (.p(in), .n(gnd));
endmodule
"
    ))
}

/// DVC-11 AC5: the ±UIC pair differs only in the first point. Without
/// UIC the computed t = 0 operating point is evaluated (the argmin is
/// 0.0); with UIC the t = 0 state is skipped (the argmin is the first
/// accepted step, t > 0) — same trajectory, same worst margin.
#[test]
fn uic_skips_t0_while_a_computed_op_is_evaluated() {
    let mut plain = uic_circuit(false);
    let no_uic = collect_tran(&mut plain, 1e-6, 1e-7);
    assert_eq!(no_uic.stats.margins.len(), 1);
    assert!(
        (no_uic.stats.margins[0].worst - 1.0).abs() < 1e-9,
        "margin 2 − 1 = 1.0 at every point, got {}",
        no_uic.stats.margins[0].worst
    );
    assert_eq!(
        no_uic.stats.margins[0].argmin,
        Some(0.0),
        "the computed t = 0 operating point is an evaluation point"
    );

    let mut seeded = uic_circuit(true);
    let uic = collect_tran(&mut seeded, 1e-6, 1e-7);
    assert_eq!(uic.stats.margins.len(), 1);
    assert!(
        (uic.stats.margins[0].worst - 1.0).abs() < 1e-9,
        "same trajectory, same worst margin — the runs differ only in the first point, got {}",
        uic.stats.margins[0].worst
    );
    let argmin = uic.stats.margins[0].argmin.expect("evaluated at accepted steps");
    assert!(argmin > 0.0, "the UIC t = 0 state is skipped — the first evaluated point is an accepted step, got {argmin}");
    assert!(
        uic.stats.validation.is_empty() && no_uic.stats.validation.is_empty(),
        "a satisfied require reports nothing in either run"
    );
}

/// DVC-11 AC5: an *inconsistent* UIC seed that violates the require is
/// still never evaluated at t = 0 — the violation surfaces only from the
/// clamped first accepted step onward (ngspice CKTsetIC holds the seed
/// through it), never with a t = 0 coordinate. Strict would abort at that
/// first accepted step, so collect carries the proof.
#[test]
fn a_violated_uic_state_is_skipped_at_t0() {
    let mut circuit = build(&format!(
        "{DISCIPLINES}
module Vsrc (inout Electrical p, inout Electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout Electrical p, inout Electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Cap (inout Electrical p, inout Electrical n);
    parameter real c = 1e-9;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
        @(initial_step) V(p, n) <+ 5.0;
    end
    constraint begin
        require hi : V(p, n) <= 2.0;
    end
endmodule
module Top ();
    Electrical gnd, in, x;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(x));
    Cap c1 (.p(x), .n(gnd));
endmodule
"
    ));
    let result = collect_tran(&mut circuit, 1e-6, 1e-7);
    assert!(
        !result.stats.validation.is_empty(),
        "the 5 V seed violates the 2 V require once the clamped first step is accepted"
    );
    for finding in &result.stats.validation {
        assert!(
            finding.time > 0.0,
            "the UIC t = 0 state is never an evaluation point — no finding at t = 0, got one"
        );
    }
    let argmin = result.stats.margins[0].argmin.expect("evaluated at accepted steps");
    assert!(argmin > 0.0, "no t = 0 evaluation, got argmin {argmin}");
}
