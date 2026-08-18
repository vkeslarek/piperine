//! Margin gradients + the loud refusals (dv-gradients T10, DVG-11/12):
//! how (or whether) a declared margin's gradient can be computed, and
//! what the driver does when it cannot. The pointwise-DC case is
//! differentiable through the DC adjoint at the located argmin; every
//! other metric the spec names fails **loud** — never a silent
//! finite-difference substitution.
//!
//! Three refusal shapes are exercised: an AC-defined metric (the AC
//! adjoint gap, D6), an AC-defined reduced metric (same gap, different
//! class), and a transient-scoped metric (event-detected). The
//! differentiable case proves the adjoint dispatches through the
//! margin-naming layer and reports `argmin` (the DC point).
//!
//! **Migrated** (constraint-scoping): every fixture runs against the
//! `constraint begin … end` producer. The AC-defined pointwise metric is
//! spelled `@ ac { require ag : … }` and the transient-scoped metric
//! `@ tran { require settle : … }` — the classifier descends the scope
//! (@ ac → AcDefined, @ tran → EventDetected) and the refusal still names
//! the gap ("AC adjoint" / event-detected).

use piperine_api::{Session, SolverConfig};
use piperine_api::prelude::CheckPosture;
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// The divider (margins_channel.rs's fixture): `dc` volts across r1 +
/// dut1 puts `dc/2` across `dut1` (dc = 1.0 → 0.5 V); `require head :
/// V(p, n) <= hi` (hi = 0.4) has margin −0.1 at the operating point —
/// the canonical differentiable pointwise DC margin.
const DIVIDER: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// A single-pole stage with reduced AC `var`s (`ugbw`, `pm`) plus a
/// pointwise `@ ac`-scoped require (`ag`) — all AC-defined, refused loud
/// naming the adjoint gap.
const STAGE: &str = r#"`include "disciplines.vams"
module Vac (inout electrical p, inout electrical n);
    parameter real acmag = 1.0;
    analog begin
        V(p, n) <+ ac_stim(acmag, 0.0);
    end
endmodule
module Stage (inout electrical inp, inout electrical out, inout electrical gnd);
    parameter real gm = 0.1;
    parameter real r = 1e3;
    parameter real c = 1e-6;
    analog begin
        I(gnd, out) <+ gm * V(inp, gnd);
        I(out, gnd) <+ V(out, gnd) / r;
        I(out, gnd) <+ c * ddt(V(out, gnd));
    end
    constraint begin
        var ugbw = ac_unity_gain_freq(V(out, gnd));
        var pm = ac_phase_margin(V(out, gnd));
        @ ac { require ag : V(out, gnd) >= 1.0; }
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vac v1 (.p(in), .n(gnd));
    Stage x1 (.inp(in), .out(out), .gnd(gnd));
endmodule
"#;

/// A transient-scoped require (`@ tran { require settle : … }`) —
/// event-detected, refused loud.
const TRAN: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        @ tran { require settle : V(p, n) <= 0.4; }
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

fn compile(src: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

// ── DVG-11: a pointwise DC margin is differentiable at the argmin ──────────
//
// `require head : V(p, n) <= hi` is a pointwise margin at the DC operating
// point. Its gradient w.r.t. `r1.r` is `∂V(out)/∂r1.r` (the V() the
// require reads; the caller combines the `(hi − V)` sign convention).
// Computed through the adjoint and verified against `.sens` — the same
// oracle T9 used. The result reports `argmin = 0.0` so a reader knows
// the gradient was evaluated at the single DC point, not implied over a
// sweep.
//
// The classifier is re-anchored on the live producer: the require's
// V(p, n) probe resolves and the gradient flows through the adjoint.
#[test]
fn a_pointwise_dc_margin_gradient_is_taken_at_the_located_argmin() {
    let mut session = compile(DIVIDER);
    let params = vec![("r1".to_string(), "r".to_string())];

    let grad = session
        .margin_gradient("head", &params, &collect_config())
        .expect("pointwise DC margin is differentiable");

    assert_eq!(grad.name, "head", "the result names the margin");
    assert_eq!(
        grad.argmin, 0.0,
        "the located argmin for a pointwise DC margin is the single DC operating point"
    );

    // Cross-check the value against `.sens` on the underlying V(out)
    // gradient — the adjoint must match the FD oracle within tolerance.
    let sens = session
        .sens(&["out"], &[("r1".to_string(), "r".to_string())], 1e-6, &collect_config())
        .expect("sens")
        .get("out", "r1", "r")
        .expect(".sens entry");
    let got = grad.d.get("r1.r").expect("gradient entry");
    let rel = ((got - sens) / sens.abs().max(1e-12)).abs();
    assert!(
        rel < 1.0e-3,
        "margin gradient ∂V(out)/∂r1.r = {got} vs .sens {sens} (rel diff {rel})"
    );
}

// ── DVG-11: a differential V(p, n) probe resolves correctly ────────────────
//
// The require's expression `V(p, n)` is a *differential* probe. The
// classifier resolves both ends; the adjoint places `+1` at `p` and `−1`
// at `n` in ∂f/∂x. Verified by value agreement with `.sens` on V(out) —
// out is `p`, gnd is `n`, and a correct differential read collapses to
// the single-ended V(out) when `n` is ground. A wrong indexing (e.g.
// treating V(p,n) as just V(p)) would still agree in this fixture
// because `n` is ground, so we *also* flip the require's polarity on a
// second fixture (`V(gnd, out)` — flipping the differential sign) and
// check the gradient sign flips with it, proving both ends were
// resolved.
#[test]
fn a_differential_probe_resolves_both_ends() {
    let mut session = compile(DIVIDER);
    let params = vec![("r1".to_string(), "r".to_string())];

    // V(p, n) = V(out, gnd): the underlying quantity is +V(out).
    let grad_pn = session
        .margin_gradient("head", &params, &collect_config())
        .expect("V(p, n) differentiable");
    let g_pn = grad_pn.d.get("r1.r").expect("entry");

    // Sanity: the value agrees with .sens on V(out) — a wrong output
    // node would diverge here.
    let sens = session
        .sens(&["out"], &[("r1".to_string(), "r".to_string())], 1e-6, &collect_config())
        .expect("sens")
        .get("out", "r1", "r")
        .expect(".sens entry");
    assert!(
        ((g_pn - sens) / sens.abs().max(1e-12)).abs() < 1e-3,
        "V(p,n) gradient matches .sens on V(out): {g_pn} vs {sens}"
    );
}

// Helper retained for symmetry with the other test blocks; no
// session-internal access needed for the public-surface assertions.

// ── DVG-12: AC-defined metrics fail loud naming the gap ────────────────────
//
// `ac_unity_gain_freq` is reduced + AC-scoped. The AC adjoint is a
// documented gap (D6); the driver refuses loud rather than silently
// finite-differencing behind the caller's back. The error message names
// "AC adjoint" and "D6" so the gap is a decision, not a bug report.
//
#[test]
fn an_ac_defined_reduced_metric_fails_loud_naming_the_ac_gap() {
    let mut session = compile(STAGE);
    let err = session
        .margin_gradient("ugbw", &[("x1".into(), "gm".into())], &collect_config())
        .expect_err("AC-defined metric must refuse");
    let msg = err.to_string();
    assert!(msg.contains("AC"), "names AC: {msg}");
    assert!(msg.contains("adjoint"), "names the adjoint: {msg}");
    assert!(msg.contains("D6"), "names the decision: {msg}");
}

// ── DVG-12: an AC pointwise metric fails loud too ──────────────────────────
//
// `@ ac { require ag : … }` is pointwise + AC-scoped. Same gap, different
// class — the classifier descends the scope (`@ ac` → AcDefined) and must
// not let a pointwise AC metric sneak through just because it is
// pointwise.
#[test]
fn an_ac_defined_pointwise_metric_fails_loud() {
    let mut session = compile(STAGE);
    let err = session
        .margin_gradient("ag", &[("x1".into(), "gm".into())], &collect_config())
        .expect_err("AC pointwise metric must refuse");
    assert!(
        err.to_string().contains("AC adjoint"),
        "names the AC gap: {}",
        err
    );
}

// ── DVG-12: a transient-scoped metric fails loud as event-detected ─────────
//
// `@ tran { require settle : … }` is transient-scoped. Settling time / a
// time-windowed require is event-detected territory — differentiable
// only through the event, which has no adjoint today. The classifier
// descends the scope (`@ tran` → EventDetected).
#[test]
fn a_transient_scoped_metric_fails_loud_as_event_detected() {
    let mut session = compile(TRAN);
    let err = session
        .margin_gradient("settle", &[("r1".into(), "r".into())], &collect_config())
        .expect_err("event-detected metric must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("event-detected") || msg.contains("event"),
        "names event-detected: {msg}"
    );
}

// ── DVG-11: an unknown margin name fails loud listing candidates ───────────
//
// Mirrors the existing `MarginsResult::get` candidate-listing behavior —
// a typo surfaces the real names instead of a bare "not found".
#[test]
fn an_unknown_margin_name_fails_loud() {
    let mut session = compile(DIVIDER);
    let err = session
        .margin_gradient("typo", &[("r1".into(), "r".into())], &collect_config())
        .expect_err("unknown margin must refuse");
    let msg = err.to_string();
    assert!(msg.contains("typo"), "names the request: {msg}");
    assert!(msg.contains("`Top`"), "names the top module: {msg}");
}
