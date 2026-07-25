//! `piperine fmt` must never insert a blank line between a `///` doc-comment
//! run and the declaration it documents — the lexer's doc-attach rule drops
//! a pending run on a blank line (LSP-06), so a blank-line-inserting
//! formatter silently strips every doc comment it "cleans up".

use piperine_lang::parse::format::{FormatOptions, TokenFormatter};
use piperine_lang::parse::lexer::Lexer;

fn format(src: &str) -> String {
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize_all().expect("lexer must succeed");
    TokenFormatter::format_source(src, &tokens, FormatOptions::default())
}

/// A `///` doc comment directly above `mod` must stay directly above it —
/// no blank line inserted between them.
#[test]
fn doc_comment_stays_attached_to_mod_after_formatting() {
    let src = "/// A resistor.\nmod Resistor(inout p: Electrical, inout n: Electrical) { }\n";
    let out = format(src);
    assert!(
        out.contains("/// A resistor.\nmod Resistor"),
        "doc comment must stay directly adjacent to `mod`, got:\n{out}"
    );
}

/// Same, for `fn`.
#[test]
fn doc_comment_stays_attached_to_fn_after_formatting() {
    let src = "/// Doubles x.\nfn double(x: Real) -> Real { return 2.0 * x; }\n";
    let out = format(src);
    assert!(
        out.contains("/// Doubles x.\nfn double"),
        "doc comment must stay directly adjacent to `fn`, got:\n{out}"
    );
}

/// A multi-line `///` block must also stay attached — the LAST doc line is
/// the one immediately preceding the declaration.
#[test]
fn multiline_doc_comment_stays_attached() {
    let src = "/// Line one.\n/// Line two.\nmod M() { }\n";
    let out = format(src);
    assert!(
        out.contains("/// Line two.\nmod M"),
        "the doc block's last line must stay directly adjacent to `mod`, got:\n{out}"
    );
}

/// A plain `//` (non-doc) comment is NOT doc-attaching — the formatter's
/// normal blank-line-before-declaration behavior must be unaffected for it
/// (this proves the fix is scoped to `///` specifically, not comments in
/// general).
#[test]
fn plain_comment_still_gets_blank_line_before_mod() {
    let src = "// just a note\nmod M() { }\n";
    let out = format(src);
    assert!(
        out.contains("// just a note\n\nmod M"),
        "a plain `//` comment should still get the usual blank-line separation, got:\n{out}"
    );
}
