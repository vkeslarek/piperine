//! Scope: MSL parse-phase diagnostics (`piperine_lang::msl`): the spec's error
//! codes at the offending span, recovery, and professional messages
//! (requirement MSLP-04, MSLP-06). Grows with the parser: T6 starts it with
//! the nesting limit and the in-expression `+>` / `<-` error.

use piperine_lang::msl::Span;
use piperine_lang::msl::parser::{MAX_DEPTH, parse, parse_expr};

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

/// The codes of a source's diagnostics with their spans.
fn codes(src: &str) -> Vec<(&'static str, Span)> {
    parse(src).diagnostics.iter().map(|d| (d.code, d.span())).collect()
}

#[test]
fn forbidden_forms_in_a_function_body_report_e0121() {
    // Part I §3 rule 6: a function body has no `signal`, `<-`, `+>`,
    // `bound_step` and no `on`.
    for (src, at, len) in [
        ("fn f() { signal x: Bit; }", 9, 14),
        ("fn f() { on clk.rise() { q <- d; } }", 9, 25),
        ("fn f() { q <- d; }", 9, 7),
        ("fn f() { i(p, n) +> 1.0; }", 9, 15),
        ("fn f() { bound_step(1.0); }", 9, 15),
        // Nested in control flow and closures: still the function body.
        ("fn f() { if a { x +> 1.0; } }", 16, 9),
        ("fn f() { let g = |x| { y <- x; }; }", 23, 7),
    ] {
        let found = codes(src);
        assert!(
            found.contains(&("E0121", Span::new(at, at + len))),
            "{src:?}: {found:?}"
        );
    }
    // The same statements are fine outside a function body.
    let r = parse("fn f() {} struct S;");
    assert!(r.diagnostics.is_empty());
    // A method body is a function body.
    let found = codes("impl X { fn f(self) { q <- 1; } }");
    assert!(found.iter().any(|(c, _)| *c == "E0121"), "{found:?}");
}

#[test]
fn macros_other_than_emit_and_include_str_report_e0901() {
    // Appendix A.1 rule 5: any other `path !` followed by a delimiter, and
    // `macro_rules`, are errors; `emit!` and `include_str!` are grammar forms.
    for (src, at, len) in [
        ("fn f() { println!(\"x\"); }", 9, 13),
        ("fn f() { let v = vec![1, 2]; }", 17, 10),
        ("fn f() { a::b!{ } }", 9, 8),
        ("macro_rules! m { () => {} }", 0, 27),
    ] {
        let found = codes(src);
        assert!(found.contains(&("E0901", Span::new(at, at + len))), "{src:?}: {found:?}");
    }
    let r = parse("fn f() { emit!(\"sim\", finish = true); } const A: &'static str = include_str!(\"a.txt\");");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn keyword_as_identifier_reports_e0903() {
    // Appendix A.1 rule 6: an added keyword is not an identifier, nor is a
    // Rust keyword (`out` is reserved, so `fn out()` is an error).
    for (src, at, len) in [
        ("fn out() {}", 3, 3),
        ("fn f(on: u8) {}", 5, 2),
        ("struct S { after: u8 }", 11, 5),
        ("fn f() { let flip = 1; }", 13, 4),
        ("const module: u8 = 1;", 6, 6),
    ] {
        let found = codes(src);
        assert_eq!(found.first(), Some(&("E0903", Span::new(at, at + len))), "{src:?}: {found:?}");
    }
    // Contextual keywords stay identifiers.
    let r = parse("fn analog() {} fn flow(potential: u8, value: u8, resolve: u8) {}");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn depth_limit_fits_8mib_stack_in_debug() {
    // MSLP-04 AC 7: 256 nesting levels parse on the caller's thread with an
    // 8 MiB stack (the default main-thread size), 300 report E0910, and the
    // parser spawns no larger-stack thread of its own.
    let handle = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            for (open, close) in [("(", ")"), ("{ ", " }"), ("[", "]"), ("f(", ")"), ("if a { ", " }")] {
                let src = format!("{}1{}", open.repeat(256), close.repeat(256));
                let (expr, diags) = parse_expr(&src);
                assert!(expr.is_some(), "{open:?} at 256");
                assert!(diags.is_empty(), "{open:?} at 256: {diags:?}");
                let src = format!("{}1{}", open.repeat(300), close.repeat(300));
                let (_, diags) = parse_expr(&src);
                assert!(
                    !diags.is_empty() && diags.iter().all(|d| d.code == "E0910"),
                    "{open:?} at 300: {diags:?}"
                );
            }
            // The whole-file entry holds too, on the same stack.
            let body = format!("{}1{}", "{ ".repeat(250), " }".repeat(250));
            let r = parse(&format!("fn f() {{ {body} }}"));
            assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        })
        .expect("spawn the 8 MiB test thread");
    handle.join().expect("no stack overflow, no panic");

    // No thread is spawned by the parser: its entry module names no stack size.
    let entry = include_str!("../src/msl/parser/entry.rs");
    assert!(!entry.contains("stack_size") && !entry.contains("thread::"), "the parser runs on the caller's thread");
}
