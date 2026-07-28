//! Host-surface proof of SOA-on-model (dv-core T27, DVC-16): the shipped
//! spice models (`mos1`, `dio`, `bjt`) carry their own `constraint` blocks,
//! so a designer who never writes a `constraint` declaration gets SOA
//! enforcement for free the moment their model card sets a real limit.
//!
//! Two end-to-end contracts are proven here, both with **zero user-written
//! constraint code** — the requires come from `headers/spice/*.phdl`:
//!
//!   * **Inert by default (DVC-16 AC1, design §C5):** every unset limit is
//!     the documented `1e99` sentinel, so each `require` is trivially
//!     satisfied and the default `strict` posture completes cleanly on a
//!     stock card. The shipped gallery does not regress.
//!   * **Loud on violation (DVC-16 AC2/AC4):** a model card that sets a
//!     real limit, driven past it, fails loud in `strict` posture naming
//!     the device instance AND the constraint label — the same finding
//!     shape the in-model mos1 block (T26) and the diode/BJT blocks added
//!     here produce.
//!
//! This is the host-side half of DVC-16: cross-crate (lang headers →
//! codegen → solver → host API), driven through [`Session`].

use std::path::PathBuf;

use piperine::{Session, SolverConfig};
use piperine_lang::{Design, SourceMap};

/// The source map a real `piperine` invocation builds: the bundled stdlib
/// headers under the `piperine`/`spice` namespaces, so the test's
/// `use spice::…` resolves exactly as it does on the CLI.
fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn elaborate(src: &str) -> Design {
    piperine_lang::parse_and_elaborate(src, &headers_source_map())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// DVC-16 AC1 + design §C5: the **stock gallery** is inert. A mos1 with
/// the default model card (no SOA field overridden) elaborates and
/// simulates unchanged in default `strict` posture — the sentinel-default
/// limits keep every `require` trivially satisfied, so no finding fires
/// and the analysis completes. This is the acceptance test that an
/// invented limit did not slip in: any require that binds to a finite
/// default would trip on the gallery.
#[test]
fn stock_card_is_inert_under_default_strict_posture() {
    const STOCK: &str = "\
use piperine::disciplines;
use spice::sources;
use spice::passives;
use spice::mos;

mod Bias() {
    wire gnd : Electrical;
    wire vdd : Electrical;
    wire d   : Electrical;
    vd : vsrc (.p = vdd, .n = gnd) { .dc = 5.0 };
    r1 : res  (.p = vdd, .n = d)   { .r = 10.0e3 };
    m1 : mos1 (.d = d, .g = d, .s = gnd, .b = gnd) {
        .model = Mos1Model { .vto = 1.0, .kp = 2.0e-5 }
    };
}
";
    let design = elaborate(STOCK);
    let mut session = Session::compile(&design.fork(), "Bias").expect("compiles");
    // Default SolverConfig carries checks = CheckPosture::Strict (DVC-12).
    let op = session.op(&SolverConfig::default(), None).expect("strict op completes clean");
    assert!(op.requires_ok(), "no finding fires on the stock card");
    assert!(op.first_violation().is_none(), "no violation recorded");
    assert!(
        op.v("d".to_string()).unwrap_or_else(|e| panic!("v(d): {e}")) > 0.0,
        "the diode-connected NMOS sits above threshold"
    );
}

/// DVC-16 AC2: a model card that sets `vds_max`, driven past it in strict
/// posture, fails loud naming the device instance AND the constraint
/// label. Cross-crate proof (lang header → codegen kernel → solver → host
/// API) with zero user-written constraint code.
#[test]
fn mos1_card_with_vds_max_driven_past_aborts_strict_naming_instance_and_constraint() {
    // V(d) = 1.0 V, V(s) = 0 → Vds = 1.0 V > vds_max = 0.5 V.
    // Gate held above Vto so the channel conducts and the solver lands on
    // a real operating point (rather than a cutoff triviality).
    const VIOLATED: &str = "\
use piperine::disciplines;
use spice::sources;
use spice::mos;

mod Top() {
    wire gnd : Electrical;
    wire vg  : Electrical;
    wire vd  : Electrical;
    vg_src : vsrc (.p = vg, .n = gnd) { .dc = 2.0 };
    vd_src : vsrc (.p = vd, .n = gnd) { .dc = 1.0 };
    m1 : mos1 (.d = vd, .g = vg, .s = gnd, .b = gnd) {
        .model = Mos1Model { .vto = 1.0, .kp = 2.0e-5, .vds_max = 0.5 }
    };
}
";
    let design = elaborate(VIOLATED);
    let mut session = Session::compile(&design.fork(), "Top").expect("compiles");
    let err = session
        .op(&SolverConfig::default(), None)
        .expect_err("strict aborts when Vds > vds_max");
    let msg = err.to_string();
    assert!(msg.contains("vds_ox"), "names the violated constraint: {msg}");
    assert!(msg.contains("m1"), "names the device instance: {msg}");
}

/// DVC-16 AC1 + AC2 (collect posture): the same mos1 card driven past
/// `vds_max` does not abort under `collect`; the violation is recorded
/// on the result with full provenance — constraint label, instance path,
/// the offending margin value. Proves the in-model require reaches the
/// host result surface (T21/T23 channels).
#[test]
fn mos1_violation_under_collect_records_finding_with_provenance() {
    use piperine_api::prelude::CheckPosture;

    const VIOLATED: &str = "\
use piperine::disciplines;
use spice::sources;
use spice::mos;

mod Top() {
    wire gnd : Electrical;
    wire vg  : Electrical;
    wire vd  : Electrical;
    vg_src : vsrc (.p = vg, .n = gnd) { .dc = 2.0 };
    vd_src : vsrc (.p = vd, .n = gnd) { .dc = 1.0 };
    m1 : mos1 (.d = vd, .g = vg, .s = gnd, .b = gnd) {
        .model = Mos1Model { .vto = 1.0, .kp = 2.0e-5, .vds_max = 0.5 }
    };
}
";
    let design = elaborate(VIOLATED);
    let mut session = Session::compile(&design.fork(), "Top").expect("compiles");
    let config = SolverConfig { checks: CheckPosture::Collect, ..Default::default() };
    let op = session.op(&config, None).expect("collect completes");
    assert!(!op.requires_ok(), "the violation gates requires_ok");
    let entry = op.margins().get("vds_ox").expect("the in-model require is in the channel");
    assert_eq!(entry.instance_path, "m1", "the authored instance path");
    assert!(entry.scalar() < 0.0, "the margin is negative at the DC point: {}", entry.scalar());
    let finding = op.first_violation().expect("a finding is recorded");
    assert_eq!(finding.label, "vds_ox");
    assert_eq!(finding.instance_path, "m1");
}

/// DVC-16 AC4: the same pattern on the diode. A foundry card that sets
/// `bv_max` (reverse breakdown SOA cap) and a fixture driven past it fail
/// loud in strict posture naming the device instance and the constraint.
#[test]
fn diode_card_with_bv_max_driven_past_aborts_strict_naming_instance_and_constraint() {
    // Reverse-bias the diode to -2.0 V, past bv_max = 1.0 V.
    // I(p,n) at V=-2.0 V with the default ibv=1e-3 sits in the ngspice
    // breakdown region; the SOA require fires on the terminal voltage.
    const REVERSE: &str = "\
use piperine::disciplines;
use spice::sources;
use spice::diode;

mod Top() {
    wire gnd : Electrical;
    wire vp  : Electrical;
    wire vn  : Electrical;
    vp_src : vsrc (.p = vp, .n = gnd) { .dc = 0.0 };
    vn_src : vsrc (.p = vn, .n = gnd) { .dc = 2.0 };
    d1 : dio (.p = vp, .n = vn) {
        .model = DioModel { .bv_max = 1.0 }
    };
}
";
    let design = elaborate(REVERSE);
    let mut session = Session::compile(&design.fork(), "Top").expect("compiles");
    let err = session
        .op(&SolverConfig::default(), None)
        .expect_err("strict aborts when |Vr| > bv_max");
    let msg = err.to_string();
    assert!(msg.contains("vr"), "names the violated reverse-breakdown constraint: {msg}");
    assert!(msg.contains("d1"), "names the device instance: {msg}");
}

/// DVC-16 AC4: the same pattern on the BJT. A foundry card that sets
/// `vce_max` (V_CEO breakdown) and a fixture driven past it fail loud in
/// strict posture naming the device instance and the constraint.
#[test]
fn bjt_card_with_vce_max_driven_past_aborts_strict_naming_instance_and_constraint() {
    // Open-collector NPN with VCE forced to 6.0 V > vce_max = 5.0 V.
    // Base driven above emitter to keep the BJT in forward-active so the
    // operating point is well-defined; collector floats to the supply
    // through a resistor (the classic common-emitter bias).
    const OPEN_COLLECTOR: &str = "\
use piperine::disciplines;
use spice::sources;
use spice::passives;
use spice::bjt;

mod Top() {
    wire gnd : Electrical;
    wire vcc : Electrical;
    wire vb  : Electrical;
    wire vc  : Electrical;
    vcc_src : vsrc (.p = vcc, .n = gnd) { .dc = 6.0 };
    vb_src  : vsrc (.p = vb,  .n = gnd) { .dc = 0.7 };
    rc      : res  (.p = vcc, .n = vc)  { .r = 1.0e3 };
    q1 : bjt (.c = vc, .b = vb, .e = gnd, .sub = gnd) {
        .model = BjtModel { .vce_max = 5.0 }
    };
}
";
    let design = elaborate(OPEN_COLLECTOR);
    let mut session = Session::compile(&design.fork(), "Top").expect("compiles");
    let err = session
        .op(&SolverConfig::default(), None)
        .expect_err("strict aborts when V_CE > vce_max");
    let msg = err.to_string();
    assert!(msg.contains("vce"), "names the violated V_CEO constraint: {msg}");
    assert!(msg.contains("q1"), "names the device instance: {msg}");
}
