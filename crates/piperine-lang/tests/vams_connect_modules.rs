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

// ── connect_mode (§7.8.3, LRMC-C7-03, T32) ───────────────────────────────
//
// LRM §7.8.3.1/§7.8.3.2's own worked examples (Figure 7-9/7-10, Example 1-4)
// name project-specific TTL/`d2a`/`cmosA2d` connect modules whose bodies
// this crate has no stdlib copy of, and Example 3/4's own `#(.r(30k)
// input ...` text is missing its closing paren before `input` in the LRM
// source itself — so these fixtures restate the same normative rule
// (§7.8.3: "merged ... group all ports ... one connector module" /
// "split ... one connect module for each port") on a minimal two-leaf
// design instead of transcribing the LRM's own unparseable example text.

/// Two digital leaves share one net crossing into `electrical` — under
/// the LRM §7.8.3 *default* (`merged`, no `connect_mode` given at all,
/// same as the legacy bare-pair shell), exactly one bridge instance is
/// shared for both ports on that net (§7.8.3.1).
const MERGED_DEFAULT: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect logic, electrical;
    endconnectrules

    connectmodule bridge (inout logic a, inout electrical b);
    endmodule

    module digital_leaf (inout logic p);
    endmodule

    module top ();
        electrical n;
        digital_leaf d1 (n);
        digital_leaf d2 (n);
    endmodule
"#;

#[test]
fn merged_default_shares_one_connect_module_across_both_ports_on_the_net() {
    let design = parse_and_elaborate(MERGED_DEFAULT, &SourceMap::dummy()).expect("elaborates");
    let top_flat = design.flat_module("top").expect("top's flattened form");
    let bridges: Vec<_> = top_flat.instances.iter().filter(|i| i.module == "bridge").collect();
    assert_eq!(bridges.len(), 1, "merged (LRM default): one shared connect module for the whole net, got {bridges:?}");
}

/// Same design, `connect bridge merged logic, electrical;` — the explicit
/// `merged` keyword produces the identical single-instance result.
const MERGED_EXPLICIT: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect bridge merged logic, electrical;
    endconnectrules

    connectmodule bridge (inout logic a, inout electrical b);
    endmodule

    module digital_leaf (inout logic p);
    endmodule

    module top ();
        electrical n;
        digital_leaf d1 (n);
        digital_leaf d2 (n);
    endmodule
"#;

#[test]
fn explicit_merged_keyword_matches_the_default_behavior() {
    let design = parse_and_elaborate(MERGED_EXPLICIT, &SourceMap::dummy()).expect("elaborates");
    let top_flat = design.flat_module("top").expect("top's flattened form");
    let bridges: Vec<_> = top_flat.instances.iter().filter(|i| i.module == "bridge").collect();
    assert_eq!(bridges.len(), 1);
}

/// `connect bridge split logic, electrical;` — §7.8.3.2: the net is
/// segmented, one connect-module instance per port.
const SPLIT_EXPLICIT: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect bridge split logic, electrical;
    endconnectrules

    connectmodule bridge (inout logic a, inout electrical b);
    endmodule

    module digital_leaf (inout logic p);
    endmodule

    module top ();
        electrical n;
        digital_leaf d1 (n);
        digital_leaf d2 (n);
    endmodule
"#;

#[test]
fn split_segments_one_connect_module_instance_per_port() {
    let design = parse_and_elaborate(SPLIT_EXPLICIT, &SourceMap::dummy()).expect("elaborates");
    let top_flat = design.flat_module("top").expect("top's flattened form");
    let bridges: Vec<_> = top_flat.instances.iter().filter(|i| i.module == "bridge").collect();
    assert_eq!(bridges.len(), 2, "split (§7.8.3.2): one connect module per port, got {bridges:?}");
}

/// §7.7.3: the parameter-passing attribute (`#(.name(value))` on a
/// `connect` statement) reaches the auto-inserted instance.
const WITH_PARAMS: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect bridge merged #(.tt(3.5)) logic, electrical;
    endconnectrules

    connectmodule bridge (inout logic a, inout electrical b);
        parameter real tt = 1.0;
    endmodule

    module digital_leaf (inout logic p);
    endmodule

    module top ();
        electrical n;
        digital_leaf d1 (n);
    endmodule
"#;

#[test]
fn parameter_passing_attribute_reaches_the_inserted_instance() {
    let design = parse_and_elaborate(WITH_PARAMS, &SourceMap::dummy()).expect("elaborates");
    let top_flat = design.flat_module("top").expect("top's flattened form");
    let bridge = top_flat.instances.iter().find(|i| i.module == "bridge").expect("a bridge was inserted");
    assert_eq!(bridge.params.len(), 1, "params: {:?}", bridge.params);
    assert_eq!(bridge.params[0].0, "tt");
}
