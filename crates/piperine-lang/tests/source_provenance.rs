//! Covers `SourceProvenance`/`Span`/`ExpansionFrame` (T1/T2 of
//! `.specs/features/preprocessor-span-mapping/tasks.md`): the new
//! domain types and forward `resolve()`. 1:1 to SPANMAP-01/02/03/07 plus
//! the spec's zero-length-span and straddling-span edge cases.

use piperine_lang::error::PreprocessedSpan as PreSpan;
use piperine_lang::source_provenance::{Builder, SourceId, SourceProvenance};

/// SPANMAP-01/02: a segment-covered offset maps to its real
/// `(source_id, local_offset)` — not the flat buffer offset.
#[test]
fn resolve_maps_segment_covered_offset_to_real_source_and_local_offset() {
    let mut b = Builder::new();
    // Buffer: "AAAA" (top-level, bytes 0..4) + "BBBBBBBB" (included file,
    // bytes 4..12) — the included file's local text is longer than the
    // top-level file, the exact shape that pushed offsets past the
    // top-level file's own length in the original crash (SPANMAP-02).
    let top = b.add_source("top.vams", "AAAA");
    let inc = b.add_source("inc.vams", "xxxxBBBBBBBBxxxx");
    b.record(0..4, top, 0, Vec::new());
    b.record(4..12, inc, 4, Vec::new());
    let prov = b.build();

    // Offset 6 in the flat buffer (inside "BBBBBBBB") must resolve to
    // inc.vams, local offset 4 + (6-4) = 6 — never top.vams, and never the
    // raw flat offset 6 misapplied to top.vams's own 4-byte text.
    let resolved = prov.resolve(PreSpan::new(6, 8));
    assert_eq!(resolved.source_id, inc);
    assert_eq!(resolved.start, 6);
    assert_eq!(resolved.end, 8);
    assert_eq!(&prov.text(inc)[resolved.start..resolved.end], "BB");
}

/// SPANMAP-01: a span fully inside top-level (unexpanded) text resolves to
/// that file's own identity and offsets unchanged — the "nothing expanded"
/// no-regression case.
#[test]
fn resolve_top_level_only_span_is_identity_mapped() {
    let mut b = Builder::new();
    let top = b.add_source("top.vams", "module m; endmodule");
    b.record(0..20, top, 0, Vec::new());
    let prov = b.build();

    let resolved = prov.resolve(PreSpan::new(0, 6));
    assert_eq!(resolved.source_id, top);
    assert_eq!(resolved.start, 0);
    assert_eq!(resolved.end, 6);
    assert_eq!(&prov.text(top)[resolved.start..resolved.end], "module");
}

/// SPANMAP-03: the same file path included twice resolves both inclusions
/// to one stable `SourceId`, not two synthetic copies.
#[test]
fn repeated_source_name_shares_one_source_id() {
    let mut b = Builder::new();
    let first = b.add_source("disciplines.vams", "discipline electrical; enddiscipline;");
    let second = b.add_source("disciplines.vams", "discipline electrical; enddiscipline;");
    assert_eq!(first, second);
}

/// Spec Edge Case: a zero-length `PreprocessedSpan` still resolves to a
/// valid, in-bounds, zero-width `Span` — no panic, no divide-by-zero.
#[test]
fn resolve_zero_length_span_is_valid_and_zero_width() {
    let mut b = Builder::new();
    let top = b.add_source("top.vams", "module m; endmodule");
    b.record(0..20, top, 0, Vec::new());
    let prov = b.build();

    let resolved = prov.resolve(PreSpan::new(5, 5));
    assert_eq!(resolved.source_id, top);
    assert_eq!(resolved.start, 5);
    assert_eq!(resolved.end, 5);
}

/// Design's "Relationships" note: a span that straddles two segments (e.g.
/// an unterminated construct detected only after crossing an `` `include ``
/// boundary) resolves using the segment covering its *start* offset —
/// matching the existing "unterminated X anchors on start" convention.
#[test]
fn resolve_straddling_span_uses_start_segment() {
    let mut b = Builder::new();
    let top = b.add_source("top.vams", "AAAA");
    let inc = b.add_source("inc.vams", "xxxxBBBB");
    b.record(0..4, top, 0, Vec::new());
    b.record(4..8, inc, 4, Vec::new());
    let prov = b.build();

    // Span starts in top-level text (offset 2) but its end (offset 6) lands
    // inside the included segment.
    let resolved = prov.resolve(PreSpan::new(2, 6));
    assert_eq!(resolved.source_id, top);
    assert_eq!(resolved.start, 2);
}

/// Design's Risks-table mitigation: a hand-built/test `SourceProvenance`
/// with no recorded segments degrades `resolve()` to a synthetic `<test>`
/// source at the same offset rather than the loud-fail path.
#[test]
fn empty_test_provenance_degrades_instead_of_failing_loud() {
    let prov = SourceProvenance::empty_for_test();
    let resolved = prov.resolve(PreSpan::new(10, 20));
    assert_eq!(resolved.start, 10);
    assert_eq!(resolved.end, 20);
    assert_eq!(prov.name(resolved.source_id), "<test>");
}

/// SPANMAP-07: with no directives (one segment covering the whole flat
/// buffer, same as the source text), every offset resolves identically to
/// today's un-mapped behavior.
#[test]
fn resolve_no_directives_case_is_identity_for_whole_file() {
    let mut b = Builder::new();
    let text = "module m;\n  parameter real x = 1.0;\nendmodule";
    let top = b.add_source("top.vams", text);
    b.record(0..text.len(), top, 0, Vec::new());
    let prov = b.build();

    let resolved = prov.resolve(PreSpan::new(0, text.len()));
    assert_eq!(resolved.source_id, top);
    assert_eq!(resolved.start, 0);
    assert_eq!(resolved.end, text.len());
    assert_eq!(prov.text(top), text);
}

/// `SourceId` type check — a plain data shape (T1's "compile with the
/// fields the design specifies").
#[test]
fn source_id_is_copy_eq() {
    let a = SourceId(0);
    let b = a;
    assert_eq!(a, b);
}
