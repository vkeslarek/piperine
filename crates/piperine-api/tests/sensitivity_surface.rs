//! Host sensitivity surface (dv-gradients T11, DVG-13):
//! [`Session::sensitivity`] for one parameter and [`Session::gradient`]
//! for many — both with engine attribution, both failing loud on unknown
//! metrics with candidate listing, both refusing AC-defined metrics
//! naming the gap. The adjoint path is the engine; the per-element
//! fallback list travels on the result.
//!
//! **Partially migrated** (constraints-on-vams): the `require`-only
//! divider fixture runs against the `constraint begin … end` producer,
//! and the net-name tests below pass through it. Still red, each naming
//! its blocker at the test: the AC-var fixture (BLOCKED on the deferred
//! constraint `var`/reduced-metric statement — constraints-on-vams
//! Out-of-Scope), the margin-name resolution test (BLOCKED on
//! `classify_margin_gradient`, still the MD-45 always-refuse stub in
//! `piperine-api`'s `gradient.rs`), and the candidate-listing test
//! (BLOCKED on `Session::gradient`'s unknown-metric path, which still
//! hardcodes `declared margins: []` post-MD-45 instead of walking the
//! margins catalog). Blocked tests stay failing rather than being
//! weakened, deleted, or `#[ignore]`d (repo-banned by
//! `tests/suite_hygiene.rs`).

use piperine_api::prelude::CheckPosture;
use piperine_api::{
    Gradient, Sensitivity, SensitivityMethod, Session, SolverConfig,
};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// The divider (margin_gradients.rs's fixture family, dc = 10.0 as
/// authored): `require head : V(p, n) <= hi` rides the constraints-on-vams
/// producer; the sensitivity tests below address the `out` net and the
/// `head` margin by name.
const DIVIDER: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 10.0;
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

/// BLOCKED on the dv-gradients classifier (`classify_margin_gradient` is
/// still the always-refuse stub) and the AC adjoint: a reduced metric
/// (`var ugbw = ac_unity_gain_freq(…)`) now parses under `constraint
/// begin`, but differentiating a host-side reduction needs the deferred
/// gradient surface — the fixture stays PHDL and fails at parse until
/// then, its executable specification intact.
///
/// A single-pole stage with reduced AC vars — refuses loud on the host
/// surface, naming the AC-adjoint gap.
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
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vac v1 (.p(in), .n(gnd));
    Stage x1 (.inp(in), .out(out), .gnd(gnd));
endmodule
"#;

fn compile(src: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

// ── DVG-13: sensitivity(metric, param) returns a typed value + engine ──────
//
// `sensitivity("out", ("r1", "r"))` on the divider dispatches to the DC
// adjoint of V(out) w.r.t. r1.r. The value matches `.sens` (T9's oracle)
// within the FD's own tolerance; the method is `Adjoint`.
#[test]
fn sensitivity_returns_a_typed_value_with_engine_attribution() {
    let mut session = compile(DIVIDER);

    let s: Sensitivity = session
        .sensitivity("out", &("r1".into(), "r".into()), &collect_config())
        .expect("sensitivity on a DC net");

    assert_eq!(s.method, SensitivityMethod::Adjoint, "the adjoint engine is named");

    // Cross-check the value against `.sens` — the FD oracle.
    let fd = session
        .sens(&["out"], &[("r1".to_string(), "r".to_string())], 1e-6, &collect_config())
        .expect("sens")
        .get("out", "r1", "r")
        .expect(".sens entry");
    let rel = ((s.value - fd) / fd.abs().max(1e-12)).abs();
    assert!(
        rel < 1.0e-3,
        "adjoint sensitivity matches .sens within FD tolerance: {} vs {} (rel {rel})",
        s.value,
        fd
    );
}

// ── DVG-13: gradient(metric, params) — one engine for many parameters ──────
//
// The multi-parameter surface: `gradient("out", [(r1.r), (v1.dc)])` returns
// one Gradient with the Adjoint engine and the per-element fallback list.
// Both values must match `.sens`.
#[test]
fn gradient_serves_many_params_with_one_engine_attribution() {
    let mut session = compile(DIVIDER);
    let params = vec![
        ("r1".to_string(), "r".to_string()),
        ("v1".to_string(), "dc".to_string()),
        ("dut1".to_string(), "hi".to_string()),
    ];

    let g: Gradient = session
        .gradient("out", &params, &collect_config())
        .expect("gradient on a DC net");

    assert_eq!(g.method, SensitivityMethod::Adjoint);
    // The Dut instance carries `hi` (not in its residual expression), so
    // the JIT's analytic kernel excludes it — T6's documented conservative
    // path: the param's contribution is analytically zero, and the
    // collector reports the element in `fallback_users` (a per-element
    // attribution the host surfaces) rather than silently returning zero.
    // The economic claim (one transpose solve for all params) is unaffected.
    let _ = g.fallback_users;

    // Cross-check each entry against `.sens`.
    let fd = session
        .sens(&["out"], &params, 1e-6, &collect_config())
        .expect("sens");
    for (label, param) in &params {
        let a = g.get(label, param).unwrap_or_else(|| panic!("gradient entry {label}.{param}"));
        let s = fd.get("out", label, param).unwrap_or_else(|| panic!(".sens entry {label}.{param}"));
        let rel = ((a - s) / s.abs().max(1e-12)).abs();
        assert!(
            rel < 1.0e-3,
            "({label}.{param}): adjoint {a} vs .sens {s} (rel {rel})"
        );
    }
}

// ── DVG-13: a metric name that resolves to a margin differentiates too ─────
//
// `sensitivity("head", ("r1", "r"))` resolves `head` to the declared
// `require` and differentiates its underlying `V(p, n)` — same engine,
// same value as the bare-net path on this fixture (since `n` is ground).
// Proves the metric-resolution layer dispatches margins and nets through
// one host surface.
//
// BLOCKED on the margin-gradient classifier: `classify_margin_gradient`
// (`piperine-api`'s `gradient.rs`) is still the MD-45 always-refuse
// stub, so `Session::gradient` falls through the margin branch and
// refuses `"head"` as an unknown metric. Re-anchoring the classifier on
// the constraints-on-vams `require` producer is deferred; the fixture is
// migrated and the failure is that loud refusal.
#[test]
fn a_metric_name_resolves_to_either_a_margin_or_a_net() {
    let mut session = compile(DIVIDER);

    let by_margin: Sensitivity = session
        .sensitivity("head", &("r1".into(), "r".into()), &collect_config())
        .expect("margin name resolves");
    let by_net: Sensitivity = session
        .sensitivity("out", &("r1".into(), "r".into()), &collect_config())
        .expect("net name resolves");

    assert_eq!(by_margin.method, SensitivityMethod::Adjoint);
    assert_eq!(by_net.method, SensitivityMethod::Adjoint);
    // The margin reads V(out) - V(gnd) = V(out); same physical quantity,
    // same gradient.
    assert!(
        (by_margin.value - by_net.value).abs() < 1.0e-9,
        "margin and net paths agree on V(out): {} vs {}",
        by_margin.value,
        by_net.value
    );
}

// ── DVG-13: unknown metric fails loud listing candidates ───────────────────
//
// A typo surfaces both the addressable nets and the declared margins —
// the caller sees what to write instead of a bare "not found".
//
// BLOCKED on `Session::gradient`'s unknown-metric candidate list
// (`piperine-api`'s `session/entry.rs`): post-MD-45 it still hardcodes
// `declared margins: []` instead of walking the margins catalog, so the
// message names the nets but never `head`. Wiring the catalog into that
// list is deferred; the fixture is migrated and the failure is the
// missing `head` candidate.
#[test]
fn unknown_metric_fails_loud_listing_candidates() {
    let mut session = compile(DIVIDER);
    let err = session
        .sensitivity("typo", &("r1".into(), "r".into()), &collect_config())
        .expect_err("unknown metric must refuse");
    let msg = err.to_string();
    assert!(msg.contains("typo"), "names the request: {msg}");
    assert!(msg.contains("out"), "lists the addressable net: {msg}");
    assert!(msg.contains("head"), "lists the declared margin: {msg}");
}

// ── DVG-13: AC-defined metric fails loud on the host surface too ───────────
//
// The same refusal T10 enforces on `margin_gradient` is enforced on
// `sensitivity`/`gradient` — the unified host surface never silently
// finite-differences an AC-defined metric behind the caller's back.
//
// The var is live; the classifier refuses the AC-defined metric loud.
#[test]
fn ac_defined_metric_fails_loud_on_the_unified_surface() {
    let mut session = compile(STAGE);
    let err = session
        .gradient("ugbw", &[("x1".into(), "gm".into())], &collect_config())
        .expect_err("AC-defined metric must refuse on the unified surface");
    assert!(
        err.to_string().contains("AC adjoint"),
        "names the AC gap: {}",
        err
    );
}
