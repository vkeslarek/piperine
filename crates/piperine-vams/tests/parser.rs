//! Proves the `Parser` façade (tasks.md T12, VAMS-05): the single
//! preprocess -> lex -> parse entry point, and that errors surface with
//! the deepest meaningful span rather than an outer "somewhere in this
//! file" span.

use piperine_vams::error::ParseErrorKind;
use piperine_vams::{IncludeResolver, Parser};

#[test]
fn parses_a_minimal_module_end_to_end() {
    let src = "module res(inout electrical p, n); endmodule";
    let file = Parser::new().parse(src).expect("full pipeline should parse a minimal module");
    assert_eq!(file.items.len(), 1);
}

#[test]
fn parses_a_module_using_a_macro_defined_via_the_preprocessor() {
    let src = "`define GAIN 4\nmodule amp(inout electrical p); parameter integer g = `GAIN; endmodule";
    let file = Parser::new().parse(src).expect("preprocessor stage must run before lex/parse");
    assert_eq!(file.items.len(), 1);
}

#[test]
fn lexical_error_carries_the_offending_token_span_not_an_outer_span() {
    // A stray, unterminated string literal — the lexer must fail with a
    // span pointing AT the bad token, not span 0..len of the whole file.
    let src = "module m(inout electrical p); parameter s = \"unterminated; endmodule";
    let err = Parser::new().parse(src).unwrap_err();
    assert!(err.span.start > 0, "lexical error span must point at the offending token, not file start");
    assert!(err.span.end <= src.len());
}

#[test]
fn grammar_error_carries_the_span_of_the_construct_that_failed_to_parse() {
    // `endmodule` is missing its `module` — the grammar-level error must
    // point at the malformed construct, not the file as a whole.
    let src = "module m(inout electrical p);\n  parameter real r = ;\nendmodule";
    let err = Parser::new().parse(src).unwrap_err();
    assert!(err.span.start > 0);
    assert!(err.span.end > err.span.start);
}

#[test]
fn missing_include_fails_loud_with_the_directive_span_when_no_resolver_is_given() {
    // A leading comment line pushes the `include directive off byte 0, so
    // the span assertion actually distinguishes "points at the directive"
    // from "defaulted to the start of the file".
    let src = "// header\n`include \"missing.vams\"\nmodule m(inout electrical p); endmodule";
    let err = Parser::new().parse(src).unwrap_err();
    match err.kind {
        ParseErrorKind::IncludeNotFound { ref path, .. } => assert_eq!(path, "missing.vams"),
        other => panic!("expected IncludeNotFound with the deepest (directive) span, got {other:?}"),
    }
    let directive_offset = src.find("`include").unwrap();
    assert_eq!(
        err.span.start, directive_offset,
        "include-not-found span must point at the `include directive itself, not the file start"
    );
}

struct StaticResolver;
impl IncludeResolver for StaticResolver {
    fn resolve(&self, path: &str) -> Option<String> {
        if path == "defs.vams" {
            Some("nature Voltage; units = \"V\"; endnature".to_string())
        } else {
            None
        }
    }
}

#[test]
fn with_include_resolver_wires_a_custom_resolver_into_the_full_pipeline() {
    let src = "`include \"defs.vams\"\nmodule m(inout electrical p); endmodule";
    let file = Parser::with_include_resolver(StaticResolver)
        .parse(src)
        .expect("custom resolver must satisfy the `include and let parsing proceed");
    // The included nature declaration plus the module.
    assert_eq!(file.items.len(), 2);
}

#[test]
fn default_parser_has_no_include_support_and_fails_loud_on_any_include() {
    let src = "`include \"anything.vams\"\nmodule m(inout electrical p); endmodule";
    let err = Parser::new().parse(src).unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::IncludeNotFound { .. }));
}
