//! T52 (VAMS-20) — `InsertConnectModules` (LRM ch. 7) elaborates cleanly
//! alongside real `.vams` source. The pass's own selection-algorithm and
//! MD-25 (derived-artifact) unit tests live with the pass itself
//! (`crates/piperine-lang/src/elab/lower/connect.rs`, `#[cfg(test)]`,
//! per this feature's established pattern — see `flatten.rs`'s own tests
//! for precedent); since T8 (LRMC-C7-01) those tests run a genuine
//! cross-discipline connection through the real, full
//! `parse_and_elaborate` pipeline too — `ResolveDisciplines` no longer
//! unconditionally vetoes a connectrules-eligible mismatch before this
//! pass gets a chance (`eligible_bridge`, shared by both passes). This
//! target proves the *pipeline-level* claims: `connectrules`/
//! `connectmodule` declarations parse and elaborate without disturbing an
//! ordinary design, and the pass is correctly positioned in `PASSES`
//! (after `ResolveDisciplines`, before `FlattenHierarchy`).

use piperine_lang::{parse_and_elaborate, SourceMap};

/// A source declaring `connectrules`/`connectmodule` (LRM ch. 7 surface)
/// alongside an ordinary same-discipline design — proves the new
/// registration (`register.rs`) and pass (`InsertConnectModules`) don't
/// disturb elaboration of a design that never needs a bridge.
const WITH_CONNECT_INFRASTRUCTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect logic, electrical;
    endconnectrules

    connectmodule bridge (inout logic a, inout electrical b);
    endmodule

    module leaf (inout electrical p, inout electrical n);
        analog V(p, n) <+ 0;
    endmodule

    module top ();
        electrical vin, vout;
        leaf l1 (vin, vout);
    endmodule
"#;

#[test]
fn connectrules_and_connectmodule_declarations_elaborate_without_disturbing_the_design() {
    let design = parse_and_elaborate(WITH_CONNECT_INFRASTRUCTURE, &SourceMap::dummy())
        .expect("connectrules/connectmodule declarations coexist with an ordinary design");

    // The connect infrastructure elaborates as an ordinary module (LRM
    // ch. 7: a connectmodule is a module) — reachable exactly like `top`
    // and `leaf`, per this crate's uniform elaboration of every
    // `ModuleKind`.
    assert!(design.module("bridge").is_some(), "the declared connectmodule elaborates as a module");
    assert!(design.module("leaf").is_some());
    let top = design.module("top").expect("top elaborates");
    assert_eq!(top.instances.len(), 1, "no bridge was auto-inserted — `leaf`'s connection never crosses disciplines");
    assert_eq!(top.instances[0].name(), "l1");
}

/// MD-25 (derived-artifact rule), proven at the pipeline level: since no
/// bridge was needed for this design, `Design::flat_modules` reflects
/// exactly the authored instance count too — `InsertConnectModules`
/// contributed nothing extra to flatten.
#[test]
fn no_crossing_means_flat_modules_matches_the_authored_instance_count() {
    let design = parse_and_elaborate(WITH_CONNECT_INFRASTRUCTURE, &SourceMap::dummy()).expect("elaborates");
    let flat_top = design.flat_module("top").expect("`top` has a flattened (or fallback authored) form");
    assert_eq!(flat_top.instances.len(), 1, "no connect bridge was spliced into the flattened graph");
}
