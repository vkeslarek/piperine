//! LRM-COVERS: 6.4
//! LRMC-C5-07 (`.specs/features/lrm-conformance-closure/`, T29): an
//! instance naming a `paramset` (§6.4) resolves to the paramset's
//! associated module with its pre-computed parameter bindings actually
//! applied — §6.4's own SPICE-model-card-conversion worked example.
//!
//! **Fixture adaptation (disclosed):** the LRM's own concrete syntax is
//! `paramset name module_name;` (no parens around the module name, Annex
//! A.1.9's `paramset_declaration`) — this crate's *existing*, pre-dating
//! this task, already-committed-and-tested grammar
//! (`crates/piperine-lang/tests/items_remaining.rs::
//! paramset_declares_module_target_and_params`) instead requires
//! `paramset name (module_name);`, a real, separate, undisclosed-until-now
//! FORM divergence from Annex A. Fixing it is a grammar change with its
//! own pre-existing test to update, outside LRMC-C5-07's own declared
//! scope (`elab/lower/{register.rs,vams.rs}` only) — this file's fixtures
//! use the existing (parenthesized) form so as not to touch
//! `grammar/item.rs` under this task; the values, parameter names, and
//! `.field = expr;` bindings are otherwise §6.4's own example verbatim.

use piperine_lang::pom::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// §6.4's own SPICE-model-card example: `nmos3` is "a behavioral module
/// that contains the same equations as the SPICE primitive"; `nch` is the
/// paramset converting the `.model nch nmos (...)` card.
const FIXTURE: &str = r#"
    module nmos3 (inout electrical d, inout electrical g, inout electrical s, inout electrical b);
        parameter real l = 1e-6;
        parameter real w = 1e-6;
        parameter real ad = 0.0;
        parameter real as = 0.0;
        parameter real kp = 0.0;
        parameter real tox = 0.0;
        parameter real u0 = 0.0;
        parameter real nsub = 0.0;
        parameter real vmax = 0.0;
        parameter real tpg = 0.0;
        parameter real nfs = 0.0;
    endmodule

    paramset nch (nmos3);
        parameter real l = 1e-6 from [0.25e-6:inf);
        parameter real w = 1e-6 from [0.2e-6:inf);
        .l = l;
        .w = w;
        .ad = w * 0.5e-6;
        .as = w * 0.5e-6;
        .kp = 5e-5;
        .tox = 3e-8;
        .u0 = 650;
        .nsub = 1.3e17;
        .vmax = 0;
        .tpg = 1;
        .nfs = 0.8e12;
    endparamset

    module top ();
        electrical d1, g, s0, b0;
        nch #(.l(1e-6), .w(10e-6)) m1 (d1, g, s0, b0);
    endmodule
"#;

#[test]
fn paramset_instance_resolves_to_the_associated_module_with_bound_params() {
    let d = elab(FIXTURE);
    let top = d.module("top").expect("top elaborated");
    let m1 = top.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    // §6.4's own indirection: the *instantiated* module is `nmos3` (the
    // paramset's associated module), not `nch` (the paramset's own name).
    assert_eq!(m1.module, "nmos3");
    let get = |name: &str| m1.params.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    assert_eq!(get("l"), Some(Value::Real(1e-6)), "l = l, with the instance override l=1e-6");
    assert_eq!(get("w"), Some(Value::Real(10e-6)), "w = w, with the instance override w=10e-6");
    assert_eq!(get("ad"), Some(Value::Real(10e-6 * 0.5e-6)), "ad = w * 0.5e-6, using the overridden w");
    assert_eq!(get("as"), Some(Value::Real(10e-6 * 0.5e-6)), "as = w * 0.5e-6, using the overridden w");
    assert_eq!(get("kp"), Some(Value::Real(5e-5)));
    assert_eq!(get("tox"), Some(Value::Real(3e-8)));
    assert_eq!(get("u0"), Some(Value::Int(650)), "`.u0 = 650;` is an integer literal, per the LRM's own text");
    assert_eq!(get("nsub"), Some(Value::Real(1.3e17)));
}

#[test]
fn paramset_instance_uses_the_paramsets_own_defaults_when_not_overridden() {
    let d = elab(&FIXTURE.replace("nch #(.l(1e-6), .w(10e-6)) m1", "nch m1"));
    let top = d.module("top").expect("top elaborated");
    let m1 = top.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    let get = |name: &str| m1.params.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    // Paramset's own default: l = 1e-6, w = 1e-6 (no instance override).
    assert_eq!(get("l"), Some(Value::Real(1e-6)));
    assert_eq!(get("w"), Some(Value::Real(1e-6)));
    assert_eq!(get("ad"), Some(Value::Real(1e-6 * 0.5e-6)));
}

#[test]
fn overriding_a_parameter_the_paramset_does_not_declare_fails_loud() {
    // §6.4: "an instance of the paramset that attempts to override any of
    // the other parameters of the underlying module nmos3 would generate
    // an error" — `kp` is a real nmos3 parameter, but not one of the
    // paramset's own two (`l`, `w`).
    let src = FIXTURE.replace("nch #(.l(1e-6), .w(10e-6)) m1", "nch #(.kp(1.0)) m1");
    let msg = elab_err(&src);
    assert!(msg.contains("kp"), "names the disallowed override: {msg}");
}

#[test]
fn paramset_instance_stays_walkable_instance_to_submodule_to_sub_instances() {
    // MD-25: the instance -> submodule -> sub-instance tree must stay
    // navigable exactly as elaboration produces it -- a paramset instance
    // is a real Instance whose `module` names a real, walkable submodule
    // (not a synthetic/opaque handle), so a consumer reading `top`'s
    // instances back out finds `nmos3` in `Design::modules` and can
    // recurse into *its* own instances (empty here, but reachable).
    let d = elab(FIXTURE);
    let top = d.module("top").expect("top elaborated");
    let m1 = top.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    let submodule = d.module(&m1.module).expect("m1.module names a walkable submodule in Design::modules");
    assert_eq!(submodule.name(), "nmos3");
    assert!(submodule.instances.is_empty(), "nmos3 itself declares no sub-instances");
}

#[test]
fn a_paramset_statement_naming_an_unknown_module_parameter_fails_loud() {
    let src = r#"
        module leaf (inout electrical p);
            parameter real real_param = 1.0;
        endmodule

        paramset ps (leaf);
            .nonexistent_param = 1.0;
        endparamset

        module top ();
            electrical p;
            ps m1 (p);
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("nonexistent_param"), "names the offending .field: {msg}");
}
