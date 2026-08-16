//! Covers the render-boundary fix (tasks.md T8, SPANMAP-01/02/03/07): a
//! parse/elaboration error's rendered `miette::Report` shows the real
//! originating file and a valid, in-bounds snippet — never the top-level
//! file's own (possibly too-short) text, never an `OutOfBounds` crash.
//! This is the committed repro of the exact crash class the
//! `vams-corpus` sweep found (`vsine.vams`, an `` `include ``d header
//! longer than the file that includes it).

use piperine_lang::{parse_and_elaborate, SourceMap};
use tempfile::TempDir;

/// SPANMAP-01/02: an error inside `` `include ``d content — where the
/// included file is longer than the top-level file that includes it,
/// pushing the flat-buffer offset past the top-level file's own byte
/// length — renders using the *included* file's own name and text, not
/// the top-level file's, and never panics/reports out-of-bounds.
#[test]
fn error_inside_a_longer_include_resolves_to_the_included_file() {
    let dir = TempDir::new().unwrap();
    // The included file is deliberately much longer than the tiny
    // top-level file that includes it — the exact shape that pushed
    // offsets past the top-level file's own length in the original crash.
    // The malformed token (`;` where an identifier is expected) sits
    // strictly inside the included content, not at end-of-file, so the
    // error can't legitimately land on any trailing top-level text.
    let padding = "// padding line to make this file long\n".repeat(20);
    let included = format!("{padding}module Bad(;\nendmodule\n");
    std::fs::write(dir.path().join("big.vams"), &included).unwrap();

    let top_level = "`include \"big.vams\"";
    let source_map = SourceMap::new(dir.path().to_path_buf());

    let err = parse_and_elaborate(top_level, &source_map).expect_err("malformed module should fail to parse");
    // Rendering (Debug, which drives miette's fancy graphical renderer)
    // must not panic and must not report an out-of-bounds span.
    let rendered = format!("{err:?}");
    assert!(!rendered.contains("OutOfBounds"), "rendered report reported OutOfBounds:\n{rendered}");
    assert!(
        rendered.contains("big.vams"),
        "rendered report should name the included file (big.vams), got:\n{rendered}"
    );
    assert!(
        !rendered.contains(top_level),
        "rendered report should not fall back to the top-level file's own (too-short) text"
    );
}

/// SPANMAP-07: a plain top-level file with no `` `include ``/`` `define ``
/// directives resolves identically to today's un-mapped behavior — same
/// file, same offsets, same snippet — the majority existing-suite case.
#[test]
fn error_in_plain_source_resolves_to_the_top_level_file_unchanged() {
    let source_map = SourceMap::dummy();
    let src = "module Bad(";
    let err = parse_and_elaborate(src, &source_map).expect_err("malformed module should fail to parse");
    let rendered = format!("{err:?}");
    assert!(!rendered.contains("OutOfBounds"), "rendered report reported OutOfBounds:\n{rendered}");
    // The only source text involved is `src` itself — its own malformed
    // module text must appear in the rendered snippet.
    assert!(rendered.contains("Bad"), "rendered report should show the top-level file's own text, got:\n{rendered}");
    // SPANMAP-01's zero-regression AC: a caller that never gave a real
    // file path (`parse_and_elaborate`'s public API takes a bare `&str`)
    // must render exactly as it did before this feature — no synthetic
    // `<input>:` name label ever appeared pre-feature, and none may now.
    assert!(
        !rendered.contains("<input>"),
        "rendered report must not show the internal top-level sentinel name, got:\n{rendered}"
    );
}
