//! Annex B conformance gate (tasks.md T4, requirement VAMS-24). Two
//! independent checks, both re-run every phase (this gate is meant to stay
//! meaningful as the crate grows, not just pass once):
//!
//! 1. `Keywords::all()` matches Table B.1 **exactly** — re-derived from the
//!    checked-in `VAMS-LRM-2-4.md` at test time (not a copy compared to
//!    itself), so a future edit that silently drops or adds a keyword to
//!    `keywords.rs` without updating the table (or vice versa) fails loud.
//! 2. Every keyword is rejected as a plain identifier with a typed error
//!    naming it (`Keywords::expect_identifier`), and every keyword lexes as
//!    a plain `Tok::Ident` (§2.8.2: a keyword is lexically an identifier;
//!    only a grammar-level "expect identifier" check distinguishes them).

use piperine_vams::{Keywords, Lexer, ParseErrorKind, Span, Tok};
use std::collections::BTreeSet;
use std::path::Path;

/// Re-derives the keyword set from the LRM's own Annex B table text,
/// independently of `keywords.rs`'s `KEYWORDS` constant.
fn keywords_from_lrm_table() -> BTreeSet<String> {
    let lrm_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VAMS-LRM-2-4.md");
    let text = std::fs::read_to_string(&lrm_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", lrm_path.display()));

    // Anchored on the table caption, not the "List of keywords" heading —
    // that heading text also appears earlier in the LRM's own table of
    // contents, which would truncate the scanned range to nothing.
    let start = text.find("Table B.1").expect("Table B.1 caption not found in LRM");
    let end = text[start..]
        .find("Analog language subset")
        .map(|i| start + i)
        .expect("Annex C heading not found after Annex B in LRM");
    let table_text = &text[start..end];

    // Table cells are backtick-delimited (`` `above` ``); every other
    // segment after splitting on '`' is a cell's contents. Keep only
    // segments that look like a keyword (lowercase ascii + digits + '_',
    // non-empty) to skip stray backticks in surrounding prose.
    let mut out = BTreeSet::new();
    for (i, seg) in table_text.split('`').enumerate() {
        if i % 2 == 1
            && !seg.is_empty()
            && seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            out.insert(seg.to_string());
        }
    }
    out
}

#[test]
fn keyword_table_matches_annex_b_exactly() {
    let from_lrm = keywords_from_lrm_table();
    let from_table: BTreeSet<String> = Keywords::all().iter().map(|s| s.to_string()).collect();

    let missing: Vec<_> = from_lrm.difference(&from_table).collect();
    let extra: Vec<_> = from_table.difference(&from_lrm).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "keywords.rs KEYWORDS drifted from LRM Annex B Table B.1 — \
         missing (in LRM, not in keywords.rs): {missing:?}, \
         extra (in keywords.rs, not in LRM): {extra:?}"
    );
}

#[test]
fn every_annex_b_keyword_is_reserved() {
    for kw in Keywords::all() {
        assert!(Keywords::is_reserved(kw), "{kw} should be reserved but is_reserved returned false");
    }
}

#[test]
fn every_annex_b_keyword_lexes_as_a_plain_identifier() {
    // §2.8.2: a keyword is "a predefined simple identifier" — lexically
    // indistinguishable from any other identifier at the lexer layer.
    for kw in Keywords::all() {
        let toks: Vec<Tok> = Lexer::new(kw)
            .tokenize()
            .unwrap_or_else(|e| panic!("{kw} failed to lex: {e}"))
            .into_iter()
            .map(|l| l.tok)
            .filter(|t| !matches!(t, Tok::Eof))
            .collect();
        assert_eq!(toks, vec![Tok::Ident(kw.to_string())], "{kw} did not lex as a plain Ident");
    }
}

#[test]
fn every_annex_b_keyword_is_rejected_as_an_identifier_with_a_named_error() {
    for kw in Keywords::all() {
        let err = Keywords::expect_identifier(kw, Span::new(0, kw.len()))
            .expect_err(&format!("{kw} should be rejected as an identifier"));
        match err.kind {
            ParseErrorKind::UnexpectedToken { expected, found } => {
                assert_eq!(expected, "an identifier");
                assert!(found.contains(kw), "error for {kw} must name the keyword, got {found:?}");
            }
            other => panic!("{kw}: expected UnexpectedToken, got {other:?}"),
        }
        assert_eq!(err.span, Span::new(0, kw.len()));
    }
}

#[test]
fn a_non_keyword_identifier_is_accepted() {
    assert!(!Keywords::is_reserved("my_signal_name"));
    assert!(Keywords::expect_identifier("my_signal_name", Span::new(0, 14)).is_ok());
}
