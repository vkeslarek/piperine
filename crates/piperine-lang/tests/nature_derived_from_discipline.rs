//! LRM-COVERS: 3.6.2.6
//!
//! lrm-conformance-wave3 T24: `nature n : discipline.slot;` derives from
//! the nature currently bound to that discipline's `potential`/`flow`
//! slot (LRM §3.6.2.6's own worked example, continuing T23's §3.6.2.5
//! `flow.abstol = 10u;` fixture: "`nature ttl_net_curr : ttl.flow; //
//! abstol = 10u as modified in ttl`") — attributes are inherited from the
//! *effective* bound nature (including a discipline-level override), and
//! the derived nature may itself override an inherited attribute or add
//! a new one. §3.11.1 compatibility follows the resulting derivation
//! family, exactly as it already does for a plain `nature : parent`
//! derivation (`vams_nature_compatibility.rs`).

use piperine_lang::value::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const BASE: &str = r#"
    nature ttl_volt;
        units = "V";
        access = V;
        abstol = 100u;
    endnature
    nature ttl_curr;
        units = "A";
        access = I;
        abstol = 1e-3;
    endnature
    discipline ttl;
        potential ttl_volt;
        flow ttl_curr;
        flow.abstol = 1e-9;
    enddiscipline
"#;

/// AC1 (T24 Done-when #1): a nature derived from `ttl.flow` with no
/// attributes of its own inherits the *effective* (T23-overridden)
/// abstol, 1e-9 — not `ttl_curr`'s own 1e-3.
#[test]
fn derived_nature_inherits_the_disciplines_overridden_attribute() {
    let src = format!(
        "{BASE}
        nature ttl_net_curr : ttl.flow;
        endnature
        module m (inout ttl p);
        endmodule
        "
    );
    let d = elab(&src);
    let n = d.nature("ttl_net_curr").expect("ttl_net_curr elaborated");
    assert_eq!(n.abstol(), Some(1e-9), "inherits the discipline-overridden abstol, not ttl_curr's own 1e-3");
    assert_eq!(n.access(), Some("I"), "access is inherited unchanged (illegal to override, §3.6.1.2)");
    assert_eq!(n.units(), Some("A"));
}

/// AC2 (T24 Done-when #2): a local override on the derived nature wins
/// over the inherited value, and a brand-new attribute is added — the
/// LRM's own combined example (§3.6.2.6):
/// `nature ttl_net_volt : ttl.potential; abstol = 1m; maxval = 12.3;`
#[test]
fn local_override_and_new_attribute_both_work() {
    let src = format!(
        "{BASE}
        nature ttl_net_volt : ttl.potential;
            abstol = 1m;
            maxval = 12.3;
        endnature
        module m (inout ttl p);
        endmodule
        "
    );
    let d = elab(&src);
    let n = d.nature("ttl_net_volt").expect("ttl_net_volt elaborated");
    assert_eq!(n.abstol(), Some(1e-3), "local override (1m) wins over the inherited ttl_volt abstol (100u)");
    assert_eq!(n.units(), Some("V"), "units still inherited from ttl_volt (illegal to override)");
    assert_eq!(n.user_attrs().get("maxval"), Some(&Value::Real(12.3)), "new attribute lands in user_attrs");
}

/// AC3 (T24 Done-when #3): §3.11.1 compatibility follows the derivation
/// family this task creates. `ttl_alt`'s flow is bound to
/// `ttl_net_curr` (derived from `ttl.flow`, i.e. from `ttl_curr`); `ttl`'s
/// own flow is `ttl_curr` directly. The Base Nature Rule makes the two
/// disciplines compatible even though their flow natures have different
/// names — mirrors `vams_nature_compatibility.rs`'s own assertion style
/// for a plain `nature : parent` derivation.
#[test]
fn compatibility_follows_the_discipline_derivation_family() {
    let src = format!(
        "{BASE}
        nature ttl_net_curr : ttl.flow;
        endnature
        discipline ttl_alt;
            potential ttl_volt;
            flow ttl_net_curr;
        enddiscipline
        module drv_ttl (inout ttl p);
        endmodule
        module drv_alt (inout ttl_alt p);
        endmodule
        module top ();
            drv_ttl a(shared);
            drv_alt b(shared);
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated — disciplines resolved as compatible");
    assert_eq!(top.instances.len(), 2, "both instances present, unmodified by compatibility resolution");
}

/// Edge case (spec.md "Edge Cases"): deriving from a discipline slot that
/// has no binding at all fails loud citing §3.6.2.6.
#[test]
fn deriving_from_an_unbound_discipline_slot_fails_loud() {
    let msg = elab_err(
        r#"
        discipline bare;
        enddiscipline
        nature n : bare.flow;
        endnature
        module m (inout bare p);
        endmodule
        "#,
    );
    assert!(msg.contains("bare"), "error must name the discipline, got: {msg}");
    assert!(msg.contains("3.6.2.6"), "error must cite §3.6.2.6, got: {msg}");
}
