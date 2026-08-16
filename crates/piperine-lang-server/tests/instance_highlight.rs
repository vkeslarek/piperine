//! BUG-3 (LSB-07..10, T8): document-highlight/goto on a labeled instance
//! targets the clicked token (label or type name), not the whole
//! multi-line instance statement.
//!
//! Fixture mirrors spec.md's exact reported repro shape: a labeled
//! instance with a parameter override.
//!
//! **lang-server-vams-migration (T9)**: migrated to real Verilog-AMS.
//! **Every test in this file is blocked** by the feature's documented
//! `span: None` gap, from two directions at once:
//! - `elab/lower/vams.rs` always builds `Instance` with `label_span: None`,
//!   `type_span: None` (confirmed: every `Instance { ... }` literal in that
//!   file hardcodes both) — so even if cursor-context ever engaged, the
//!   Instance-kind resolution itself has no real span to report.
//! - `resolve_in_module` (the only place an `Instance` is ever a
//!   resolution candidate) requires cursor context (`span_contains
//!   (m.span, offset)`), which never succeeds because `Module::span` is
//!   also always `None`.
//! - `occurrences_at`/`index_design` are separately and unconditionally
//!   empty for VAMS content (see `document_highlight.rs`'s file-level
//!   doc), so even the "click the type name, which coincidentally matches
//!   a real module name via the step-3 global scan" path still fails once
//!   it reaches `decl_span.expect(...)` (`Module::span: None` too).

use piperine_lang_server::occurrences::occurrences_for_decl_span;
use piperine_lang_server::state::DocumentState;

const SRC: &str = "module RampSource(p, n);\n\
  inout electrical p, n;\n\
  parameter real slope = 0.0;\n\
endmodule\n\
module Top(vin, gnd);\n\
  inout electrical vin, gnd;\n\
  RampSource #(.slope(4.0e5)) src(.p(vin), .n(gnd));\n\
endmodule\n";

fn analyzed(src: &str) -> DocumentState {
    let mut doc = DocumentState::new(src.to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.design.is_some(), "fixture must elaborate cleanly");
    doc
}

/// Highlighting the label `"src"` returns a tight 3-byte range, not the
/// whole-statement range (spec.md's exact reported bug).
///
/// **Blocked**: see file-level doc — `occurrences_at` is always empty for
/// VAMS content.
#[test]
fn highlighting_labeled_instance_label_targets_only_the_label_token() {
    let doc = analyzed(SRC);
    let label_offset = SRC.find("src(").expect("label present in fixture");

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
/// doc and goto jumps to the module declaration.
///
/// **Blocked**: `RampSource` resolves as `Module` via the step-3 global
/// name scan (unaffected by span), but `res.decl_span.expect(...)` panics
/// — `Module::span` is always `None`.
#[test]
fn clicking_instance_type_name_resolves_to_the_module() {
    let doc = analyzed(SRC);
    let type_offset = SRC.find("RampSource #(").expect("type name present in fixture");

    let res = doc.resolve_at(type_offset).expect("type name should resolve");
    assert_eq!(
        res.name, "RampSource",
        "clicking the type name resolves to the module `RampSource`, got name `{}`",
        res.name
    );
    // Its decl_span must be the module's own declaration (the `module
    // RampSource ...` at the top), NOT the instance statement.
    let decl_span = res.decl_span.expect("module resolution carries a decl_span");
    let mod_decl_offset = SRC.find("module RampSource").expect("module decl present");
    assert!(
        decl_span.offset() >= mod_decl_offset && decl_span.offset() < mod_decl_offset + 20,
        "type-name click's decl_span must point at the `module RampSource` declaration (offset ~{mod_decl_offset}), got offset {}",
        decl_span.offset()
    );
}

/// An unlabeled instance's type-name click also resolves to the module
/// (same as the labeled case — the label is irrelevant to what the type
/// name references).
///
/// **Blocked**: same reason as above.
#[test]
fn clicking_unlabeled_instance_type_name_resolves_to_the_module() {
    let src = "module RampSource(p, n);\n\
  inout electrical p, n;\n\
  parameter real slope = 0.0;\n\
endmodule\n\
module Top(vin, gnd);\n\
  inout electrical vin, gnd;\n\
  RampSource inst1(.p(vin), .n(gnd));\n\
endmodule\n";
    let doc = analyzed(src);
    let type_offset = src.find("RampSource inst1").expect("unlabeled instance present in fixture");

    let res = doc.resolve_at(type_offset).expect("type name should resolve");
    assert_eq!(res.name, "RampSource", "unlabeled instance's type name resolves to the module");
    let decl_span = res.decl_span.expect("module resolution carries a decl_span");
    let mod_decl_offset = src.find("module RampSource").expect("module decl present");
    assert!(
        decl_span.offset() >= mod_decl_offset && decl_span.offset() < mod_decl_offset + 20,
        "must point at the `module RampSource` declaration, got offset {}",
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
///
/// **Blocked**: `doc.resolve_at(label_offset)` on the label itself never
/// resolves at all — the label branch of `resolve_in_module` requires
/// cursor context (`Module::span: None`), and the label word "src" matches
/// no module/discipline/schema name for the step-3 fallback to catch.
#[test]
fn resolved_decl_span_has_a_matching_binding_in_the_resolution_index() {
    let doc = analyzed(SRC);
    let label_offset = SRC.find("src(").expect("label present in fixture");

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
