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
const CHAINS: &[&str] = &["unary minus", "closure chain", "return chain", "generic angle"];

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
    // MSLP-04 AC 6 says "nesting", not "delimiters": prefix, closure,
    // `return` and generic nesting past 256 levels give
    // E0910 and no stack overflow, parse and print, on an 8 MiB stack.
    let handle = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            for kind in CHAINS {
                let deep = parse(&chain_source(kind, 20000));
                assert!(
                    deep.diagnostics.iter().any(|d| d.code == "E0910"),
                    "{kind}: no E0910 at 20000: {:?}",
                    deep.diagnostics.iter().map(|d| &*d.code).collect::<Vec<_>>()
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
    parse(src)
        .diagnostics
        .iter()
        .map(|d| (&*Box::leak(d.code.to_string().into_boxed_str()), d.span()))
        .collect()
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

    // The fit rests on chumsky's `stacker` feature (heap-grown stack segments,
    // allowed by AC 7), so the manifest declares it explicitly rather than by
    // default...
    let manifest = include_str!("../Cargo.toml");
    let chumsky = manifest.lines().find(|l| l.starts_with("chumsky")).expect("chumsky dependency");
    assert!(chumsky.contains("\"stacker\""), "stacker is declared: {chumsky}");
    // ...and no thread is spawned by any MSL source: none names a stack size.
    fn sources(dir: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("src/msl is readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(std::fs::read_to_string(&path).expect("source"));
            }
        }
    }
    let mut all = Vec::new();
    sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/msl"), &mut all);
    assert!(all.len() >= 10, "{} sources scanned", all.len());
    assert!(
        all.iter().all(|s| !s.contains("stack_size") && !s.contains("thread::")),
        "the parser runs on the caller's thread"
    );
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
fn an_on_every_inside_a_reaction_is_a_placement_in_a_reaction_body_e0350() {
    // mixed~98e4dc1b of the Book (chapter 9, part III §8.3.4): `on every(..) { .. }` written inside
    // another reaction places the event module `every` there (E0350); any other `on` in a reaction
    // stays E0404
    let src = "digital for M { on at(1u) { on every(0.0, 10u) { } } }";
    let found = codes(src);
    let at = src.find("on every(0.0, 10u) { }").expect("fixture holds its statement");
    assert_eq!(found, [("E0350", Span::new(at, at + "on every(0.0, 10u) { }".len()))], "{found:?}");
    let src = "digital for M { on at(1u) { on clk.rise() { } } }";
    assert!(codes(src).iter().all(|(code, _)| *code == "E0404"), "{:?}", codes(src));
    // the call anywhere in the event expression places the module: a conjunct, a parenthesis
    for event in ["every(0.0, 10u) && en", "en && every(0.0, 10u)", "(every(0.0, 10u))"] {
        let src = format!("digital for M {{ on at(1u) {{ on {event} {{ }} }} }}");
        let found = codes(&src);
        let at = src.find("on every").or_else(|| src.find("on en")).or_else(|| src.find("on (")).expect("fixture holds its statement");
        let end = at + format!("on {event} {{ }}").len();
        assert_eq!(found, [("E0350", Span::new(at, end))], "{src}: {found:?}");
    }
    let src = "digital for M { on at(1u) { on a && b { } } }";
    assert!(codes(src).iter().all(|(code, _)| *code == "E0404"), "{:?}", codes(src));
}

#[test]
fn an_emit_statement_in_a_module_body_reports_e0601() {
    // values~64ee8d1d of the Book (chapter 5, part VI §19 rule 10): `emit!` written where only
    // items go is E0601 at the statement, not a syntax error; the items after it still parse
    let src = "module M() { emit!(\"hello\"); let x = 1.0; }";
    let found = codes(src);
    let at = src.find("emit!(\"hello\");").expect("fixture holds its statement");
    assert_eq!(found, [("E0601", Span::new(at, at + "emit!(\"hello\");".len()))], "{found:?}");
}

#[test]
fn an_emit_statement_at_the_top_of_a_file_reports_e0601() {
    // part VI §19 rule 10 holds for every position: written among the file's items it is E0601 as
    // in a module body, and the items after it still parse
    let src = "emit!(\"hello\");\nmodule M() { }";
    let found = codes(src);
    let at = src.find("emit!(\"hello\");").expect("fixture holds its statement");
    assert_eq!(found, [("E0601", Span::new(at, at + "emit!(\"hello\");".len()))], "{found:?}");
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
        fixture("E0602", "fn f() { emit!(\"s\", ", "a: 1", "); }"),
        fixture("E0611", "const A: &'static str = include_str!(", "1", ");"),
        // Appendix A
        fixture("E0900", "fn f( ", "{", " }"),
        fixture("E0900", "fn f() { let x = 1 ", "}", ""),
        fixture("E0900", "fn f() {} ", "/*", " never closed"),
        fixture("E0901", "fn f() { ", "println!(\"x\")", "; }"),
        fixture("E0902", "const A: f64 = ", "1e3m", ";"),
        fixture("E0903", "fn ", "out", "() {}"),
        // The row's other half: a Rust keyword no production accepts.
        fixture("E0903", "", "type", " A = u8;"),
        fixture("E0903", "", "static", " X: u8 = 1;"),
        fixture("E0903", "fn f() { ", "yield", " 1; }"),
        fixture("E0903", "fn f() { let ", "self", " = 1; }"),
        // An SI letter with letters that are no type suffix: an unknown suffix.
        fixture("E0900", "const A: f64 = ", "1meg", ";"),
        fixture("E0905", "const A: u8 = ", "'a'", ";"),
        fixture("E0909", "const A: f64 = ", "1e999", ";"),
        fixture("E0907", "fn f() { let x = a ", "+>", " b; }"),
        fixture("E0907", "fn f() { g(a ", "<-", " b); }"),
        fixture("E0908", "fn f() { let ", "Unknown", " = 1; }"),
        fixture("E0908", "module M() { ", "Unknown", ": f64 = 1.0; }"),
        fixture("E0908", "fn f() { S { ", "Unknown", ": 1 }; }"),
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

#[test]
fn keyword_diagnostics_cover_both_halves_of_e0903() {
    // Appendix A row E0903: "a keyword used as an identifier, or a Rust
    // keyword where no production accepts it". Every unused Rust keyword,
    // and the path keywords as bindings.
    for kw in ["type", "static", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen"] {
        let src = format!("fn f() {{ {kw} 1; }}");
        let found = codes(&src);
        assert_eq!(found.first(), Some(&("E0903", Span::new(9, 9 + kw.len()))), "{kw}: {found:?}");
    }
    for (src, at, len) in [
        ("fn f() { let self = 1; }", 13, 4),
        ("fn f() { let Self = 1; }", 13, 4),
        ("fn f() { let super = 1; }", 13, 5),
        ("fn f() { let crate = 1; }", 13, 5),
        ("fn f() { for self in x {} }", 13, 4),
        ("fn f() { let (a, self) = t; }", 17, 4),
    ] {
        let found = codes(src);
        assert_eq!(found, vec![("E0903", Span::new(at, at + len))], "{src:?}");
    }
    // As a path segment or a constructor the same words stay valid.
    let r = parse("impl S { fn f(self) -> Self { Self { a: self.a } } fn g() -> u8 { crate::x + super::y } }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn unclosed_delimiter_is_reported_at_the_opener() {
    // The fault is the opening delimiter, not the end of the file: the
    // primary label sits on it, a second label on what met it first.
    let src = "fn f() { g(1, 2; h(3); }";
    let r = parse(src);
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
    let d = &r.diagnostics[0];
    assert_eq!((&*d.code, d.span()), ("E0900", Span::new(10, 11)));
    assert_eq!(d.secondary.first().map(|(s, _)| *s), Some(Span::new(23, 24)), "{d:?}");
    assert!(d.message.contains("expected `)`"), "{}", d.message);

    // At the end of input the second label is the end.
    let r = parse("fn f() { let x = 1;");
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(r.diagnostics[0].span(), Span::new(7, 8));
    assert!(r.diagnostics[0].message.starts_with("unexpected end of input, expected `}`"));

    // Source order: the unclosed `[` of the first function precedes the later
    // function's own error, though the parser ran to the end of the file.
    let src = "fn f() { let x = [1, 2; let y = 3; }\nfn g() { let = 1; }";
    let found = codes(src);
    let at = src.find('[').unwrap();
    assert_eq!(found.first(), Some(&("E0900", Span::new(at, at + 1))), "{found:?}");
    let later = src.find("= 1; }").unwrap();
    assert!(found.contains(&("E0900", Span::new(later, later + 1))), "{found:?}");
    assert!(found.windows(2).all(|w| w[0].1.start <= w[1].1.start));
    assert_eq!(found.len(), 2, "{found:?}");
}

#[test]
fn one_fault_is_one_diagnostic() {
    // No second error after the end of input for a fault already reported.
    for src in [
        "fn f() { let s = \"abc",
        "fn f() {\n/* abc\n}",
        "fn a() {}\nfn f() { let s = \"abc\nfn g() {}",
    ] {
        let found = codes(src);
        assert_eq!(found.len(), 1, "{src:?}: {found:?}");
    }
    // A generation `if` in a digital block takes an `else`; the statement in
    // it is E0434 alone.
    let src = "digital for M { if c { on e {} } else { y = 2; } }";
    let at = src.find("y = 2;").unwrap();
    assert_eq!(codes(src), vec![("E0434", Span::new(at, at + 6))]);
}

#[test]
fn comparisons_do_not_chain() {
    // Rust 2024 makes `==`, `!=`, `<`, `<=`, `>`, `>=` one non-associative
    // level: a chain is an error, a parenthesized one is not.
    for src in ["a < b < c", "a == b == c", "a <= b > c", "a != b < c", "a as u8 < b"] {
        let (_, diags) = parse_expr(src);
        assert!(!diags.is_empty() && diags.iter().all(|d| d.code == "E0900"), "{src:?}: {diags:?}");
    }
    for src in ["(a < b) == (c < d)", "a == b && c < d", "a < b || c > d", "a as u8 > b as u8"] {
        let (_, diags) = parse_expr(src);
        assert!(diags.is_empty(), "{src:?}: {diags:?}");
    }
}

#[test]
fn raw_identifier_of_another_word_is_e0900() {
    // POL-10 AC 2: `r#` before any word but the seven added keywords is one
    // E0900 that lists the seven; `r#out` itself is accepted.
    for word in ["type", "in", "foo"] {
        let src = format!("const X: u8 = r#{word};");
        let r = parse(&src);
        assert_eq!(r.diagnostics.len(), 1, "{src}: {:?}", r.diagnostics);
        let d = &r.diagnostics[0];
        assert_eq!(d.code, "E0900", "{src}");
        for seven in ["module", "nature", "signal", "on", "out", "flip", "after"] {
            assert!(d.message.contains(seven), "{src}: {}", d.message);
        }
        let at = src.find("r#").unwrap();
        assert_eq!(d.span().start, at, "{src}");
    }
    let ok = parse("const r#out: u8 = 1;");
    assert!(ok.diagnostics.is_empty(), "{:?}", ok.diagnostics);
}

#[test]
fn dangling_doc_comment_has_its_own_message() {
    // POL-12 AC 2: a doc comment followed by `}` or the end of input is one
    // E0900 labelled at the comment, saying it documents nothing.
    for (src, comment) in [
        ("fn f() { let x = 1; /// nothing\n}", "/// nothing"),
        ("fn f() { /// only\n}", "/// only"),
        ("const X: u8 = 1;\n/// nothing", "/// nothing"),
        ("module M() { signal a: u8; /// last\n}", "/// last"),
    ] {
        let r = parse(src);
        assert_eq!(r.diagnostics.len(), 1, "{src:?}: {:?}", r.diagnostics);
        let d = &r.diagnostics[0];
        assert_eq!(d.code, "E0900", "{src:?}");
        assert!(d.message.contains("documents nothing"), "{src:?}: {}", d.message);
        let at = src.find(comment).unwrap();
        assert_eq!(d.span().start, at, "{src:?}");
        assert_eq!(d.span().end, at + comment.len(), "{src:?}");
    }
}

/// The diagnostics of `src` as (code, text the span covers, message).
fn faults(src: &str) -> Vec<(String, String, String)> {
    parse(src)
        .diagnostics
        .iter()
        .map(|d| (d.code.to_string(), src[d.span().start..d.span().end].to_string(), d.message.clone()))
        .collect()
}

#[test]
fn commit_target_must_be_a_place() {
    // POL-14 AC 1: eight non-places (a tuple index is none: B.3.2 `place` has `.IDENT` only) give one E0900 over the left side; four places are clean.
    for lhs in ["-q", "a + b", "f(q)", "q.x()", "*q", "q as u8", "q.0", "a.b.1"] {
        let src = format!("digital for M {{ on e {{ {lhs} <- 1; }} }}");
        let found = faults(&src);
        assert_eq!(found.len(), 1, "{lhs}: {found:?}");
        assert_eq!((found[0].0.as_str(), found[0].1.as_str()), ("E0900", lhs), "{lhs}");
        assert!(found[0].2.starts_with("expected a place"), "{}", found[0].2);
        for named in ["carrier", "`x[i]`", "`x.f`"] {
            assert!(found[0].2.contains(named), "{named} in {}", found[0].2);
        }
        // The commit stays in the tree, its left side as written.
        let file = parse(&src).file;
        let piperine_lang::msl::ast::ItemKind::Behavior(b) = &file.items[0].kind else { panic!("{file:?}") };
        let piperine_lang::msl::ast::BehaviorBody::Digital(items) = &b.body else { panic!() };
        let piperine_lang::msl::ast::DigitalItemKind::Reaction(r) = &items[0].kind else { panic!("{items:?}") };
        assert!(
            matches!(&r.body.stmts[0].kind, piperine_lang::msl::ast::StmtKind::Commit { place, .. } if place.span.0.end - place.span.0.start == lhs.len()),
            "{lhs}: {:?}",
            r.body.stmts
        );
    }
    for lhs in ["q", "q[0]", "st.n", "a.b[2].c"] {
        let src = format!("digital for M {{ on e {{ {lhs} <- 1; }} }}");
        assert!(faults(&src).is_empty(), "{lhs}: {:?}", faults(&src));
    }
}

#[test]
fn parenthesized_place_is_rejected_in_both_forms() {
    // POL-14 AC 2: a definition and a commit alike.
    for src in ["module M() { (q) = 1; }", "digital for M { on e { (q) <- 1; } }"] {
        let found = faults(src);
        assert_eq!(found.len(), 1, "{src}: {found:?}");
        assert_eq!((found[0].0.as_str(), found[0].1.as_str()), ("E0900", "(q)"), "{src}");
        assert!(found[0].2.starts_with("expected a place"), "{}", found[0].2);
    }
    assert!(faults("module M() { q.f[1] = 1; }").is_empty());
    // The definition rejects a tuple index too, so both forms agree.
    assert_eq!(faults("module M() { q.0 = 1; }").len(), 1);
}

/// Exactly one diagnostic: its code, the text its span covers, whether it has a help.
fn only(src: &str) -> (String, String, bool) {
    let r = parse(src);
    assert_eq!(r.diagnostics.len(), 1, "{src:?}: {:?}", r.diagnostics);
    let d = &r.diagnostics[0];
    (d.code.to_string(), src[d.span().start..d.span().end].to_string(), d.help.is_some())
}

#[test]
fn signal_outside_a_module_body_is_e0404() {
    // POL-14 AC 3: `signal` in `analog`, a reaction or a closure outside a function.
    for src in [
        "analog for M { signal t: E; }",
        "digital for M { on e { signal t: E; } }",
        "module M() { let f = || { signal t: E; }; }",
    ] {
        assert_eq!(only(src), ("E0404".into(), "signal t: E;".into(), true), "{src}");
    }
}

#[test]
fn behavior_statements_in_a_module_body_are_e0404() {
    // POL-14 AC 4: one diagnostic each, over the statement, with a help.
    for stmt in ["q <- 1;", "i(a) +> 1.0;", "on e { }", "bound_step(1.0);", "q.0 <- 1;"] {
        let src = format!("module M() {{ {stmt} }}");
        assert_eq!(only(&src), ("E0404".into(), stmt.into(), true), "{src}");
    }
    // A module body that is only items stays clean.
    assert!(faults("module M() { signal s: Bit; q = 1; }").is_empty());
}

#[test]
fn function_body_reports_e0121_only() {
    // POL-14 AC 5: the five forms in a `fn` body, closures inside it included.
    for stmt in ["x +> 1.0;", "q <- 1;", "on e { }", "signal s: E;", "bound_step(1.0);"] {
        for src in [format!("fn f() {{ {stmt} }}"), format!("fn f() {{ let g = || {{ {stmt} }}; }}")] {
            let found = faults(&src);
            assert_eq!(found.len(), 1, "{src}: {found:?}");
            assert_eq!(found[0].0, "E0121", "{src}");
        }
    }
}

#[test]
fn pub_on_a_behavior_block_is_e0900() {
    // POL-14 AC 6: one E0900 at `pub` with a help; the block is still a Behavior item.
    for src in ["pub analog for M { }", "pub digital for M { }"] {
        let r = parse(src);
        assert_eq!(r.diagnostics.len(), 1, "{src}: {:?}", r.diagnostics);
        let d = &r.diagnostics[0];
        assert_eq!(d.code, "E0900");
        assert_eq!(d.span(), Span::new(0, 3), "{src}");
        assert!(d.help.as_deref().is_some_and(|h| h.contains("visibility")), "{:?}", d.help);
        assert_eq!(r.file.items.len(), 1, "{src}");
        assert!(matches!(r.file.items[0].kind, piperine_lang::msl::ast::ItemKind::Behavior(_)), "{src}");
    }
    assert!(faults("analog for M { }").is_empty());
}

#[test]
fn forms_outside_the_grammar_are_parse_errors() {
    // POL-14 AC 7: one E0900 for each.
    for src in ["nature A { }", "module M() { signal g: (Bit, Bit); }", "module M() { signal h: &Bit; }"] {
        let found = faults(src);
        assert_eq!(found.len(), 1, "{src}: {found:?}");
        assert_eq!(found[0].0, "E0900", "{src}");
    }
    // The same in a block statement.
    assert_eq!(faults("analog for M { signal h: &Bit; }").iter().filter(|f| f.0 == "E0900").count(), 1);
    // A port of value type `(Bit, Bit)` is unaffected: only `signal` uses a sigtype.
    assert!(faults("module M(a: (Bit, Bit)) { }").is_empty());
    // Carrier types stay clean.
    assert!(faults("module M() { signal a: Bit; signal b: flip Stream<T>; signal c: [Bit; 4]; }").is_empty());
}

// ---- the removed Rust forms of B.3.6 (POL-09 AC 5 to 7) ---------------------------------------

/// One removed form: the start of its B.3.6 first cell, a source that uses it
/// once, the code of its row, a phrase its message holds (it names the form)
/// and a phrase its help holds (it names what to write instead).
struct Removed {
    row: &'static str,
    src: &'static str,
    code: &'static str,
    message: &'static str,
    help: &'static str,
}

const fn removed(row: &'static str, src: &'static str, code: &'static str, message: &'static str, help: &'static str) -> Removed {
    Removed { row, src, code, message, help }
}

const REMOVED: &[Removed] = &[
    removed("type aliases", "type T = f64;", "E0903", "type aliases are not part of the language", "write the type out"),
    removed("associated types", "trait Tr { type X; }", "E0903", "associated types are not part of the language", "name the type where it is used"),
    removed("associated types", "impl Tr for S { type X = u8; }", "E0903", "associated types are not part of the language", "name the type where it is used"),
    removed("`static` items", "static X: u8 = 1;", "E0903", "`static` items are not part of the language", "`const`"),
    removed("trait-bound `where` clauses", "fn f<T>(x: T) where T: Tr { }", "E0900", "trait-bound `where` clauses are not part of the language", "<T: Tr>"),
    removed("`impl Trait` types", "fn f(x: impl Tr) { }", "E0900", "`impl Trait` types are not part of the language", "<T: Tr>"),
    removed("qualified paths", "fn f() { let x: <T as Tr>::X = 1; }", "E0900", "qualified paths are not part of the language", "T::X"),
    removed("qualified paths", "fn f() { let x = <T as Tr>::g(); }", "E0900", "qualified paths are not part of the language", "T::X"),
    removed("restricted visibility", "pub(crate) fn f() { }", "E0900", "restricted visibility is not part of the language", "`pub`"),
    removed("restricted visibility", "pub(super) struct S;", "E0900", "restricted visibility is not part of the language", "`pub`"),
    removed("loop labels", "fn f() { 'a: loop { } }", "E0905", "loop labels are not part of the language", "restructure the loop"),
    removed("loop labels", "fn f() { loop { break 'a; } }", "E0905", "loop labels are not part of the language", "restructure the loop"),
    removed("`&self` and `&mut self` receivers", "impl S { fn h(&self) { } }", "E0900", "`&self` and `&mut self` receivers are not part of the language", "write `self`"),
    removed("`&self` and `&mut self` receivers", "impl S { fn h(&mut self) { } }", "E0900", "`&self` and `&mut self` receivers are not part of the language", "write `self`"),
    removed("`mut` and pattern parameters", "fn f(mut x: u8) { }", "E0900", "`mut` and pattern parameters are not part of the language", "a plain name"),
    removed("`mut` and pattern parameters", "fn f((a, b): (u8, u8)) { }", "E0900", "`mut` and pattern parameters are not part of the language", "a plain name"),
    removed("half-open range patterns", "fn f() { match x { ..5 => { } _ => { } } }", "E0900", "half-open range patterns are not part of the language", "`..=x`"),
    removed("a leading `::`", "fn f() { let x = ::a::b; }", "E0900", "a leading `::` is not part of the language", "a path from a name in scope"),
    removed("a leading `::`", "use ::a::b;", "E0900", "a leading `::` is not part of the language", "a path from a name in scope"),
    removed("a leading `::`", "use ::a::{b, c};", "E0900", "a leading `::` is not part of the language", "a path from a name in scope"),
    removed("trait-bound `where` clauses", "struct S<T> where T: Tr { a: T }", "E0900", "trait-bound `where` clauses are not part of the language", "<T: Tr>"),
    removed("trait-bound `where` clauses", "impl<T> S<T> where T: Tr { }", "E0900", "trait-bound `where` clauses are not part of the language", "<T: Tr>"),
    removed("trait-bound `where` clauses", "impl<T> Tr for S<T> where T: Tr { }", "E0900", "trait-bound `where` clauses are not part of the language", "<T: Tr>"),
    removed("trait-bound `where` clauses", "trait Tr<T> where T: U { }", "E0900", "trait-bound `where` clauses are not part of the language", "<T: Tr>"),
    removed("trait-bound `where` clauses", "enum E<T> where T: Tr { A }", "E0900", "trait-bound `where` clauses are not part of the language", "<T: Tr>"),
    removed("named function-pointer parameters", "fn f(p: fn(x: f64) -> f64) { }", "E0900", "named function-pointer parameters are not part of the language", "`fn(f64) -> f64`"),
    removed("named function-pointer parameters", "fn f() -> fn(x: f64) { }", "E0900", "named function-pointer parameters are not part of the language", "`fn(f64) -> f64`"),
    removed("attributes and doc comments on a parameter", "fn f(#[a] x: f64) { }", "E0900", "attributes and doc comments on parameters", "on the item"),
    removed("attributes and doc comments on a parameter", "fn f(/// x\n y: f64) { }", "E0900", "attributes and doc comments on parameters", "on the item"),
    removed("attributes and doc comments on a parameter", "fn f<#[a] T>() { }", "E0900", "attributes and doc comments on parameters", "on the item"),
    removed("attributes and doc comments on a parameter", "fn f<\n/// x\nT>() { }", "E0900", "attributes and doc comments on parameters", "on the item"),
    removed("attributes and doc comments on a parameter", "fn f() { S { #[a] x: 1 }; }", "E0900", "attributes and doc comments on parameters", "on the item"),
    removed("attributes and doc comments on a parameter", "fn f() { S { /// d\n x: 1 }; }", "E0900", "attributes and doc comments on parameters", "on the item"),
    removed("double borrows", "fn f() { g(&&a); }", "E0900", "double borrows are not part of the language", "`&a`"),
    removed("double borrows", "fn f() { let x = &&a; }", "E0900", "double borrows are not part of the language", "`&a`"),
    removed("inner attributes", "#![allow(x)] fn f() { }", "E0900", "inner attributes are not part of the language", "an outer attribute"),
    removed("the never type", "fn f() -> ! { }", "E0900", "the never type is not part of the language", "a unit return type"),
    removed("raw-pointer types", "fn f(p: *const u8) { }", "E0900", "raw-pointer types are not part of the language", "a value or an array"),
    removed("raw-pointer types", "fn f(p: *mut u8) { }", "E0900", "raw-pointer types are not part of the language", "a value or an array"),
    removed("`union` items", "union U { a: u8 }", "E0900", "`union` items are not part of the language", "`struct`"),
    removed("`extern` blocks", "extern \"C\" { fn f(); }", "E0900", "`extern` blocks are not part of the language", "`extern \"C\" fn`"),
    removed("`extern crate` items", "extern crate util;", "E0900", "`extern crate` is not part of the language", "[dependencies]"),
];

#[test]
fn removed_rust_forms_report_targeted_messages() {
    // POL-09 AC 5: one diagnostic per use, the code of its row, a message that
    // names the form and a help that names what to write instead.
    for r in REMOVED {
        let parsed = parse(r.src);
        assert_eq!(parsed.diagnostics.len(), 1, "{}: {:?}", r.src, parsed.diagnostics);
        let d = &parsed.diagnostics[0];
        assert_eq!(d.code, r.code, "{}", r.src);
        assert!(d.message.contains(r.message), "{}: message {:?} lacks {:?}", r.src, d.message, r.message);
        assert!(d.help.as_deref().is_some_and(|h| h.contains(r.help)), "{}: help {:?} lacks {:?}", r.src, d.help, r.help);
    }
    // The existing spans of `type` and `static` hold: the keyword.
    assert_eq!(codes("type A = u8;"), [("E0903", Span::new(0, 4))]);
    assert_eq!(codes("static X: u8 = 1;"), [("E0903", Span::new(0, 6))]);
    // Forms next to a removed one stay accepted.
    for src in [
        "impl S { fn h(self) { } }",
        "pub fn f() { }",
        "fn f(x: u8) -> f64 { 1.0 }",
        "fn f() { match x { 1..=5 => { } _ => { } } }",
        "fn f() { let x = a::b; }",
        "#[derive(Repr)] struct S;",
        "extern \"C\" fn f();",
        "fn f<T: Tr>(x: T) { }",
        "fn f() { let a = 1 - 1; let g = x * 2; }",
        "fn f() { let a = x && &y; g(&a); }",
        "fn f(p: fn(f64) -> f64) { }",
    ] {
        let found = faults(src);
        assert!(found.is_empty(), "{src}: {found:?}");
    }
}

#[test]
fn every_b36_row_has_a_fixture() {
    // POL-09 AC 6: one fixture at least per row of B.3.6, matched by the first cell.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/appendix_b_grammar.md");
    let text = std::fs::read_to_string(path).expect("appendix B is UTF-8");
    let sec = text.split("### B.3.6").nth(1).and_then(|s| s.split("\n## ").next()).expect("B.3.6");
    let cells: Vec<&str> = sec
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Form") && !l.starts_with("| ---"))
        .filter_map(|l| l.trim_matches('|').split('|').next())
        .map(str::trim)
        .collect();
    assert_eq!(cells.len(), 21, "{cells:?}");
    for cell in &cells {
        assert!(REMOVED.iter().any(|r| cell.starts_with(r.row)), "B.3.6 row `{cell}` has no fixture");
    }
    for r in REMOVED {
        assert!(cells.iter().any(|c| c.starts_with(r.row)), "fixture `{}` matches no row", r.row);
    }
}

#[test]
fn body_let_annotation_has_its_own_message() {
    // POL-09 AC 7: one E0900 that says a body `let` takes no type annotation.
    let src = "module M() { let y: f64 = 1.0; }";
    let found = faults(src);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "E0900");
    assert!(found[0].2.contains("body `let` takes no type annotation"), "{}", found[0].2);
    // A `let` in a function keeps its annotation.
    assert!(faults("fn f() { let y: f64 = 1.0; }").is_empty());
}

/// A flat chain of `n` links, as `flat_source` builds it.
const FLAT: &[&str] = &["sum", "method calls", "casts", "else-if arms", "try"];

fn flat_source(kind: &str, n: usize) -> String {
    match kind {
        "sum" => format!("fn f() -> f64 {{ 1{} }}", "+1".repeat(n - 1)),
        "method calls" => format!("fn f() -> f64 {{ a{} }}", ".g()".repeat(n)),
        "casts" => format!("fn f() -> f64 {{ 1 {}}}", "as f64 ".repeat(n)),
        "else-if arms" => format!("fn f() {{ if a {{}} {}}}", "else if a {} ".repeat(n)),
        _ => format!("fn f() -> f64 {{ a{} }}", "?".repeat(n)),
    }
}

#[test]
fn flat_chains_up_to_4096_links_are_not_nesting() {
    // POL-15 AC 2: 4096 links parse clean; print, build and to_ast finish on
    // an 8 MiB debug stack.
    let handle = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            for kind in FLAT {
                let src = flat_source(kind, 4096);
                let r = parse(&src);
                assert!(r.diagnostics.is_empty(), "{kind}: {:?}", r.diagnostics.iter().take(2).collect::<Vec<_>>());
                let printed = piperine_lang::msl::print(&r.file);
                assert!(printed.len() > 4096, "{kind}");
                let design = piperine_lang::pom::Design::builder().source("f.msl", src.as_str()).build();
                for file in design.files() {
                    let _ = file.to_ast();
                }
            }
        })
        .expect("spawn the 8 MiB test thread");
    handle.join().expect("no stack overflow, no panic");
}

#[test]
fn flat_chain_past_4096_links_is_e0910() {
    // POL-15 AC 3: 4097 and 20000 links give exactly one E0910 naming the
    // bound; the chain inside 10 nested parentheses is summed along its path.
    let handle = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            for n in [4097, 20000] {
                // `n` links: `n` plus signs, so the sum has `n + 1` terms.
                let r = parse(&flat_source("sum", n + 1));
                let e: Vec<_> = r.diagnostics.iter().filter(|d| d.code == "E0910").collect();
                assert_eq!(e.len(), 1, "{n} terms: {:?}", r.diagnostics);
                assert!(e[0].message.contains("4096"), "{}", e[0].message);
                assert_eq!(r.diagnostics.len(), 1, "{n} terms: {:?}", r.diagnostics);
                let _ = piperine_lang::msl::print(&r.file);
            }
            let nested = format!("fn f() -> f64 {{ {}1{}{} }}", "(".repeat(10), "+1".repeat(4097), ")".repeat(10));
            let r = parse(&nested);
            assert_eq!(r.diagnostics.iter().filter(|d| d.code == "E0910").count(), 1, "{:?}", r.diagnostics);
        })
        .expect("spawn");
    handle.join().expect("no stack overflow, no panic");
}

#[test]
fn bad_where_list_is_one_diagnostic() {
    // POL-16 AC 1: a comma in a `where` is one diagnostic, labelled at the
    // comma; the module keeps its name and its generic.
    let src = "module M<const N: u32>() where N > 0, N < 5 { }";
    let r = parse(src);
    let comma = src.find(',').unwrap();
    let found: Vec<_> = r.diagnostics.iter().map(|d| (&*d.code, d.span())).collect();
    assert_eq!(found, vec![("E0900", Span::new(comma, comma + 1))], "{:?}", r.diagnostics);
    assert_eq!(r.file.items.len(), 1);
    let piperine_lang::msl::ast::ItemKind::Module(m) = &r.file.items[0].kind else {
        panic!("M is a Module item, not {:?}", r.file.items[0].kind)
    };
    assert_eq!(m.name.name, "M");
    let generics = m.generics.as_ref().expect("the generic list is kept");
    assert_eq!(generics.params.len(), 1);
    assert_eq!(generics.params[0].name.name, "N");

    // An item keyword inside a generic list is not a place to resume: a bad
    // default is one diagnostic and the next item parses.
    let src = "module M<const N: u32 = , const K: u32>() { }\nfn g() {}";
    let r = parse(src);
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
    assert!(r.file.items.iter().any(|i| matches!(&i.kind, piperine_lang::msl::ast::ItemKind::Fn(f) if f.name.name == "g")));
}

#[test]
fn statement_errors_recover_inside_the_block() {
    // POL-16 AC 2: a bad statement is skipped to its `;`; the block, its
    // item and the statements after it survive.
    use piperine_lang::msl::ast::{ExprKind, ItemKind, StmtKind};
    let src = "fn f() { let a = ; foo(; let c = 3; }";
    let r = parse(src);
    let first_semi = src.find(';').unwrap();
    assert!(
        r.diagnostics.iter().any(|d| d.span().start == first_semi),
        "a diagnostic at the first `;`: {:?}",
        r.diagnostics
    );
    let let_c = src.find("let c").unwrap();
    assert!(
        r.diagnostics.iter().all(|d| !(d.span().start >= let_c && d.span().start < let_c + "let c = 3;".len())),
        "no diagnostic labels `let c = 3;`: {:?}",
        r.diagnostics
    );
    assert_eq!(r.file.items.len(), 1, "{:?}", r.file.items);
    let ItemKind::Fn(f) = &r.file.items[0].kind else { panic!("{:?}", r.file.items[0].kind) };
    let body = f.body.as_ref().expect("the function keeps its body");
    let lets: Vec<_> = body.stmts.iter().filter(|s| matches!(s.kind, StmtKind::Let { init: Some(_), .. })).collect();
    assert_eq!(lets.len(), 1, "{:?}", body.stmts);
    assert_eq!(lets[0].span.0, Span::new(let_c, let_c + "let c = 3;".len()));
    // The statements that failed are error nodes, in place.
    let errors = body
        .stmts
        .iter()
        .filter(|s| matches!(&s.kind, StmtKind::Expr { expr, .. } if matches!(expr.kind, ExprKind::Error)))
        .count();
    assert!(errors >= 1, "{:?}", body.stmts);
}

#[test]
fn a_bad_port_keeps_the_module() {
    // POL-16 AC 3: one bad port is one diagnostic, at the `,`; the module
    // keeps its other ports and its body, and the bad port is not invented.
    use piperine_lang::msl::ast::{ItemKind, ModuleItemKind};
    let src = "module M(in a: , b: Bit) { signal s: Bit; }";
    let r = parse(src);
    let comma = src.find(',').unwrap();
    let found: Vec<_> = r.diagnostics.iter().map(|d| (&*d.code, d.span())).collect();
    assert_eq!(found, vec![("E0900", Span::new(comma, comma + 1))], "{:?}", r.diagnostics);
    assert_eq!(r.file.items.len(), 1);
    let ItemKind::Module(m) = &r.file.items[0].kind else { panic!("{:?}", r.file.items[0].kind) };
    assert_eq!(m.ports.iter().map(|p| p.name.name.as_str()).collect::<Vec<_>>(), ["b"]);
    assert!(m.body.iter().any(|i| matches!(&i.kind, ModuleItemKind::Signal { name, .. } if name.name == "s")), "{:?}", m.body);

    // The last port bad, and a bad port in the middle of three.
    let r = parse("module M(a: Bit, b: ) { }");
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
    let ItemKind::Module(m) = &r.file.items[0].kind else { panic!() };
    assert_eq!(m.ports.iter().map(|p| p.name.name.as_str()).collect::<Vec<_>>(), ["a"]);
    let r = parse("module M(a: Bit, in : Bit, c: Bit) { }");
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
    let ItemKind::Module(m) = &r.file.items[0].kind else { panic!() };
    assert_eq!(m.ports.iter().map(|p| p.name.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
}

#[test]
fn comparison_lists_are_not_nesting() {
    // POL-15 / D8: a `<` only counts as nesting when a `>` closes it as a
    // generic list; N comma-separated comparisons never do.
    let n = 1000;
    let cmp = |sep: &str| vec!["a < b"; n].join(sep);
    let sources = [
        format!("fn f() {{ let v = [{}]; }}", cmp(", ")),
        format!("fn f() {{ g({}); }}", cmp(", ")),
        format!("fn f() {{ let t = ({}); }}", cmp(", ")),
        format!("fn f() {{ let s = S {{ {} }}; }}", (0..n).map(|i| format!("a{i}: x < 1")).collect::<Vec<_>>().join(", ")),
        format!("fn f() {{ match x {{ {} }} }}", (0..n).map(|i| format!("{i} => a < b,")).collect::<String>()),
        format!("module M() {{ let v = [{}]; }}", cmp(", ")),
        format!("fn f() {{ let v = [{}]; }}", vec!["a < b, c > d"; n].join(", ")),
    ];
    for src in &sources {
        let r = parse(src);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics.iter().take(2).collect::<Vec<_>>());
    }
    // Real generic nesting is still bounded.
    let deep = format!("fn f() -> {}u8{} {{ 1 }}", "Option<".repeat(300), ">".repeat(300));
    assert!(parse(&deep).diagnostics.iter().any(|d| d.code == "E0910"));
}

#[test]
fn flat_chains_survive_a_default_2_mib_thread() {
    // POL-15 AC 2: the bound must hold on spawned threads, not only an 8 MiB
    // main stack: parse, build and print of a 4096-term sum.
    let handle = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| {
            for kind in FLAT {
                let src = flat_source(kind, 4096);
                let r = parse(&src);
                assert!(r.diagnostics.is_empty(), "{kind}: {:?}", r.diagnostics.iter().take(2).collect::<Vec<_>>());
                let printed = piperine_lang::msl::print(&r.file);
                assert!(printed.len() > 4096, "{kind}");
                let design = piperine_lang::pom::Design::builder().source("f.msl", src.as_str()).build();
                for file in design.files() {
                    let _ = file.to_ast();
                }
            }
        })
        .expect("spawn the 2 MiB test thread");
    handle.join().expect("no stack overflow, no panic");
}

#[test]
fn reserved_keyword_help_is_true_and_a_doc_before_else_documents_nothing() {
    // D15: an added keyword used as a name is told to write `r#word`; another
    // reserved word is not told raw identifiers are unsupported (A.1 rule 6).
    let help = |src: &str| {
        let r = parse(src);
        assert_eq!(r.diagnostics.len(), 1, "{src}: {:?}", r.diagnostics);
        assert_eq!(r.diagnostics[0].code, "E0903", "{src}");
        r.diagnostics[0].help.clone().unwrap_or_default()
    };
    let out = help("fn f() { let out = 1; }");
    assert!(out.contains("r#out"), "{out}");
    let yielded = help("fn f() { let yield = 1; }");
    assert!(!yielded.contains("not supported"), "{yielded}");
    assert!(yielded.contains("choose another name"), "{yielded}");
    // A doc comment before `else` documents nothing: one E0900, no keyword fault.
    let src = "fn f() { if a { } /// x\nelse { } }";
    let r = parse(src);
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(r.diagnostics[0].code, "E0900");
    assert!(r.diagnostics[0].message.contains("documents nothing"), "{}", r.diagnostics[0].message);
    assert_eq!(&src[r.diagnostics[0].span().start..r.diagnostics[0].span().end], "/// x");
}

#[test]
fn extern_crate_is_a_removed_form() {
    // PKG-01 AC 9: exactly one E0900 at `extern`, the item kept as an error, the rest of the file parsed.
    let parsed = parse("extern crate util;");
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let d = &parsed.diagnostics[0];
    assert_eq!(d.code, "E0900");
    assert!(d.message.contains("`extern crate` is not part of the language"), "{}", d.message);
    assert!(d.help.as_deref().is_some_and(|h| h.contains("[dependencies]")), "{:?}", d.help);
    assert_eq!(codes("extern crate util;"), [("E0900", Span::new(0, 6))]);
    assert_eq!(codes("extern crate util as u;\nfn f() { }"), [("E0900", Span::new(0, 6))]);
    // `extern "C" fn` and an `extern` block keep their own behaviour.
    assert!(faults("extern \"C\" fn f();").is_empty());
    assert_eq!(faults("extern \"C\" { fn f(); }").len(), 1);
}
