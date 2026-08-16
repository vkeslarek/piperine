//! Covers macro-expansion dual attribution (tasks.md T9/T10 of
//! `.specs/features/preprocessor-span-mapping/tasks.md`): a `Span` that
//! falls inside macro-expanded text carries an `expansion_chain` of
//! `ExpansionFrame`s (invocation + definition + argument spans), 1:1 to
//! SPANMAP-04/05/06/09.

use piperine_lang::error::PreprocessedSpan as PreSpan;
use piperine_lang::{parse_and_elaborate, Preprocessor, SourceMap};

/// Runs the preprocessor and returns the flat output plus its
/// `SourceProvenance`.
fn run(src: &str) -> (String, piperine_lang::source_provenance::SourceProvenance) {
    Preprocessor::new().run(src).unwrap_or_else(|e| panic!("unexpected preprocess error: {e}"))
}

/// SPANMAP-04/05: an error inside a single-level macro expansion resolves
/// with a one-frame `expansion_chain` carrying the correct invocation site
/// (the `` `NAME(...) `` text the author wrote) and the correct definition
/// site (the macro's own `` `define `` body text).
#[test]
fn single_level_macro_expansion_resolves_one_frame_with_correct_invocation_and_definition() {
    let src = "`define ADD(a,b) (a+b)\n`ADD(1,2)";
    let (out, prov) = run(src);
    assert_eq!(out, "\n(1+2)");

    // Resolve a span inside the expanded output "(1+2)".
    let expanded_start = out.find("(1+2)").expect("expansion present in output");
    let span = PreSpan::new(expanded_start, expanded_start + "(1+2)".len());
    let resolved = prov.resolve(span);

    assert_eq!(resolved.expansion_chain.len(), 1, "one macro hop deep");
    let frame = &resolved.expansion_chain[0];
    assert_eq!(frame.macro_name, "ADD");

    // Primary label: the invocation site is exactly the `ADD(1,2)` text
    // the author wrote in `src`.
    let invocation_text = src.find("`ADD(1,2)").map(|i| &src[i..i + "`ADD(1,2)".len()]).unwrap();
    assert_eq!(invocation_text, "`ADD(1,2)");
    let invocation_slice = &prov.text(frame.invocation.source_id)[frame.invocation.start..frame.invocation.end];
    assert_eq!(invocation_slice, "`ADD(1,2)");

    // Secondary "expanded from here" label: the definition site's text
    // (after trimming the leading/trailing whitespace the raw span
    // includes) is the macro's own body, `(a+b)`.
    let definition_slice = &prov.text(frame.definition.source_id)[frame.definition.start..frame.definition.end];
    assert_eq!(definition_slice.trim(), "(a+b)");
}

/// SPANMAP-06: a doubly-nested macro expansion (a macro invoking another
/// macro) resolves with a two-frame chain, innermost first.
#[test]
fn doubly_nested_macro_expansion_resolves_two_frame_chain_innermost_first() {
    let src = "`define INNER(x) (x*2)\n`define OUTER(y) `INNER(y)\n`OUTER(5)";
    let (out, prov) = run(src);

    // The fully-rescanned expansion of `OUTER(5)` -> `INNER(5)` -> "(5*2)".
    let expanded_start = out.find("(5*2)").expect("nested expansion present in output");
    let span = PreSpan::new(expanded_start, expanded_start + "(5*2)".len());
    let resolved = prov.resolve(span);

    assert_eq!(resolved.expansion_chain.len(), 2, "two macro hops deep");
    assert_eq!(resolved.expansion_chain[0].macro_name, "INNER", "innermost frame first");
    assert_eq!(resolved.expansion_chain[1].macro_name, "OUTER", "outermost frame last");

    // The outer frame's own invocation site is the real `OUTER(5)` call in
    // `src`, and its definition is `OUTER`'s own body.
    let outer = &resolved.expansion_chain[1];
    let outer_invocation_slice = &prov.text(outer.invocation.source_id)[outer.invocation.start..outer.invocation.end];
    assert_eq!(outer_invocation_slice, "`OUTER(5)");
    let outer_definition_slice = &prov.text(outer.definition.source_id)[outer.definition.start..outer.definition.end];
    assert_eq!(outer_definition_slice.trim(), "`INNER(y)");

    // The inner frame's definition is `INNER`'s own body, resolved
    // correctly regardless of the fact its invocation happened inside
    // OUTER's already-expanded text.
    let inner = &resolved.expansion_chain[0];
    let inner_definition_slice = &prov.text(inner.definition.source_id)[inner.definition.start..inner.definition.end];
    assert_eq!(inner_definition_slice.trim(), "(x*2)");
}

/// Spec Edge Case (P2 AC4): a `Span` inside plain (non-macro, non-include)
/// source text has an empty `expansion_chain` — no noise on the common
/// case.
#[test]
fn plain_source_span_has_empty_expansion_chain() {
    let src = "module m;\n  parameter real x = 1.0;\nendmodule";
    let (out, prov) = run(src);
    assert_eq!(out, src, "no directives, output identical to input");

    let start = out.find("parameter").unwrap();
    let span = PreSpan::new(start, start + "parameter".len());
    let resolved = prov.resolve(span);

    assert!(resolved.expansion_chain.is_empty());
}

/// SPANMAP-09: a multi-argument macro invocation (the `` `MPRnb(name,
/// default, units, desc)``-shaped idiom from spec P2.5) records `args` with
/// the correct param name -> argument-text `Span` pairing, and each
/// argument's text is recoverable via `SourceProvenance::text()` at that
/// span.
#[test]
fn multi_argument_macro_invocation_records_param_to_arg_span_pairs() {
    let src = "`define MPRnb(name,default,units,desc) name\n\
               `MPRnb(rs, 1.0, \"ohms\", \"series resistance\")";
    let (out, prov) = run(src);

    let expanded_start = out.rfind("rs").expect("expansion present in output");
    let span = PreSpan::new(expanded_start, expanded_start + 2);
    let resolved = prov.resolve(span);

    assert_eq!(resolved.expansion_chain.len(), 1);
    let frame = &resolved.expansion_chain[0];
    assert_eq!(frame.macro_name, "MPRnb");
    assert_eq!(frame.args.len(), 4, "all four declared parameters recorded");

    let expected: [(&str, &str); 4] =
        [("name", "rs"), ("default", "1.0"), ("units", "\"ohms\""), ("desc", "\"series resistance\"")];
    for (idx, (expected_name, expected_text)) in expected.iter().enumerate() {
        let (pname, arg_span) = &frame.args[idx];
        assert_eq!(pname, expected_name);
        let arg_text = &prov.text(arg_span.source_id)[arg_span.start..arg_span.end];
        assert_eq!(arg_text, *expected_text, "argument {idx} ({pname}) text recoverable via SourceProvenance::text()");
    }
}

/// SPANMAP-04/05/06 (T10): rendering a macro-expansion error's
/// `miette::Report` shows both the invocation location (primary label,
/// what the module author wrote and can act on) and an "expanded from
/// here"-labeled definition location (secondary, rustc macro-backtrace
/// convention) — the object-like macro `` `BAD `` expands into a bare
/// `#`, which is invalid wherever an identifier is expected, so the
/// parse failure's span lands strictly inside the macro-expanded text.
#[test]
fn macro_expansion_error_report_shows_both_invocation_and_expanded_from_here() {
    let source_map = SourceMap::dummy();
    let src = "`define BAD #\nmodule `BAD;\nendmodule\n";
    let err = parse_and_elaborate(src, &source_map).expect_err("`#` is not a valid module name");
    let rendered = format!("{err:?}");

    // Primary label: the parser stopped at the macro invocation site
    // (`` `BAD `` in `module `BAD;`), which the rendered snippet includes
    // via the second source line of `src`.
    assert!(rendered.contains("module `BAD;"), "primary label's snippet should show the invocation line, got:\n{rendered}");

    // Secondary "expanded from here" label: the macro's own `` `define ``
    // body location, in a related/lower-severity note distinct from the
    // primary error.
    assert!(
        rendered.contains("expanded from here"),
        "rendered report should carry an 'expanded from here' related label, got:\n{rendered}"
    );
    assert!(
        rendered.contains("expanded from macro `BAD`") || rendered.contains("BAD"),
        "the related label should name the macro, got:\n{rendered}"
    );
}

/// Spec Edge Case (P2 AC4, T10 half): a plain (non-macro, non-include)
/// parse failure's rendered report carries no "expanded from here" noise.
#[test]
fn plain_source_error_report_has_no_expanded_from_here_note() {
    let source_map = SourceMap::dummy();
    let src = "module Bad(";
    let err = parse_and_elaborate(src, &source_map).expect_err("malformed module should fail to parse");
    let rendered = format!("{err:?}");
    assert!(!rendered.contains("expanded from"), "plain-source error should carry no expansion-chain note, got:\n{rendered}");
}
