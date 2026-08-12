//! Proves the preprocessor (tasks.md T5) against LRM §10's textual
//! directives: `` `define ``/`` `undef `` (incl. arguments and rescan),
//! the `` `ifdef ``/`` `ifndef ``/`` `else ``/`` `elsif ``/`` `endif ``
//! family, `` `include `` via an injected [`IncludeResolver`] (no
//! filesystem I/O in the crate), and the fail-loud error paths.

use piperine_vams::{IncludeResolver, ParseErrorKind, Preprocessor};
use std::collections::HashMap;

struct MapResolver(HashMap<String, String>);

impl IncludeResolver for MapResolver {
    fn resolve(&self, path: &str) -> Option<String> {
        self.0.get(path).cloned()
    }
}

fn run(src: &str) -> String {
    Preprocessor::new().run(src).unwrap_or_else(|e| panic!("unexpected preprocess error: {e}"))
}

// ── §10.4 `define / `undef ──────────────────────────────────────────────

#[test]
fn object_like_macro_expands() {
    assert_eq!(run("`define WIDTH 8\n`WIDTH"), "\n8");
}

#[test]
fn function_like_macro_expands_with_argument_substitution() {
    assert_eq!(run("`define ADD(a,b) (a+b)\n`ADD(1,2)"), "\n(1+2)");
}

#[test]
fn function_like_macro_wrong_arity_is_an_error() {
    let err = Preprocessor::new().run("`define ADD(a,b) (a+b)\n`ADD(1)").unwrap_err();
    match err.kind {
        ParseErrorKind::MacroArityMismatch { name, expected, found } => {
            assert_eq!(name, "ADD");
            assert_eq!(expected, 2);
            assert_eq!(found, 1);
        }
        other => panic!("expected MacroArityMismatch, got {other:?}"),
    }
}

#[test]
fn rescan_expands_macros_used_inside_another_macros_body() {
    // CPP-style rescan: WIDTH's expansion is itself scanned for `MAX_WIDTH`.
    assert_eq!(run("`define MAX_WIDTH 8\n`define WIDTH `MAX_WIDTH\n`WIDTH"), "\n\n8");
}

#[test]
fn undef_removes_a_macro_so_later_use_is_undefined() {
    let err = Preprocessor::new().run("`define X 1\n`undef X\n`X").unwrap_err();
    match err.kind {
        ParseErrorKind::UndefinedMacro { name } => assert_eq!(name, "X"),
        other => panic!("expected UndefinedMacro, got {other:?}"),
    }
}

#[test]
fn undefined_macro_use_is_an_error_naming_it() {
    let err = Preprocessor::new().run("`NEVER_DEFINED").unwrap_err();
    match err.kind {
        ParseErrorKind::UndefinedMacro { name } => assert_eq!(name, "NEVER_DEFINED"),
        other => panic!("expected UndefinedMacro, got {other:?}"),
    }
}

#[test]
fn multiline_macro_body_via_line_continuation() {
    assert_eq!(run("`define BODY line1 \\\nline2\n`BODY"), "\nline1 \nline2");
}

// ── `ifdef / `ifndef / `else / `elsif / `endif ──────────────────────────

#[test]
fn ifdef_true_branch_taken_when_macro_defined() {
    assert_eq!(run("`define X 1\n`ifdef X\nyes\n`endif"), "\n\nyes\n");
}

#[test]
fn ifdef_false_branch_skipped_when_macro_undefined() {
    assert_eq!(run("`ifdef NOPE\nyes\n`endif"), "");
}

#[test]
fn ifndef_takes_branch_when_macro_undefined() {
    assert_eq!(run("`ifndef NOPE\nyes\n`endif"), "\nyes\n");
}

#[test]
fn else_branch_taken_when_ifdef_condition_false() {
    assert_eq!(run("`ifdef NOPE\na\n`else\nb\n`endif"), "\nb\n");
}

#[test]
fn elsif_chain_picks_the_first_true_branch() {
    // The `elsif B` line's own trailing newline is emitted because the
    // branch is already active by the time that newline is scanned — same
    // behavior as `ifdef_true_branch_taken_when_macro_defined`'s leading
    // blank line above.
    let src = "`define B 1\n`ifdef A\na\n`elsif B\nb\n`elsif C\nc\n`else\nd\n`endif";
    assert_eq!(run(src), "\n\nb\n");
}

#[test]
fn nested_ifdef_inside_a_skipped_branch_does_not_leak() {
    let src = "`ifdef NOPE\n`ifdef ALSO_NOPE\ninner\n`endif\nouter\n`else\ntaken\n`endif";
    assert_eq!(run(src), "\ntaken\n");
}

#[test]
fn endif_without_matching_ifdef_is_an_error() {
    let err = Preprocessor::new().run("`endif").unwrap_err();
    match err.kind {
        ParseErrorKind::UnmatchedConditionalDirective { directive } => assert_eq!(directive, "endif"),
        other => panic!("expected UnmatchedConditionalDirective, got {other:?}"),
    }
}

#[test]
fn else_without_matching_ifdef_is_an_error() {
    let err = Preprocessor::new().run("`else").unwrap_err();
    match err.kind {
        ParseErrorKind::UnmatchedConditionalDirective { directive } => assert_eq!(directive, "else"),
        other => panic!("expected UnmatchedConditionalDirective, got {other:?}"),
    }
}

#[test]
fn unterminated_conditional_is_an_error_naming_the_opener() {
    let err = Preprocessor::new().run("`ifdef X\nnever closed").unwrap_err();
    match err.kind {
        ParseErrorKind::UnterminatedConditional { directive } => assert_eq!(directive, "ifdef"),
        other => panic!("expected UnterminatedConditional, got {other:?}"),
    }
}

// ── `include (via injected resolver — no filesystem I/O) ────────────────

#[test]
fn include_inlines_the_resolved_file() {
    let mut files = HashMap::new();
    files.insert("defs.vams".to_string(), "electrical a;".to_string());
    let out = Preprocessor::with_include_resolver(MapResolver(files))
        .run("`include \"defs.vams\"\nmodule m; endmodule")
        .unwrap();
    assert_eq!(out, "electrical a;\nmodule m; endmodule");
}

#[test]
fn include_not_found_is_an_error_naming_the_path() {
    let err = Preprocessor::with_include_resolver(MapResolver(HashMap::new()))
        .run("`include \"missing.vams\"")
        .unwrap_err();
    match err.kind {
        ParseErrorKind::IncludeNotFound { path } => assert_eq!(path, "missing.vams"),
        other => panic!("expected IncludeNotFound, got {other:?}"),
    }
}

#[test]
fn recursive_include_is_an_error_naming_the_cycle() {
    let mut files = HashMap::new();
    files.insert("a.vams".to_string(), "`include \"a.vams\"".to_string());
    let err = Preprocessor::with_include_resolver(MapResolver(files))
        .run("`include \"a.vams\"")
        .unwrap_err();
    match err.kind {
        ParseErrorKind::RecursiveInclude { path } => assert_eq!(path, "a.vams"),
        other => panic!("expected RecursiveInclude, got {other:?}"),
    }
}

#[test]
fn default_resolver_rejects_every_include() {
    let err = Preprocessor::new().run("`include \"anything.vams\"").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::IncludeNotFound { .. }));
}

// ── `timescale / `resetall pass through untouched ───────────────────────

#[test]
fn timescale_and_resetall_pass_through_as_directive_text() {
    assert_eq!(run("`timescale 1ns/1ps\ncode"), "`timescale 1ns/1ps\ncode");
    assert_eq!(run("`resetall\ncode"), "`resetall\ncode");
}

// ── Comments/strings are not scanned for directives ─────────────────────

#[test]
fn backtick_inside_a_string_is_not_a_directive() {
    assert_eq!(run(r#""a `not_a_directive b""#), r#""a `not_a_directive b""#);
}

#[test]
fn backtick_inside_a_line_comment_is_not_a_directive() {
    assert_eq!(run("// `not_a_directive\ncode"), "// `not_a_directive\ncode");
}
