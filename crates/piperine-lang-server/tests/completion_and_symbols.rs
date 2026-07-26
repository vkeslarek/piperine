//! Completion and document symbols: schema-name completion after `@` (and its
//! suppression off attribute position), plus the outline entries `extract_symbols`
//! produces.

mod common;
use common::*;

/// `@rf|` completes to `rfport` — the built-in `rfport` schema is always
/// in-scope (registered unconditionally in `ElabContext::new()`).
///
/// Types `@rf` after a valid document, simulating the moment completion is
/// triggered mid-edit: `doc.source` is updated but not re-analyzed, so
/// `doc.ctx` still holds the last successful elaboration's registries — the
/// pre-existing stale-but-valid resilience (`state.rs`) completion rides.
#[test]
fn schema_completion_after_at_sign_offers_in_scope_schema_names() {
    let mut doc = DocumentState::new("mod Top() {}\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some(), "must elaborate cleanly: {:?}", doc.errors);

    doc.source = "mod Top() {}\n@rf".to_string();
    let offset = doc.source.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"rfport"), "expected `rfport` in {labels:?}");
}

/// Only schema names matching the typed prefix are offered — `@zz` must not
/// include `rfport`.
#[test]
fn schema_completion_filters_by_typed_prefix() {
    let mut doc = DocumentState::new("mod Top() {}\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some());

    doc.source = "mod Top() {}\n@zz".to_string();
    let offset = doc.source.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"rfport"), "`@zz` must not offer `rfport`: {labels:?}");
}

/// Off the `@` position entirely, completion falls back to the ordinary
/// predictive-parser completions — schema names are not injected everywhere.
#[test]
fn completion_off_attr_position_does_not_offer_schema_names() {
    let mut doc = DocumentState::new("mod Top() {}\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some());

    let offset = doc.source.len();
    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"rfport"), "unrelated position must not offer schema names: {labels:?}");
}

/// A `@rfport`-annotated wire shows the attribute as a nested outline entry
/// on the wire's own declaration.
#[test]
fn attribute_instance_appears_as_outline_entry_on_its_declaration() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = 1, z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    let design = doc.design.as_ref().expect("source must elaborate cleanly");

    let symbols = extract_symbols(design, src);
    let module_sym = symbols.iter().find(|s| s.name == "M").expect("module `M` must be in the outline");
    let wire_children = module_sym.children.as_ref().expect("module must have children");
    let wire_sym = wire_children.iter().find(|s| s.name.starts_with("rf_in")).expect("wire `rf_in` must be in the outline");

    let attr_children = wire_sym.children.as_ref().expect("wire with an attribute must have outline children");
    assert!(
        attr_children.iter().any(|c| c.name == "@rfport"),
        "expected an `@rfport` outline entry, got: {:?}",
        attr_children.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}

/// A wire with no attribute has no attribute children in the outline (no
/// regression / no spurious entries).
#[test]
fn outline_entry_without_attribute_has_no_attribute_children() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\nwire plain : Electrical;\n}\n";
    let doc = analyzed(src);
    let design = doc.design.as_ref().expect("source must elaborate cleanly");

    let symbols = extract_symbols(design, src);
    let module_sym = symbols.iter().find(|s| s.name == "M").expect("module `M` must be in the outline");
    let wire_children = module_sym.children.as_ref().expect("module must have children");
    let wire_sym = wire_children.iter().find(|s| s.name.starts_with("plain")).expect("wire `plain` must be in the outline");

    assert!(wire_sym.children.is_none(), "an un-attributed wire must not carry outline children");
}
