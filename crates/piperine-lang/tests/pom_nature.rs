//! LRM-COVERS: 3.6.1.1, 3.6.1.2, 3.6.1.3
//! Wave-2 (T69) re-reading, no new tests: this file's own header already
//! names each clause it proves — `base_nature_has_no_parent_and_is_not_
//! derived`/`derived_nature_records_parent_name` are §3.6.1.1 "Derived
//! natures" (base vs. derived, `parent` recording the parent-nature
//! relationship); `five_standard_attributes_are_named_fields_not_map_
//! entries`/`nature_with_only_required_base_attributes_is_representable`
//! are §3.6.1.2 "Attributes" (the five predefined attributes); `user_
//! defined_attributes_live_in_a_separate_map` is §3.6.1.3 "User-defined
//! attributes" verbatim (the LRM's own `maxval` example).
//!
//! VAMS T17 (VAMS-01) — POM `Nature` node, LRM §3.6.1.
//!
//! Asserts the structural contract of the POM `Nature` type:
//! - the five LRM-standard attributes (§3.6.1.2) are named fields, not map
//!   entries — a stringly-keyed typo must not fail silently (design.md §5.1);
//! - `parent` carries the derived-nature relationship (§3.6.1.1);
//! - user-defined attributes (§3.6.1.3) live in a separate map;
//! - the type implements `Named`/`Kinded` like every other POM node.
//!
//! The elaboration that lowers `piperine_lang::ast::NatureDecl` into this
//! type is T23; this target proves the type's contract on its own.

use std::collections::HashMap;

use piperine_lang::pom::{Kind, Kinded, Named, Nature};
use piperine_lang::value::Value;

fn voltage_base() -> Nature {
    // Mirrors the LRM §3.6.1 example: `nature voltage; ... endnature`.
    Nature {
        span: None,
        doc: None,
        name: "Voltage".into(),
        parent: None,
        abstol: Some(1e-6),
        access: Some("V".into()),
        units: Some("V".into()),
        idt_nature: None,
        ddt_nature: None,
        user_attrs: HashMap::new(),
    }
}

fn current_base() -> Nature {
    // Mirrors the LRM §3.6.1 example: `nature current; ... endnature`,
    // including the `idt_nature = charge` cross-reference.
    Nature {
        span: None,
        doc: None,
        name: "Current".into(),
        parent: None,
        abstol: Some(1e-6),
        access: Some("I".into()),
        units: Some("A".into()),
        idt_nature: Some("Charge".into()),
        ddt_nature: None,
        user_attrs: HashMap::new(),
    }
}

#[test]
fn named_returns_the_declared_nature_name() {
    let n = voltage_base();
    // Named::name — design.md §5.1, "implements Named/Kinded like its
    // siblings"; the trait method must return the declared identifier.
    assert_eq!(Named::name(&n), "Voltage");
    assert_eq!(n.name(), "Voltage");
}

#[test]
fn kinded_returns_the_nature_discriminant() {
    let n = voltage_base();
    // Kinded::kind — the Node capability axis; must be Kind::Nature, not
    // the legacy Discipline variant.
    assert_eq!(Kinded::kind(&n), Kind::Nature);
}

#[test]
fn base_nature_has_no_parent_and_is_not_derived() {
    let n = voltage_base();
    // §3.6.1.1: "If a nature is not derived from any other nature, it is
    // called a base nature." `parent` is None and `is_derived` is false.
    assert_eq!(n.parent(), None);
    assert!(!n.is_derived());
}

#[test]
fn derived_nature_records_parent_name() {
    // Mirrors the LRM §3.6.1.1 derived-nature example:
    //   nature ttl_net_curr : ttl_curr; endnature
    let derived = Nature {
        span: None,
        doc: None,
        name: "ttl_net_curr".into(),
        parent: Some("ttl_curr".into()),
        abstol: None,           // inherited from parent at elab time (T23)
        access: None,           // inherited; illegal to override (§3.6.1.2)
        units: None,            // inherited; illegal to override (§3.6.1.2)
        idt_nature: None,
        ddt_nature: None,
        user_attrs: HashMap::new(),
    };
    assert_eq!(derived.parent(), Some("ttl_curr"));
    assert!(derived.is_derived());
}

#[test]
fn five_standard_attributes_are_named_fields_not_map_entries() {
    // design.md §5.1 rationale: the five LRM-standard attributes drive
    // solver tolerance (abstol) and operator typing (idt_nature/
    // ddt_nature); a typo in a string-keyed map would fail silently.
    // Assert each is reachable as a typed field, not via a string lookup.
    let n = current_base();
    assert_eq!(n.abstol(), Some(1e-6));
    assert_eq!(n.access(), Some("I"));
    assert_eq!(n.units(), Some("A"));
    assert_eq!(n.idt_nature(), Some("Charge"));
    assert_eq!(n.ddt_nature(), None);

    // The voltage example carries only the four base-required attributes;
    // idt_nature defaults to self at elab time (T23), so it is None here.
    let v = voltage_base();
    assert_eq!(v.abstol(), Some(1e-6));
    assert_eq!(v.access(), Some("V"));
    assert_eq!(v.units(), Some("V"));
    assert_eq!(v.idt_nature(), None);
    assert_eq!(v.ddt_nature(), None);
}

#[test]
fn user_defined_attributes_live_in_a_separate_map() {
    // §3.6.1.3: "user-defined attributes can be defined in a nature" —
    // the LRM's example uses `maxval = 12.3` on a derived nature. The POM
    // must keep these out of the five standard fields (an unknown name is
    // never silently dropped into a catch-all on the standard side).
    let mut user_attrs = HashMap::new();
    user_attrs.insert("maxval".into(), Value::Real(12.3));
    user_attrs.insert("minval".into(), Value::Real(-12.3));

    let n = Nature {
        span: None,
        doc: None,
        name: "ttl_curr".into(),
        parent: None,
        abstol: Some(1e-3),
        access: Some("I".into()),
        units: Some("A".into()),
        idt_nature: None,
        ddt_nature: None,
        user_attrs,
    };

    let attrs = n.user_attrs();
    assert_eq!(attrs.len(), 2);
    assert!(attrs.contains_key("maxval"));
    assert!(attrs.contains_key("minval"));
    // Round-trip: read-back values match what was stored.
    assert_eq!(attrs.get("maxval"), Some(&Value::Real(12.3)));
    assert_eq!(attrs.get("minval"), Some(&Value::Real(-12.3)));
}

#[test]
fn nature_with_only_required_base_attributes_is_representable() {
    // §3.6.1.2: abstol/access/units are required for base natures. The
    // POM does not enforce this (the elaborator does, T23); the type must
    // still represent the minimal LRM example cleanly.
    let minimal = Nature {
        span: None,
        doc: None,
        name: "Voltage".into(),
        parent: None,
        abstol: Some(1e-6),
        access: Some("V".into()),
        units: Some("V".into()),
        idt_nature: None,
        ddt_nature: None,
        user_attrs: HashMap::new(),
    };
    assert_eq!(minimal.abstol(), Some(1e-6));
    assert_eq!(minimal.access(), Some("V"));
    assert_eq!(minimal.units(), Some("V"));
    assert!(minimal.user_attrs().is_empty());
}

#[test]
fn nature_preserves_doc_comment() {
    // MD-25 + LSP-07: every POM node preserves its authored `///` doc.
    let n = Nature {
        doc: Some("The electrical potential nature.".into()),
        ..voltage_base()
    };
    assert_eq!(n.doc(), Some("The electrical potential nature."));
}

#[test]
fn nature_serde_round_trips_through_json() {
    // Design §5.1 carries serde derives; the LRM example must round-trip
    // without loss. `span` is serde-skipped (not a structural field).
    let mut user_attrs = HashMap::new();
    user_attrs.insert("maxval".into(), Value::Real(99.0));
    let original = Nature {
        span: None,
        doc: Some("doc".into()),
        name: "Current".into(),
        parent: Some("ttl_curr".into()),
        abstol: Some(1e-3),
        access: Some("I".into()),
        units: Some("A".into()),
        idt_nature: Some("Charge".into()),
        ddt_nature: Some("Current".into()),
        user_attrs,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: Nature = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.name(), "Current");
    assert_eq!(restored.parent(), Some("ttl_curr"));
    assert_eq!(restored.abstol(), Some(1e-3));
    assert_eq!(restored.access(), Some("I"));
    assert_eq!(restored.units(), Some("A"));
    assert_eq!(restored.idt_nature(), Some("Charge"));
    assert_eq!(restored.ddt_nature(), Some("Current"));
    assert_eq!(restored.doc(), Some("doc"));
    assert_eq!(restored.user_attrs().get("maxval"), Some(&Value::Real(99.0)));
}
