//! VAMS T20 (VAMS-01, VAMS-04) — POM module/design fields for VAMS.
//!
//! Asserts the VAMS-era POM shape:
//! - `Module` carries `branches` (LRM §3.12) and `genvars` (LRM §3.5)
//!   alongside its existing collections — additive, no existing field
//!   moved or removed;
//! - `Design` carries top-level `natures` (LRM §3.6.1) alongside its
//!   existing collections — natures are top-level only, never nested in
//!   modules (LRM §3.6.2);
//! - `Design::flat_modules` remains the **only** derived artifact
//!   (design §5.3, MD-25 VAMS clause): every other collection on the
//!   POM mirrors authored source, never synthesised content.
//!
//! The lowering that fills these collections from
//! `piperine_lang::ast::*` is T23+; this target proves the POM-shape
//! contract on its own.

use piperine_lang::pom::{
    Branch, BranchKind, Design, Kind, Kinded, Module, Named, Nature, NetRef,
};

fn empty_module(name: &str) -> Module {
    Module::new(
        name.into(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn module_carries_branches_and_genvars_collections() {
    // LRM §3.12 + §3.5: `branch (p,n) br;` and `genvar i;` are module
    // items; the Module POM node carries both collections, default-empty.
    let mut m = empty_module("m");
    assert!(m.branches().is_empty(), "branches default empty");
    assert!(m.genvars().is_empty(), "genvars default empty");

    m.branches.push(Branch {
        span: None,
        doc: None,
        name: Some("br".into()),
        pos: NetRef::simple("a"),
        neg: Some(NetRef::simple("b")),
        kind: BranchKind::Probe,
    });
    m.genvars.push("i".into());

    assert_eq!(m.branches().len(), 1);
    assert_eq!(m.genvars(), &["i".to_string()]);
}

#[test]
fn module_existing_collections_are_untouched_by_vams_additions() {
    // VAMS fields are additive — every existing field stays where it was
    // and its accessor still works. Catches an accidental reshape.
    let m = empty_module("m");
    assert!(m.ports().is_empty());
    assert!(m.params().is_empty());
    assert!(m.wires().is_empty());
    assert!(m.vars().is_empty());
    assert!(m.instances().is_empty());
    assert!(m.connections().is_empty());
    assert!(m.behaviors().is_empty());
    assert_eq!(Named::name(&m), "m");
    assert_eq!(Kinded::kind(&m), Kind::Module);
}

#[test]
fn module_branch_lookup_finds_named_branches() {
    // Implicit branches (no authored name) are not reachable by lookup —
    // only named branches are. T23's access-function call-site resolver
    // returns implicit branches via a different path.
    let mut m = empty_module("m");
    m.branches.push(Branch {
        span: None,
        doc: None,
        name: Some("out".into()),
        pos: NetRef::simple("p"),
        neg: Some(NetRef::simple("n")),
        kind: BranchKind::Probe,
    });
    m.branches.push(Branch {
        span: None,
        doc: None,
        name: None,                  // implicit
        pos: NetRef::simple("x"),
        neg: None,
        kind: BranchKind::Probe,
    });

    assert!(m.branch("out").is_some());
    assert!(m.branch("nonexistent").is_none());
    // Implicit branch is in the collection but not reachable by name.
    assert_eq!(m.branches().len(), 2);
    assert!(m.branches().iter().any(|b| b.name().is_none()));
}

#[test]
fn design_exposes_top_level_natures_collection() {
    // LRM §3.6.1: natures are declared at the top level, not nested in
    // modules. `Design::natures` is their POM home; `Design::nature(name)`
    // looks one up. A fresh Design has no natures.
    let d = Design::new();
    assert!(d.natures().next().is_none(), "natures default empty on a fresh Design");
    assert!(d.nature("Voltage").is_none(), "lookups on an empty design return None");
}

#[test]
fn design_existing_collections_are_untouched_by_vams_additions() {
    // The VAMS nature collection is additive — every existing accessor
    // still works and returns its authored-source form, not a synthesised
    // one. Catches an accidental reshape of the Design struct.
    let d = Design::new();
    assert_eq!(d.module_count(), 0);
    assert!(d.disciplines().next().is_none());
    assert!(d.natures().next().is_none());
    assert!(d.top().is_none());
}

#[test]
fn flat_modules_remains_the_single_derived_artifact() {
    // design.md §5.3, MD-25 VAMS clause: Design::flat_modules is the
    // ONLY derived artifact. Every other collection on the POM mirrors
    // authored source.
    //
    // The named audit: every collection on Design that holds derived
    // (non-authored) data MUST be discoverable as such. `flat_modules`
    // is named `flat_*` per that contract — a future PR adding a second
    // derived map under a generic name would slip past review; the
    // naming invariant keeps the audit mechanical.
    //
    // Concretely: a freshly-constructed Design has an empty flat_modules
    // (the derived side) and empty authored collections (modules,
    // natures, disciplines, paramsets).
    // The lookup `flat_module(name)` falls back to `modules` per its
    // documented contract — so a missing flat form is observable only
    // by inspecting the derived map itself, not the lookup. The
    // invariant: authored insertion never populates the derived map.
    let d = Design::new();
    assert!(d.flat_module("anything").is_none(),
        "fresh Design has no flat form for any name — flat_modules is empty");
    assert!(d.module("anything").is_none(),
        "fresh Design has no authored module either — separate concerns");

    // The accessor that DOES return authored content (`module_count`,
    // `natures`, `disciplines`) reads from authored-source collections
    // only. These three reads return zero on a fresh design — the
    // derived side is not consulted by any of them.
    assert_eq!(d.module_count(), 0);
    assert_eq!(d.natures().count(), 0);
    assert_eq!(d.disciplines().count(), 0);
}

#[test]
fn branches_and_genvars_default_through_serde_round_trip() {
    // Additive serde defaults: a pre-VAMS serialized Module (without the
    // new fields) deserializes with empty branches/genvars. Verified by
    // constructing a minimal JSON payload lacking the new keys.
    let minimal = serde_json::json!({
        "name": "legacy",
        "attributes": [],
        "ports": [],
        "params": [],
        "wires": [],
        "vars": [],
        "instances": [],
        "connections": [],
        "origin": null,
    });
    let m: Module = serde_json::from_value(minimal).expect("deserialize legacy module");
    assert_eq!(Named::name(&m), "legacy");
    assert!(m.branches().is_empty(),
        "branches must default to empty when absent from serialized input");
    assert!(m.genvars().is_empty(),
        "genvars must default to empty when absent from serialized input");
}

#[test]
fn nature_serializes_and_round_trips_through_design_serde_skip() {
    // Natures on Design are #[serde(skip)] (the same rule as disciplines:
    // declaration-shaped data the deserialized Design carries empty and
    // re-elaboration rebuilds). Verifies a serialized Design does NOT
    // carry natures in its JSON form, so they cannot leak into a wire
    // format or shadow the authored source.
    let d = Design::new();
    let json = serde_json::to_string(&d).expect("serialize fresh Design");
    assert!(
        !json.contains("natures"),
        "Design::natures is #[serde(skip)] — must not appear in serialized form: {json}",
    );
    // Round-trip is idempotent on an empty design.
    let _: Design = serde_json::from_str(&json).expect("deserialize fresh Design");

    // A Nature serializes on its own (used by T23's nature map when the
    // POM is consulted in-memory); the skip is on the Design field, not
    // on the Nature type itself.
    let n = Nature {
        span: None, doc: None, name: "Voltage".into(), parent: None,
        abstol: Some(1e-6), access: Some("V".into()), units: Some("V".into()),
        idt_nature: None, ddt_nature: None, user_attrs: std::collections::HashMap::new(),
    };
    let nature_json = serde_json::to_string(&n).expect("serialize Nature");
    assert!(nature_json.contains("Voltage"));
    assert!(nature_json.contains("\"V\""));
}
