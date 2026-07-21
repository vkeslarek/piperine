//! Declared-first, fail-loud resolution for types and attribute schemas
//! (declared-language-surface T12, DLS-01/04).
//!
//! Type-reference resolution (`Elaborator::resolve_type`,
//! `elab/lower/resolve.rs`) and `@attr(...)` resolution
//! (`elab/lower/attrs.rs::convert_attribute`) already look up their
//! respective registry first and fail loud with no Rust-side fallback —
//! this task's job is completing the `extern attribute` registration path
//! (`elab/lower/register.rs`, landed alongside T11) and proving both halves
//! of DLS-01/04 with dedicated tests:
//!
//! - Undeclared type name fails loud (DLS-04) — already proven by
//!   `elab.rs::test_undefined_type_error`; not duplicated here.
//! - Undeclared `@attr` schema name fails loud (DLS-04) — **new coverage**,
//!   `UnknownAttrSchema` had no dedicated test before this task.
//! - A declared `extern attribute` resolves and validates a real use site
//!   (DLS-01/03) — the first test exercising `extern attribute`'s
//!   registration wiring end-to-end through `parse_and_elaborate`.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(src, &SourceMap::dummy())
}

/// DLS-04 (attribute schemas): a name with no schema registered anywhere
/// (bundle-backed or `extern attribute`) fails loud, naming the schema.
#[test]
fn undeclared_attr_schema_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Top ( inout p : Electrical ) {
            @totally_bogus_schema(x = 1) wire w : Electrical;
        }
    ";
    let err = elaborate(src).expect_err("an undeclared @attr schema must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("totally_bogus_schema"), "error should name the schema: {msg}");
}

/// DLS-01/03 (attribute schemas): an `extern attribute` declaration
/// registers a real schema — a use site providing its declared field
/// elaborates cleanly, proving `elab/lower/register.rs`'s `extern
/// attribute` wiring is a real, working consumer.
#[test]
fn extern_attribute_declares_and_resolves_a_real_use_site() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        extern attribute widget_meta { rating: Real }
        mod Top ( inout p : Electrical ) {
            @widget_meta(rating = 4.5) wire w : Electrical;
        }
    ";
    elaborate(src).expect("a use site matching the extern attribute's declared field must elaborate cleanly");
}

/// DLS-03 (attribute schemas, negative half): providing a field the
/// `extern attribute` declaration doesn't have is a normal schema-field
/// error — `extern` does not weaken field validation.
#[test]
fn extern_attribute_use_site_with_unknown_field_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        extern attribute widget_meta { rating: Real }
        mod Top ( inout p : Electrical ) {
            @widget_meta(not_a_field = 1.0) wire w : Electrical;
        }
    ";
    let err = elaborate(src).expect_err("a field not in the extern attribute's schema must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("not_a_field"), "error should name the unknown field: {msg}");
}

/// DLS-04 (types, reference point): the pre-existing "declared-first,
/// fail-loud" behavior for type references — `resolve_type` already checks
/// `TypeRegistry` and fails with `UndefinedType` for anything not found,
/// with no Rust-side fallback. Kept here (not duplicated from
/// `elab.rs::test_undefined_type_error`) only as the documented evidence
/// that this half of DLS-04 needed no code change, just confirmation the
/// existing behavior still holds after T11's `resolve.rs` changes.
#[test]
fn undeclared_type_reference_still_fails_loud_after_call_resolution_changes() {
    let src = "mod Top ( inout p : TotallyUndeclaredType );";
    let err = elaborate(src).expect_err("an undeclared type reference must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("TotallyUndeclaredType"), "error should name the undeclared type: {msg}");
}
