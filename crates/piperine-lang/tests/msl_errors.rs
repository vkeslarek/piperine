//! Scope: MSL parse-phase diagnostics (`piperine_lang::msl`): the spec's error
//! codes at the offending span, recovery, professional messages and their
//! rendering (requirement MSLP-04, MSLP-06). T6 starts it with the nesting
//! limit and the in-expression `+>` / `<-` error; T7-T11 add the context
//! errors, recovery and the message shape; T13 adds one fixture per
//! parse-phase row of the spec's error tables and the guard that keeps the
//! fixtures complete; T15 checks the miette graphical rendering.

use piperine_lang::msl::{Span, render};
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

/// The nesting shapes that use no delimiter, as `chain_source` builds them.
const CHAINS: &[&str] = &[
    "unary minus", "binary +1", "cast chain", "closure chain", "return chain", "else-if chain", "generic angle",
];

fn chain_source(kind: &str, n: usize) -> String {
    match kind {
        "unary minus" => format!("fn f() -> f64 {{ {}1 }}", "-".repeat(n)),
        "binary +1" => format!("fn f() -> f64 {{ 1{} }}", "+1".repeat(n)),
        "cast chain" => format!("fn f() -> f64 {{ 1 {}}}", "as f64 ".repeat(n)),
        "closure chain" => format!("fn f() {{ {}1 }}", "|x| ".repeat(n)),
        "return chain" => format!("fn f() -> f64 {{ {}1 }}", "return ".repeat(n)),
        "else-if chain" => format!("fn f() {{ if a {{}} {}}}", "else if a {} ".repeat(n)),
        _ => format!("fn f() -> {}u8{} {{ 1 }}", "Option<".repeat(n), ">".repeat(n)),
    }
}

#[test]
fn nesting_without_delimiters_is_also_bounded() {
    // MSLP-04 AC 6 says "nesting", not "delimiters": prefix, cast, closure,
    // `return`, `else if`, generic and binary chains past 256 levels give
    // E0910 and no stack overflow, parse and print, on an 8 MiB stack.
    let handle = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            for kind in CHAINS {
                let deep = parse(&chain_source(kind, 20000));
                assert!(
                    deep.diagnostics.iter().any(|d| d.code == "E0910"),
                    "{kind}: no E0910 at 20000: {:?}",
                    deep.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()
                );
                let _ = piperine_lang::msl::print(&deep.file);
                let ok = parse(&chain_source(kind, 100));
                assert!(ok.diagnostics.is_empty(), "{kind} at 100: {:?}", ok.diagnostics);
            }
        })
        .expect("spawn the 8 MiB test thread");
    handle.join().expect("no stack overflow, no panic");
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

#[test]
fn statements_of_the_wrong_block_kind_report_e0404() {
    // Part IV §10 rule 8: a well-formed statement in a block of the wrong
    // kind is E0404 (E0907 is `+>` or `<-` inside an expression).
    for (src, what) in [
        // `<-` and `on` in an analog block.
        ("analog for M { q <- 1; }", "q <- 1;"),
        ("analog for M { on e { } }", "on e { }"),
        // `+>` and `bound_step` in a reaction (nested blocks too).
        ("digital for M { on e { i(p, n) +> 1.0; } }", "i(p, n) +> 1.0;"),
        ("digital for M { on e { if a { bound_step(1.0); } } }", "bound_step(1.0)"),
        // In a closure outside a function.
        ("analog for M { let f = |x| { q <- x; }; }", "q <- x;"),
        ("digital for M { on e { let f = |x| { i(p, n) +> x; }; } }", "i(p, n) +> x;"),
        // In a module-body expression.
        ("module M() { let f = |x| { q <- x; }; }", "q <- x;"),
    ] {
        let found = codes(src);
        let at = src.find(what).expect("fixture holds its statement");
        assert!(found.contains(&("E0404", Span::new(at, at + what.len()))), "{src:?}: {found:?}");
        assert!(!found.iter().any(|(c, _)| *c == "E0121"), "{src:?}: {found:?}");
    }
    // Well-placed statements are clean.
    for src in [
        "analog for M { i(p, n) +> 1.0; bound_step(1.0); if a { j(p, n) +> 2.0; } }",
        "digital for M { on e { q <- 1; if a { q <- 2 after 1n; } } }",
    ] {
        let found = codes(src);
        assert!(found.is_empty(), "{src:?}: {found:?}");
    }
}

#[test]
fn a_statement_outside_a_reaction_in_digital_reports_e0434() {
    let found = codes("digital for M { q <- 1; on e { } }");
    assert_eq!(found, [("E0434", Span::new(16, 23))]);
    // Inside a generation too.
    let found = codes("digital for M { for i in 0..2 { x = 1; } }");
    assert_eq!(found.first().map(|f| f.0), Some("E0434"), "{found:?}");
}

#[test]
fn module_forms_report_e0302_e0303_e0331() {
    // E0302: a result expression needs `-> T`.
    assert_eq!(codes("module M(a: Bit) { a }"), [("E0302", Span::new(19, 20))]);
    assert!(codes("module M(a: Bit) -> Bit { a }").is_empty());
    // E0303: a module in a block or a function.
    let found = codes("fn f() { module M() {} }");
    assert!(found.contains(&("E0303", Span::new(9, 22))), "{found:?}");
    // E0331: `let mut` in a module body.
    let found = codes("module M() { let mut x = 1; }");
    assert!(found.contains(&("E0331", Span::new(17, 22))), "{found:?}");
}

#[test]
fn multiple_errors_in_source_order_with_partial_ast() {
    // MSLP-04 AC 3: three independent errors give three diagnostics, in
    // source order, and a partial AST that keeps every valid item.
    let src = "fn a() {}\nstruct { x: u8 }\nfn b() { let = 1; let y = 2; }\nconst C: u8 = ;\nfn d() {}";
    let r = parse(src);
    let spans: Vec<Span> = r.diagnostics.iter().map(|d| d.span()).collect();
    let at = |needle: &str| {
        let i = src.find(needle).unwrap();
        Span::new(i, i + needle.len())
    };
    assert_eq!(spans, [at("{ x: u8 }"), at("= 1;"), at(";\nfn d")].map(|s| Span::new(s.start, s.start + 1)), "{:?}", r.diagnostics);
    assert!(r.diagnostics.iter().all(|d| d.code == "E0900"), "{:?}", r.diagnostics);
    assert!(spans.windows(2).all(|w| w[0].start < w[1].start), "source order");

    use piperine_lang::msl::ast::{ExprKind, ItemKind, StmtKind};
    let fns: Vec<&str> = r
        .file
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Fn(f) => Some(f.name.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(fns, ["a", "b", "d"], "the valid items survive");
    assert!(r.file.items.iter().any(|i| matches!(i.kind, ItemKind::Error)), "the broken items are error nodes");
    // `b` keeps the statement after its broken one.
    let ItemKind::Fn(b) = &r.file.items.iter().find(|i| matches!(&i.kind, ItemKind::Fn(f) if f.name.name == "b")).unwrap().kind
    else {
        unreachable!()
    };
    let body = b.body.as_ref().unwrap();
    assert!(matches!(&body.stmts[0].kind, StmtKind::Expr { expr, .. } if matches!(expr.kind, ExprKind::Error)));
    assert!(matches!(body.stmts[1].kind, StmtKind::Let { .. }));

    // Errors inside one block are each reported, and the function stays.
    let r = parse("fn f() { let = 1; x +; let ok = 3; }");
    assert_eq!(r.diagnostics.len(), 2, "{:?}", r.diagnostics);
    assert_eq!(r.file.items.len(), 1);

    // Within a module body, a behavior block and a reaction.
    for src in [
        "module M(a: Bit) { r: ; s: f64; }\nfn later() {}",
        "analog for M { i(p, n) +> ; j(p, n) +> 1.0; }\nfn later() {}",
        "digital for M { on e { q <- ; r <- 1; } }\nfn later() {}",
        "nature N { potential: }\nfn later() {}",
    ] {
        let r = parse(src);
        assert_eq!(r.diagnostics.len(), 1, "{src:?}: {:?}", r.diagnostics);
        assert!(
            matches!(&r.file.items.last().unwrap().kind, ItemKind::Fn(f) if f.name.name == "later"),
            "{src:?}: the item after the error still parses"
        );
    }

    // A closing delimiter with no opener is skipped, once.
    let r = parse("}\nfn f() {}");
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].message, "unmatched `}`");
    assert!(r.file.items.iter().any(|i| matches!(&i.kind, ItemKind::Fn(f) if f.name.name == "f")));
    // The module result is not mistaken for a broken item.
    assert!(parse("module M(a: Bit) -> Bit { r: f64; a }").diagnostics.is_empty());
}

#[test]
fn unexpected_token_names_found_and_expected() {
    // MSLP-04 AC 4: the message names the found token and what was expected.
    for (src, found, expected) in [
        ("fn f() { let x = 1 }", "`}`", "`;`"),
        ("const C: u8 = ;", "`;`", "expression"),
        ("fn f( { }", "`{`", "`)`"),
        ("struct S { a: u8, , }", "`,`", "identifier"),
        ("nature N { potential: }", "`}`", "identifier"),
        ("fn f() {", "end of input", "`}`"),
    ] {
        let r = parse(src);
        let d = &r.diagnostics[0];
        assert_eq!(d.code, "E0900", "{src:?}");
        assert!(
            d.message.starts_with(&format!("unexpected {found}, expected ")) && d.message.contains(expected),
            "{src:?}: {}",
            d.message
        );
    }
    // The expected set is summarized: the dozens of operators that could
    // continue an expression collapse into one phrase, and no message lists
    // more than six alternatives.
    let r = parse("fn f() { let x = 1 }");
    assert_eq!(r.diagnostics[0].message, "unexpected `}`, expected `;` or an operator");
    for src in ["fn f() { let x = 1 }", "digital for M { on { } }", "struct S { a: u8, , }", "fn f( { }"] {
        for d in parse(src).diagnostics {
            let listed = d.message.split_once("expected ").map_or(0, |(_, rest)| rest.split(", ").count());
            assert!(listed <= 6, "{src:?}: {}", d.message);
        }
    }
}

#[test]
fn diagnostic_has_code_message_label_help() {
    // MSLP-04 AC 2: every diagnostic has a code, a one-line message, a
    // primary label at a span inside the source, and a help line naming the
    // correct form for each spec row (the generic E0900 syntax error has none).
    let deep = format!("const A: u8 = {}1{};", "[".repeat(300), "]".repeat(300));
    let fixtures = [
        ("E0910", deep.as_str()),
        ("E0101", "fn f() { let x: dyn T = 1; }"),
        ("E0121", "fn f() { x +> 1.0; }"),
        ("E0302", "module M(a: Bit) { a }"),
        ("E0303", "fn f() { module M() {} }"),
        ("E0331", "module M() { let mut x = 1; }"),
        ("E0404", "analog for M { q <- 1; }"),
        ("E0434", "digital for M { q <- 1; }"),
        ("E0901", "fn f() { println!(\"x\"); }"),
        ("E0902", "const A: f64 = 1e3m;"),
        ("E0903", "fn out() {}"),
        ("E0905", "const A: u8 = 'a';"),
        ("E0907", "fn f() { let x = a +> b; }"),
        ("E0908", "fn f() { let Unknown = 1; }"),
        ("E0900", "fn f( { }"),
    ];
    for (code, src) in fixtures {
        let r = parse(src);
        let d = r
            .diagnostics
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("{src:?} does not report {code}: {:?}", r.diagnostics));
        assert!(!d.message.is_empty() && !d.message.contains('\n'), "{code}: message {:?}", d.message);
        assert!(!d.primary.1.is_empty(), "{code}: primary label text");
        assert!(d.span().start <= d.span().end && d.span().end <= src.len(), "{code}: span {:?}", d.span());
        assert!(!d.span().is_empty() || d.span().end == src.len(), "{code}: an empty label span away from the end");
        if code != "E0900" {
            assert!(d.help.as_deref().is_some_and(|h| !h.is_empty()), "{code}: help names the correct form");
        }
    }
    // Lexical errors come with the same shape.
    for (code, src) in [("E0900", "/* never closed"), ("E0900", "const A: &str = \"open")] {
        let r = parse(src);
        let d = r.diagnostics.iter().find(|d| d.code == code).expect(src);
        assert!(!d.message.is_empty() && !d.primary.1.is_empty() && d.help.is_some(), "{src:?}");
    }
}

#[test]
fn empty_source_empty_file() {
    // MSLP-06: an empty source is an empty file and no diagnostics; so is a
    // source of whitespace and comments.
    for src in ["", "   \n\t\n", "// just a comment\n/* and another */\n"] {
        let r = parse(src);
        assert!(r.file.items.is_empty() && r.file.inner_docs.is_empty(), "{src:?}");
        assert!(r.diagnostics.is_empty(), "{src:?}: {:?}", r.diagnostics);
    }
    let r = piperine_lang::msl::parser::parse_bytes(b"");
    assert!(r.file.items.is_empty() && r.diagnostics.is_empty());
    // Inner doc comments alone make a file with docs and no items.
    let r = parse("//! The crate docs.\n");
    assert_eq!((r.file.inner_docs.len(), r.file.items.len(), r.diagnostics.len()), (1, 0, 0));
    // Invalid UTF-8: one diagnostic and no items.
    let r = piperine_lang::msl::parser::parse_bytes(&[b'f', b'n', 0xff, 0xfe]);
    assert_eq!(r.diagnostics.len(), 1);
    assert!(r.file.items.is_empty());
}

// ---- one fixture per parse-phase error row (T13) -----------------------------------------------

/// A minimal source for one spec error row: `prefix + at + suffix`, where
/// `at` is the text the diagnostic must label (`E0xxx` at exactly that span).
struct Fixture {
    code: &'static str,
    prefix: String,
    at: &'static str,
    suffix: String,
}

fn fixture(code: &'static str, prefix: &str, at: &'static str, suffix: &str) -> Fixture {
    Fixture { code, prefix: prefix.to_string(), at, suffix: suffix.to_string() }
}

/// Every parse-phase row of the spec's error tables, at least once.
fn fixtures() -> Vec<Fixture> {
    vec![
        // Part I
        fixture("E0101", "fn f() { let x: ", "dyn", " T = 1; }"),
        fixture("E0101", "fn f() { ", "unsafe", " { } }"),
        fixture("E0101", "", "async", " fn f() {}"),
        fixture("E0101", "fn f(x: ", "&mut", " u8) {}"),
        fixture("E0121", "fn f() { ", "x +> 1.0;", " }"),
        fixture("E0121", "fn f() { ", "bound_step(1.0)", "; }"),
        fixture("E0142", "fn f() { g(a: 1, ", "2", "); }"),
        fixture("E0143", "fn f(x: f64 ", "= 1.0", ") {}"),
        fixture("E0143", "fn f(", "...", ") {}"),
        fixture("E0143", "fn f() {} fn ", "f", "(x: u8) {}"),
        // Part III
        fixture("E0302", "module M(a: Bit) { ", "a", " }"),
        fixture("E0303", "fn f() { ", "module M() {}", " }"),
        fixture("E0303", "module M() { ", "module N() {}", " }"),
        fixture("E0331", "module M() { let ", "mut x", " = 1; }"),
        // Part IV
        fixture("E0404", "analog for M { ", "q <- 1;", " }"),
        fixture("E0404", "digital for M { on e { ", "i(p, n) +> 1.0;", " } }"),
        fixture("E0434", "digital for M { ", "q <- 1;", " }"),
        // Part VI
        fixture("E0602", "fn f() { emit!(", "1", ", a = 2); }"),
        fixture("E0602", "fn f() { emit!(\"s\", ", "3", "); }"),
        fixture("E0611", "const A: &'static str = include_str!(", "1", ");"),
        // Appendix A
        fixture("E0900", "fn f( ", "{", " }"),
        fixture("E0900", "fn f() { let x = 1 ", "}", ""),
        fixture("E0900", "fn f() {} ", "/*", " never closed"),
        fixture("E0901", "fn f() { ", "println!(\"x\")", "; }"),
        fixture("E0902", "const A: f64 = ", "1e3m", ";"),
        fixture("E0903", "fn ", "out", "() {}"),
        fixture("E0905", "const A: u8 = ", "'a'", ";"),
        fixture("E0907", "fn f() { let x = a ", "+>", " b; }"),
        fixture("E0907", "fn f() { g(a ", "<-", " b); }"),
        fixture("E0908", "fn f() { let ", "Unknown", " = 1; }"),
        // The 257th nested delimiter.
        Fixture {
            code: "E0910",
            prefix: format!("const A: u8 = {}", "[".repeat(256)),
            at: "[",
            suffix: format!("1{};", "]".repeat(257)),
        },
    ]
}

#[test]
fn parse_phase_rows_report_their_codes() {
    // MSLP-04 AC 1: the minimal source of each parse-phase row reports that
    // row's code at the offending span.
    for f in fixtures() {
        let src = format!("{}{}{}", f.prefix, f.at, f.suffix);
        let span = Span::new(f.prefix.len(), f.prefix.len() + f.at.len());
        let found = codes(&src);
        assert!(
            found.contains(&(f.code, span)),
            "{}: {:?} labels {span:?} ({:?}); found {found:?}",
            f.code,
            src,
            f.at
        );
    }
}

/// The codes of every spec error-table row whose phase is `parse`.
fn spec_parse_phase_codes() -> std::collections::BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec");
    let mut codes = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(dir).expect("docs/spec exists") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("spec file is UTF-8");
        for line in text.lines() {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
            let Some(first) = cells.first() else { continue };
            let Some(code) = first.strip_prefix("`E").and_then(|c| c.strip_suffix('`')) else { continue };
            if code.len() == 4 && code.bytes().all(|b| b.is_ascii_digit()) && cells.last() == Some(&"parse") {
                codes.insert(format!("E{code}"));
            }
        }
    }
    codes
}

#[test]
fn every_parse_phase_row_has_a_fixture() {
    // A parse-phase row added to the spec without a fixture fails here.
    let spec = spec_parse_phase_codes();
    assert!(spec.len() >= 19, "the spec's tables list at least the 17 rows and E0900/E0910: {spec:?}");
    let covered: std::collections::BTreeSet<String> = fixtures().iter().map(|f| f.code.to_string()).collect();
    let missing: Vec<&String> = spec.difference(&covered).collect();
    assert!(missing.is_empty(), "parse-phase rows without a fixture: {missing:?}");
    // And a fixture for a code the spec does not list as `parse` is stale.
    let stale: Vec<&String> = covered.difference(&spec).collect();
    assert!(stale.is_empty(), "fixtures for codes that are not parse-phase rows: {stale:?}");
}

#[test]
fn rendered_output_is_miette_graphical() {
    // MSLP-04 AC 5: a rendered diagnostic is a miette graphical report that
    // shows the code, the source line, the underline at the span and the
    // help line. One diagnostic per family: E0907 (a context error),
    // E0900 (a syntax error) and E0902 (a lexical error).
    for (code, src) in [
        ("E0907", "fn f() { let x = a +> b; }"),
        ("E0900", "}\nfn f() {}"),
        ("E0902", "const A: f64 = 1e3m;"),
    ] {
        let r = parse(src);
        let d = r.diagnostics.iter().find(|d| d.code == code).unwrap_or_else(|| panic!("{code}: {:?}", r.diagnostics));
        let out = render(d, "model.msl", src);
        let lines: Vec<&str> = out.lines().collect();

        // The code, on its own line, and the message under the error mark.
        assert!(lines.iter().any(|l| l.trim() == code), "{code}: code line in\n{out}");
        assert!(out.contains(&format!("× {}", d.message)), "{code}: message in\n{out}");
        // The file name with the line and column of the span start.
        let (line_no, col) = {
            let before = &src[..d.span().start];
            (before.matches('\n').count() + 1, before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1)
        };
        assert!(out.contains(&format!("[model.msl:{line_no}:{col}]")), "{code}: location in\n{out}");

        // The source line, then, under the span, an underline as long as the span.
        let src_line = src.lines().nth(line_no - 1).unwrap();
        let at = lines
            .iter()
            .position(|l| l.contains("│") && l.ends_with(src_line))
            .unwrap_or_else(|| panic!("{code}: source line in\n{out}"));
        let start_in_line = lines[at].chars().count() - src_line.chars().count() + (col - 1);
        let underline: Vec<char> = lines[at + 1].chars().collect();
        assert!(underline.contains(&'·'), "{code}: an underline row follows the source line:\n{out}");
        let marks = |c: &char| matches!(c, '─' | '┬' | '┴' | '┼');
        let first = underline.iter().position(marks).unwrap_or_else(|| panic!("{code}: underline in\n{out}"));
        assert_eq!(first, start_in_line, "{code}: the underline starts at the span:\n{out}");
        let run = underline[first..].iter().take_while(|c| marks(c)).count();
        assert_eq!(run, d.span().len(), "{code}: the underline is the span's length:\n{out}");
        // The label text hangs off the underline.
        assert!(lines[at + 2].contains(&d.primary.1), "{code}: label text in\n{out}");

        // The help line.
        let help = d.help.as_deref().unwrap_or_else(|| panic!("{code}: this diagnostic names its fix"));
        assert!(lines.iter().any(|l| l.trim() == format!("help: {help}")), "{code}: help in\n{out}");
    }
}
