//! Scope: MSL parse-phase diagnostics (`piperine_lang::msl`): the spec's error
//! codes at the offending span, recovery, and professional messages
//! (requirement MSLP-04, MSLP-06). Grows with the parser: T6 starts it with
//! the nesting limit and the in-expression `+>` / `<-` error.

use piperine_lang::msl::Span;
use piperine_lang::msl::parser::{MAX_DEPTH, parse_expr};

#[test]
fn depth_over_256_is_diagnostic_not_overflow() {
    // MSLP-04 AC 6: 300 nested parentheses give a depth diagnostic, and the
    // parser neither overflows the stack nor keeps descending.
    assert_eq!(MAX_DEPTH, 256);
    let src = format!("{}1{}", "(".repeat(300), ")".repeat(300));
    let (expr, diags) = parse_expr(&src);
    assert!(expr.is_some(), "a partial tree is still returned");
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].code, "E0910");
    // The label sits on the first parenthesis past the limit.
    assert_eq!(diags[0].span().start, 256);

    // Exactly at the limit is fine.
    let src = format!("{}1{}", "(".repeat(256), ")".repeat(256));
    let (_, diags) = parse_expr(&src);
    assert!(diags.is_empty(), "{diags:?}");

    // Blocks, `if` bodies and calls nest through more grammar layers per
    // level than parentheses; the limit holds for them too.
    for (open, close) in [("{ ", " }"), ("if a { ", " }"), ("f(", ")"), ("[", "]")] {
        let src = format!("{}1{}", open.repeat(255), close.repeat(255));
        let (_, diags) = parse_expr(&src);
        assert!(diags.is_empty(), "{open:?} at depth 255: {diags:?}");
        let src = format!("{}1{}", open.repeat(400), close.repeat(400));
        let (_, diags) = parse_expr(&src);
        assert!(diags.iter().all(|d| d.code == "E0910") && !diags.is_empty(), "{open:?} at 400: {diags:?}");
    }

    // A much deeper source (and other delimiters) is still one diagnostic.
    let src = format!("{}1{}", "[".repeat(5000), "]".repeat(5000));
    let (_, diags) = parse_expr(&src);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].code, "E0910");
}

#[test]
fn plus_gt_and_commit_inside_an_expression_report_e0907() {
    // Appendix A.5 rule 6 / A.6: neither token is an expression operator.
    for (src, at) in [("let x = a +> b;", 10), ("f(a <- b)", 4), ("c = (a +> b);", 7)] {
        let wrapped = format!("{{ {src} }}");
        let (_, diags) = piperine_lang::msl::parser::parse_block(&wrapped);
        assert!(!diags.is_empty(), "{src:?} must not parse");
        assert_eq!(diags[0].code, "E0907", "{src:?}: {diags:?}");
        assert_eq!(diags[0].span(), Span::new(at + 2, at + 4), "{src:?}");
        assert!(diags[0].help.is_some());
    }
}
