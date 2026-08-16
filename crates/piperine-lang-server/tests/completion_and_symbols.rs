//! Completion and document symbols: schema-name completion after `@` (and its
//! suppression off attribute position), plus the outline entries `extract_symbols`
//! produces.
//!
//! **lang-server-vams-migration (T7)**: migrated to real Verilog-AMS.
//! `rfport` (PHDL's ad-hoc user schema) no longer exists — VAMS re-anchored
//! the schema mechanism on exactly two fixed names, `device`/`port`
//! (`piperine_lang::seed_device_port_schemas`, always seeded by
//! `DocumentState::analyze`) — schema-completion tests below assert on
//! `device`/`port` instead.

mod common;
use common::*;

/// `@dev|` completes to `device` — the built-in `device` schema is always
/// in-scope (`seed_device_port_schemas`, called unconditionally by
/// `DocumentState::analyze`).
///
/// Types `@dev` after a valid document, simulating the moment completion is
/// triggered mid-edit: `doc.source` is updated but not re-analyzed, so
/// `doc.ctx` still holds the last successful elaboration's registries — the
/// pre-existing stale-but-valid resilience (`state.rs`) completion rides.
#[test]
fn schema_completion_after_at_sign_offers_in_scope_schema_names() {
    let mut doc = DocumentState::new("module Top();\nendmodule\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some(), "must elaborate cleanly: {:?}", doc.errors);

    doc.source = "module Top();\nendmodule\n@dev".to_string();
    let offset = doc.source.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"device"), "expected `device` in {labels:?}");
}

/// Only schema names matching the typed prefix are offered — `@zz` must not
/// include `device`.
#[test]
fn schema_completion_filters_by_typed_prefix() {
    let mut doc = DocumentState::new("module Top();\nendmodule\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some());

    doc.source = "module Top();\nendmodule\n@zz".to_string();
    let offset = doc.source.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"device"), "`@zz` must not offer `device`: {labels:?}");
}

/// Off the `@` position entirely, completion falls back to the ordinary
/// static completions — schema names are not injected everywhere.
#[test]
fn completion_off_attr_position_does_not_offer_schema_names() {
    let mut doc = DocumentState::new("module Top();\nendmodule\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some());

    let offset = doc.source.len();
    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"device"), "unrelated position must not offer schema names: {labels:?}");
}

/// A `(* port = .., kind = .. *)`-annotated port shows the attribute as a
/// nested outline entry on the port's own declaration.
///
/// **Blocked**: `handlers/symbols.rs::extract_symbols` only pushes a
/// module's own `DocumentSymbol` (and therefore its children — ports,
/// params, wires, behaviors, instances, attributes) when `span_to_range
/// (source, module.span)` succeeds, i.e. when `module.span.is_some()`.
/// `elab/lower/vams.rs` always builds `Module` with `span: None`, so a
/// VAMS module NEVER appears in the outline at all — `extract_symbols`
/// returns `[]` for any VAMS design, unconditionally (confirmed directly:
/// probing this exact fixture returns an empty `Vec`). This is the
/// feature's documented `span: None` gap (spec.md's Out of Scope table),
/// at its most severe for the outline feature specifically.
#[test]
fn attribute_instance_appears_as_outline_entry_on_its_declaration() {
    let src = "module M(p, gnd);\n(* port = \"A0\", kind = \"digital\" *) inout electrical p;\ninout electrical gnd;\nendmodule\n";
    let doc = analyzed(src);
    let design = doc.design.as_ref().expect("source must elaborate cleanly");

    let symbols = extract_symbols(design, src);
    let module_sym = symbols.iter().find(|s| s.name == "M").expect("module `M` must be in the outline");
    let port_children = module_sym.children.as_ref().expect("module must have children");
    let port_sym = port_children.iter().find(|s| s.name.starts_with('p')).expect("port `p` must be in the outline");

    let attr_children = port_sym.children.as_ref().expect("port with an attribute must have outline children");
    assert!(
        attr_children.iter().any(|c| c.name == "@port"),
        "expected a `@port` outline entry, got: {:?}",
        attr_children.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}

/// A port with no attribute has no attribute children in the outline (no
/// regression / no spurious entries).
///
/// **Blocked**: same `Module::span: None` gap as above — the module never
/// reaches the outline at all.
#[test]
fn outline_entry_without_attribute_has_no_attribute_children() {
    let src = "module M(p);\ninout electrical p;\nendmodule\n";
    let doc = analyzed(src);
    let design = doc.design.as_ref().expect("source must elaborate cleanly");

    let symbols = extract_symbols(design, src);
    let module_sym = symbols.iter().find(|s| s.name == "M").expect("module `M` must be in the outline");
    let port_children = module_sym.children.as_ref().expect("module must have children");
    let port_sym = port_children.iter().find(|s| s.name.starts_with('p')).expect("port `p` must be in the outline");

    assert!(port_sym.children.is_none(), "an un-attributed port must not carry outline children");
}
