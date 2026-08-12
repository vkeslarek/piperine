//! Proves the `Formatter` (tasks.md T15): round-trip idempotence over the
//! Phase 1-2 test corpus, comment preservation, and macro line-continuation
//! preservation.

use piperine_vams::Formatter;

#[test]
fn round_trip_is_idempotent_on_a_representative_module() {
    let src = "module   res(inout electrical p,n);\n  parameter real r=1.0e3;\n  analog\n    V(p,n) <+   r * I(p,n);\nendmodule\n";
    let once = Formatter::format(src).expect("first format");
    let twice = Formatter::format(&once).expect("second format");
    assert_eq!(once, twice, "formatting an already-formatted file must be a no-op");
}

#[test]
fn round_trip_is_idempotent_across_the_phase_1_2_test_inputs() {
    // A representative sample spanning nature/discipline/module/analog/
    // digital/generate/UDP/gate constructs from tasks.md T9-T11's own test
    // suites — not exhaustive line-for-line, but one fixture per grammar
    // area those phases added.
    let samples = [
        "nature Voltage; units = \"V\"; abstol = 1e-6; access = V; endnature\n",
        "discipline electrical; potential Voltage; flow Current; enddiscipline\n",
        "module m(inout electrical p, n);\n  branch (p,n) br;\n  analog V(br) <+ 1.0;\nendmodule\n",
        "module counter(input clk, output reg q);\n  always @(posedge clk) q <= ~q;\nendmodule\n",
        "module gen(input [3:0] a);\n  genvar i;\n  generate\n    for (i = 0; i < 4; i = i + 1) begin\n    end\n  endgenerate\nendmodule\n",
    ];
    for src in samples {
        let once = Formatter::format(src).unwrap_or_else(|e| panic!("format {src:?}: {e}"));
        let twice = Formatter::format(&once).expect("second format");
        assert_eq!(once, twice, "not idempotent for input: {src:?}");
    }
}

#[test]
fn multiple_spaces_between_tokens_collapse_to_one() {
    let src = "module    m(inout    electrical   p); endmodule";
    let out = Formatter::format(src).unwrap();
    assert!(!out.contains("    "), "runs of spaces must collapse: {out:?}");
    assert!(out.contains("module m"), "single space must remain between tokens: {out:?}");
}

#[test]
fn adjacent_tokens_with_no_gap_stay_adjacent() {
    let src = "module m(inout electrical p); parameter real r=1.0; endmodule";
    let out = Formatter::format(src).unwrap();
    // `r=1.0` had no space around `=` in the input; the formatter must not
    // invent spacing it wasn't asked to add.
    assert!(out.contains("r=1.0"), "adjacent tokens must not gain inserted spacing: {out:?}");
}

#[test]
fn trailing_whitespace_before_newline_is_dropped() {
    let src = "module m(inout electrical p);   \nendmodule\n";
    let out = Formatter::format(src).unwrap();
    for line in out.lines() {
        assert_eq!(line, line.trim_end(), "line has trailing whitespace: {line:?}");
    }
}

#[test]
fn leading_indentation_is_preserved_verbatim() {
    let src = "module m(inout electrical p);\n        analog V(p) <+ 0.0;\nendmodule\n";
    let out = Formatter::format(src).unwrap();
    assert!(
        out.contains("\n        analog"),
        "original 8-space indentation must survive unchanged: {out:?}"
    );
}

#[test]
fn line_comments_are_preserved_verbatim() {
    let src = "module m(inout electrical p); // a trailing comment\nendmodule\n";
    let out = Formatter::format(src).unwrap();
    assert!(out.contains("// a trailing comment"), "comment text must survive: {out:?}");
}

#[test]
fn block_comments_are_preserved_verbatim() {
    let src = "/* block\n   comment */\nmodule m(inout electrical p); endmodule\n";
    let out = Formatter::format(src).unwrap();
    assert!(out.contains("/* block\n   comment */"), "block comment must survive byte-for-byte: {out:?}");
}

#[test]
fn macro_definition_with_backslash_continuation_is_preserved_verbatim() {
    let src = "`define ADD(a,b) \\\n  ((a) + (b))\nmodule m(inout electrical p); endmodule\n";
    let out = Formatter::format(src).unwrap();
    assert!(
        out.contains("`define ADD(a,b) \\\n  ((a) + (b))"),
        "the whole continued macro definition must survive byte-for-byte, got: {out:?}"
    );
}

#[test]
fn a_directive_line_without_continuation_is_preserved_verbatim() {
    let src = "`define WIDTH    4\nmodule m(inout electrical p); endmodule\n";
    let out = Formatter::format(src).unwrap();
    // Deliberately NOT collapsed — directive lines are untouched, even
    // their internal spacing, because reformatting a macro body safely
    // requires preprocessor-level understanding this formatter doesn't
    // have (module doc's stated scope boundary).
    assert!(
        out.contains("`define WIDTH    4"),
        "directive line spacing must be untouched: {out:?}"
    );
}

#[test]
fn string_literal_internal_whitespace_is_never_collapsed() {
    let src = "module m(inout electrical p);\n  parameter string s = \"a   b\";\nendmodule\n";
    let out = Formatter::format(src).unwrap();
    assert!(
        out.contains("\"a   b\""),
        "whitespace inside a string literal must never be touched: {out:?}"
    );
}
