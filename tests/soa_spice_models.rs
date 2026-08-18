//! SOA constraint blocks on the spice stdlib models (mos1/mos2/mos3,
//! diode `dio`, bjt): absent-by-default sentinel limits keep every check
//! vacuously satisfied — a run on default models reports the requires
//! satisfied with huge worst margins and never aborts; overriding a limit
//! at the instance site turns the check on and a violation fails loud
//! naming the constraint and the instance. The ngspice parity fixtures
//! over the same models are unaffected (margins never touch the solved
//! voltages).

use piperine::prelude::CheckPosture;
use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn config(posture: CheckPosture) -> SolverConfig {
    SolverConfig { checks: posture, ..Default::default() }
}

/// Default sentinels: the diode's SOA requires are satisfied with
/// enormous worst margins — the checks are vacuous until an override
/// names a real limit, and the run completes under `strict`.
#[test]
fn default_sentinels_keep_the_soa_checks_vacuouly_satisfied() {
    let design = piperine_lang::parse_and_elaborate(
        r#"`include "disciplines.vams"
`include "sources.va"
`include "diode.va"
module Top ();
    electrical gnd, a;
    vsrc #(.dc(0.7)) v1 (.p(a), .n(gnd));
    dio #(.is(1.0e-14), .n_e(1.0), .rs(0.0)) d1 (.p(a), .n(gnd));
endmodule"#,
        &source_map(),
    )
    .expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("compiles");
    let op = session.op(&config(CheckPosture::Strict), None).expect("strict completes — sentinels are vacuous");
    let vd = op.margins().get("d1.fwd_soa").expect("the SOA require is in the channel");
    assert!(vd.is_satisfied(), "1e99 sentinel: satisfied");
    assert!(vd.scalar() > 1e98, "an enormous (vacuous) margin, got {}", vd.scalar());
}

/// Overriding the limit turns the check on: `fv_max = 0.5` with the
/// diode's forward drop ≈ 0.63 V fails loud naming `fwd_soa` and `d1`.
#[test]
fn an_overridden_limit_fails_loud_on_violation() {
    let design = piperine_lang::parse_and_elaborate(
        r#"`include "disciplines.vams"
`include "sources.va"
`include "diode.va"
module Top ();
    electrical gnd, a;
    vsrc #(.dc(0.7)) v1 (.p(a), .n(gnd));
    dio #(.is(1.0e-14), .n_e(1.0), .rs(0.0), .fv_max(0.5)) d1 (.p(a), .n(gnd));
endmodule"#,
        &source_map(),
    )
    .expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("compiles");
    let err = session
        .op(&config(CheckPosture::Strict), None)
        .expect_err("the real limit is violated by the forward drop");
    let msg = err.to_string();
    assert!(msg.contains("fwd_soa"), "names the constraint: {msg}");
    assert!(msg.contains("d1"), "names the instance: {msg}");
}

/// The MOS models carry the same surface (all five voltage SOA requires
/// on every level) — proven once here on mos1; mos2/mos3 share the
/// identical block.
#[test]
fn the_mos_models_carry_the_gate_oxide_requires() {
    let design = piperine_lang::parse_and_elaborate(
        r#"`include "disciplines.vams"
`include "sources.va"
`include "mos.va"
module Top ();
    electrical gnd, d, g, b;
    vsrc #(.dc(2.0))  vd (.p(d), .n(gnd));
    vsrc #(.dc(1.5))  vg (.p(g), .n(gnd));
    vsrc #(.dc(-1.0)) vb (.p(b), .n(gnd));
    mos1 #(.l(2.0e-6), .w(10.0e-6), .vto(1.0), .kp(2.0e-5), .lambda(0.02), .gamma(0.5), .phi(0.65))
        m1 (.d(d), .g(g), .s(gnd), .b(b));
endmodule"#,
        &source_map(),
    )
    .expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("compiles");
    let op = session.op(&config(CheckPosture::Collect), None).expect("collect completes");
    let vgs = op.margins().get("m1.vgs_soa").expect("the gate require is in the channel");
    assert!(vgs.is_satisfied(), "sentinel default: vacuous");
    let vgb = op.margins().get("m1.vgb_soa").expect("the bulk require too");
    assert!(vgb.is_satisfied());
}

/// The BJT carries vbe/vce requires, and its vce check is directional
/// (`V(c,e) <= vce_max`, not abs) — an overridden vce_max below the
/// operating VCE fails loud.
#[test]
fn the_bjt_carries_directional_soa_requires() {
    let design = piperine_lang::parse_and_elaborate(
        r#"`include "disciplines.vams"
`include "sources.va"
`include "passives.va"
`include "bjt.va"
module Top ();
    electrical gnd, vcc, b, c;
    isrc #(.dc(2.0e-6))  ib (.p(gnd), .n(b));
    vsrc  #(.dc(10.0))   vc (.p(vcc), .n(gnd));
    res   #(.r(20.0e3))  rc (.p(vcc), .n(c));
    bjt   #(.is(1.0e-16), .bf(100.0), .vce_max(5.0))
        q1 (.c(c), .b(b), .e(gnd), .sub(gnd));
endmodule"#,
        &source_map(),
    )
    .expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("compiles");
    let mut session = Session::compile(&design, "Top").expect("compiles");
    let err = session
        .op(&config(CheckPosture::Strict), None)
        .expect_err("the forward-active VCE violates the overridden 5 V limit");
    let msg = err.to_string();
    assert!(msg.contains("vce_soa"), "names the constraint: {msg}");
    assert!(msg.contains("q1"), "names the instance: {msg}");
}
