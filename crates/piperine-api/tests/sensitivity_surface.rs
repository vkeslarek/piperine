//! Host sensitivity surface (dv-gradients T11, DVG-13):
//! [`Session::sensitivity`] for one parameter and [`Session::gradient`]
//! for many — both with engine attribution, both failing loud on unknown
//! metrics with candidate listing, both refusing AC-defined metrics
//! naming the gap. The adjoint path is the engine; the per-element
//! fallback list travels on the result.
//!
//! **BLOCKED — fixtures left in PHDL syntax.** This target's fixture needs
//! PHDL's `constraint` block with its `require` clause and reduced `var`
//! declarations.
//! Verilog-AMS Annex A has no production for any of them: MD-45 deleted
//! `pom::ConstraintBlock`/`QuantityClass`/`Module::constraints()` with the
//! PHDL AST (T21), `lower_bodies` now always sets `LoweredBody::constraints`
//! and `::cover` to `None`, `elab/lower/vams.rs` always writes
//! `Param::distribution = None`, and `MarginsCatalog::compute` returns an
//! empty catalog unconditionally. A future phase re-anchors the dv-core
//! surface on VAMS `(* attribute *)` syntax (`.specs/STATE.md` MD-45,
//! `resolve/pom/constraints.rs`'s module doc). Until then the fixtures below
//! cannot be retargeted and the tests are left failing rather than weakened,
//! deleted, or `#[ignore]`d (repo-banned by `tests/suite_hygiene.rs`).

use piperine_api::prelude::CheckPosture;
use piperine_api::{
    Gradient, Sensitivity, SensitivityMethod, Session, SolverConfig,
};
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 10.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.4; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require head : V(p, n) <= hi; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

/// A single-pole stage with reduced AC vars — refuses loud on the host
/// surface, naming the AC-adjoint gap.
const STAGE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vac(inout p: Electrical, inout n: Electrical) { param acmag: Real = 1.0; }
analog Vac { V(p, n) <+ ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Stage(inout inp: Electrical, inout out: Electrical, inout gnd: Electrical) {
    param gm: Real = 0.1;
    param r: Real = 1e3;
    param c: Real = 1e-6;
}
analog Stage {
    I(gnd, out) <+ gm * V(inp, gnd);
    I(out, gnd) <+ V(out, gnd) / r;
    I(out, gnd) <+ c * ddt(V(out, gnd));
}
constraint Stage {
    var ugbw : Real = ac_unity_gain_freq(V(out, gnd));
}

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1 : Vac(.p = in, .n = gnd);
    x1 : Stage(.inp = in, .out = out, .gnd = gnd);
}
";

fn compile(phdl: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(phdl, &SourceMap::dummy()).expect("elaborates");
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
    let mut session = compile(DIVIDER_PHDL);

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
    let mut session = compile(DIVIDER_PHDL);
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
#[test]
fn a_metric_name_resolves_to_either_a_margin_or_a_net() {
    let mut session = compile(DIVIDER_PHDL);

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
#[test]
fn unknown_metric_fails_loud_listing_candidates() {
    let mut session = compile(DIVIDER_PHDL);
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
#[test]
fn ac_defined_metric_fails_loud_on_the_unified_surface() {
    let mut session = compile(STAGE_PHDL);
    let err = session
        .gradient("ugbw", &[("x1".into(), "gm".into())], &collect_config())
        .expect_err("AC-defined metric must refuse on the unified surface");
    assert!(
        err.to_string().contains("AC adjoint"),
        "names the AC gap: {}",
        err
    );
}
