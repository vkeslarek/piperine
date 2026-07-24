//! `@model`/`@name`/`@unit`/`@description`/`@kind` textual `extern attribute`
//! schemas (phdl-introspection-attributes PIA-04). These schemas ship in the
//! always-embedded prelude (`headers/introspection.phdl`, wired in
//! `resolve.rs::prelude_items`), so every compilation unit registers them with
//! a real `decl_span` — LSP go-to-def (MD-24) is inherited for free from the
//! `SymbolKind::AttrSchema` arm (`piperine-lang-server/src/symbol_index.rs`),
//! not duplicated here.
//!
//! What this file proves: (a) each schema is present in the prelude (a use
//! site elaborates cleanly with no per-project `extern attribute` declaration),
//! (b) field validation is not weakened by `extern` (an unknown field fails
//! loud through the shared `convert_attribute` path), (c) the keyed-only
//! grammar is honored (`value = "..."` for single-field schemas, per user
//! decision 2026-07-23). The richer placement/enum/duplicate validation lives
//! in the resolver covered by `introspection_meta.rs` (T2); this file is the
//! schema-registration gate.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(src, &SourceMap::dummy())
}

/// PIA-04: `@model(type, version)` on a module elaborates from the prelude
/// (no per-project declaration) — the schema is registered for every project.
#[test]
fn model_attribute_in_prelude_elaborates_on_module() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        @model(type = \"mos\", version = \"3\")
        mod M ( inout p : Electrical ) { }
    ";
    elaborate(src).expect("@model must elaborate via the prelude schema");
}

/// PIA-04: each single-field metadata attribute elaborates from the prelude
/// on its natural placement (var/port/wire) with the keyed `value = "..."` form.
#[test]
fn single_field_attributes_in_prelude_elaborate() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( @name(value = \"P\") inout p : Electrical ) {
            @name(value = \"i_d\") @unit(value = \"A\") @description(value = \"drain current\") @kind(value = \"State\")
            var i_d : Real;
            @name(value = \"cp\") @kind(value = \"internal\") wire cp : Electrical;
        }
    ";
    elaborate(src).expect("single-field introspection attrs must elaborate via the prelude schemas");
}

/// PIA-04 negative: an unknown field on a prelude introspection schema fails
/// loud through the shared `convert_attribute` field-validation path — `extern`
/// does not weaken schema validation.
#[test]
fn unknown_field_on_prelude_schema_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @name(not_a_field = \"x\") var v : Real;
        }
    ";
    let err = elaborate(src).expect_err("an unknown field must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("not_a_field"), "error should name the unknown field: {msg}");
}

/// PIA-04: a module with none of the new attributes still elaborates — the
/// schemas are purely additive (zero regression on existing stdlib models).
#[test]
fn module_without_introspection_attrs_still_elaborates() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var v : Real; }
    ";
    elaborate(src).expect("a module with no introspection attrs must still elaborate");
}
