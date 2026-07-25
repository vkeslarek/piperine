//! BUG-3 (LSB-07..10, T8): document-highlight/goto on a labeled instance
//! targets the clicked token (label or type name), not the whole
//! multi-line instance statement.
//!
//! Fixture mirrors spec.md's exact reported repro: `src : RampSource(.p =
//! vin, .n = gnd) { .slope = 4.0e5 };` — highlighting `"src"` used to
//! return a 56-byte range (offset 454, length 56, the whole statement)
//! instead of a 3-byte range covering just the label.

use piperine_lang_server::occurrences::occurrences_for_decl_span;
use piperine_lang_server::state::DocumentState;

const SRC: &str = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod RampSource ( inout p : Electrical, inout n : Electrical ) { param slope: Real = 0.0; }\n\
mod Top ( inout vin : Electrical, inout gnd : Electrical ) {\n\
    src : RampSource(.p = vin, .n = gnd) { .slope = 4.0e5 };\n\
}\n";

fn analyzed(src: &str) -> DocumentState {
    let mut doc = DocumentState::new(src.to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.design.is_some(), "fixture must elaborate cleanly");
    doc
}

/// Highlighting the label `"src"` returns a tight 3-byte range, not the
/// 56-byte whole-statement range (spec.md's exact reported bug).
#[test]
fn highlighting_labeled_instance_label_targets_only_the_label_token() {
    let doc = analyzed(SRC);
    let label_offset = SRC.find("src :").expect("label present in fixture");

    let ranges = doc.occurrences_at(label_offset);
    assert!(!ranges.is_empty(), "clicking the label should resolve to at least one occurrence");

    // The occurrence covering the click site itself must be exactly the
    // 3-byte `src` token, not the whole instance statement.
    let (start, end) = ranges
        .iter()
        .copied()
        .find(|&(s, e)| s <= label_offset && label_offset < e)
        .expect("one occurrence must cover the click site");
    assert_eq!(end - start, 3, "label highlight range should be exactly 3 bytes (`src`), got {}..{}", start, end);
    assert_eq!(&SRC[start..end], "src");
}

/// Clicking the type name `"RampSource"` on an instance resolves to the
/// MODULE it references (not the instance) — so hover shows the module's
/// doc and goto jumps to the module declaration. (This supersedes the
/// earlier BUG-3 "tight type-token highlight" behavior: a type name is a
/// reference to its module, and every IDE resolves it to that module —
/// the actual BUG-3 defect was highlighting the whole 56-byte *statement*,
/// which is fixed either way. See `symbol_index.rs`'s instance arm.)
#[test]
fn clicking_instance_type_name_resolves_to_the_module() {
    let doc = analyzed(SRC);
    let type_offset = SRC.find(": RampSource(").expect("type name present in fixture") + 2;

    let res = doc.resolve_at(type_offset).expect("type name should resolve");
    assert_eq!(
        res.name, "RampSource",
        "clicking the type name resolves to the module `RampSource`, got name `{}`",
        res.name
    );
    // Its decl_span must be the module's own declaration (the `mod
    // RampSource ...` at the top), NOT the 56-byte instance statement.
    let decl_span = res.decl_span.expect("module resolution carries a decl_span");
    let mod_decl_offset = SRC.find("mod RampSource").expect("module decl present");
    assert!(
        decl_span.offset() >= mod_decl_offset && decl_span.offset() < mod_decl_offset + 20,
        "type-name click's decl_span must point at the `mod RampSource` declaration (offset ~{mod_decl_offset}), got offset {}",
        decl_span.offset()
    );
}

/// An unlabeled instance's type-name click also resolves to the module
/// (same as the labeled case — the label is irrelevant to what the type
/// name references).
#[test]
fn clicking_unlabeled_instance_type_name_resolves_to_the_module() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod RampSource ( inout p : Electrical, inout n : Electrical ) { param slope: Real = 0.0; }\n\
mod Top ( inout vin : Electrical, inout gnd : Electrical ) {\n\
    RampSource(.p = vin, .n = gnd);\n\
}\n";
    let doc = analyzed(src);
    let type_offset = src.find("RampSource(.p").expect("unlabeled instance present in fixture");

    let res = doc.resolve_at(type_offset).expect("type name should resolve");
    assert_eq!(res.name, "RampSource", "unlabeled instance's type name resolves to the module");
    let decl_span = res.decl_span.expect("module resolution carries a decl_span");
    let mod_decl_offset = src.find("mod RampSource").expect("module decl present");
    assert!(
        decl_span.offset() >= mod_decl_offset && decl_span.offset() < mod_decl_offset + 20,
        "must point at the `mod RampSource` declaration, got offset {}",
        decl_span.offset()
    );
}

/// Design.md's consistency requirement: `symbol_index.rs`'s `resolve_at`
/// (which computes `decl_span` from `label_span`/`type_span` directly on
/// the click site) and `elab/resolution.rs::index_design` (which indexes
/// `ResolutionIndex` bindings by the *same* convention) must agree
/// byte-for-byte, or `occurrences_for_decl_span`'s exact-match `.find()`
/// silently returns empty. This test bypasses `DocumentState::
/// occurrences_at`'s "fall back to decl_span itself when nothing found"
/// masking (see `occurrences.rs`'s doc comment) and asserts the index
/// lookup itself succeeds — the discrimination-sensitive assertion T8's
/// gate requires.
#[test]
fn resolved_decl_span_has_a_matching_binding_in_the_resolution_index() {
    let doc = analyzed(SRC);
    let label_offset = SRC.find("src :").expect("label present in fixture");

    let resolution = doc.resolve_at(label_offset).expect("label should resolve");
    let decl_span = resolution.decl_span.expect("resolution should carry a decl_span");
    assert_eq!(decl_span.len(), 3, "resolve_at's decl_span should already be the tight label span");

    let idx = doc.resolution_index.as_ref().expect("resolution_index built after analyze");
    let occurrences = occurrences_for_decl_span(idx, decl_span);
    assert!(
        !occurrences.is_empty(),
        "index_design must index this instance's binding at the SAME decl_span symbol_index.rs \
         resolves to (label_span) — a mismatched convention on either side silently empties this list"
    );
}
