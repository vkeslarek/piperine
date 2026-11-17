//! Scope: the comment-preserving formatter of MSL (msl-cli CLI-23):
//! `piperine_lang::msl::comments` (the plain comments beside the tokens and
//! their anchors) and `piperine_lang::msl::format` (a formatted text that
//! keeps every comment, the tree and its own fixed point). Expected texts are
//! written by hand from the acceptance criterion each test cites.

use piperine_lang::msl::Span;
use piperine_lang::msl::comments::{Anchor, Comment, Comments};
use piperine_lang::msl::parser::parse;

/// The span from the first `from` of `src` to the end of the first `to` after it.
fn between(src: &str, from: &str, to: &str) -> Span {
    let start = src.find(from).unwrap_or_else(|| panic!("`{from}` is in the source"));
    let len = src[start..].find(to).unwrap_or_else(|| panic!("`{to}` follows `{from}`")) + to.len();
    Span::new(start, start + len)
}

/// The anchors of the comments of `src`, which must parse.
fn anchors(src: &str) -> (Vec<Anchor>, Vec<Span>) {
    let parsed = parse(src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let attached = Comments::of(src).attach(&parsed.file, src);
    (attached.anchors().to_vec(), attached.verbatim().to_vec())
}

/// CLI-23 AC 1 (C175): `Comments::of` lists a `//` line, a trailing `//` and
/// a `/* */`, each with its span, its text and whether it starts its line, and
/// leaves the `///`, `//!`, `/** */` and `/*! */` doc comments to the tokens.
#[test]
fn plain_comments_are_listed_with_span_text_and_line_start() {
    let src = "//! inner doc\n// own line\n/// outer doc\nfn f() {\n    let x = 1; // trailing\n    /* block */ x\n}\n/** block doc */\n/*! inner block doc */\n";
    let at = |needle: &str| {
        let start = src.find(needle).unwrap_or_else(|| panic!("{needle} is in the source"));
        Span::new(start, start + needle.len())
    };
    let expected = vec![
        Comment { span: at("// own line"), text: "// own line".to_string(), starts_line: true },
        Comment { span: at("// trailing"), text: "// trailing".to_string(), starts_line: false },
        Comment { span: at("/* block */"), text: "/* block */".to_string(), starts_line: true },
    ];
    assert_eq!(Comments::of(src).list(), expected.as_slice());
    assert_eq!(Comments::of(src).texts(), vec!["// own line", "// trailing", "/* block */"]);

    // Rust's rule for what is no doc comment: `////`, `/***` and the empty `/**/` are plain.
    let plain = "//// four\n/*** three */\n/**/\n";
    assert_eq!(Comments::of(plain).texts(), vec!["//// four", "/*** three */", "/**/"]);

    // A line comment ends before the `\r` of a CRLF line; a nested block comment is one comment.
    let crlf = "let a = 1; // crlf\r\n/* outer /* inner */ still */\r\n";
    let list = Comments::of(crlf);
    assert_eq!(list.texts(), vec!["// crlf", "/* outer /* inner */ still */"]);
    assert_eq!(list.list()[0].span, Span::new(11, 18));
    assert!(!list.list()[0].starts_line && list.list()[1].starts_line);
}

/// CLI-23 AC 4 to 6 (C180): an own-line comment is `Before` the item, member,
/// statement, arm or list element that comes next; a comment on a node's
/// last line is `After` it (after its `,` too); a comment after the last
/// child of a container, or alone in it, is `Inside` it, named by its
/// closing delimiter (the end of the file by the empty span at its length).
#[test]
fn a_comment_is_anchored_before_after_or_inside() {
    let src = "\
// before the item
module M(
    // before the element
    in a: Electrical,
    out b: Electrical, // after the element
) {
    // before the member
    r: f64 = 1.0; // after the member
}

analog for M {
    // alone in the block
}

fn f(x: i32) -> i32 {
    // before the statement
    let y = x; // after the statement
    match y {
        // before the arm
        0 => 1,
        _ => 2,
    }
    // ending the block
}
// ending the file
";
    let module = between(src, "module M(", "\n}");
    let analog_close = between(src, "analog for M", "}");
    let analog_close = Span::new(analog_close.end - 1, analog_close.end);
    let fn_close = Span::new(src.rfind('}').expect("a brace"), src.rfind('}').expect("a brace") + 1);
    let expected = vec![
        Anchor::Before(module),
        Anchor::Before(between(src, "in a", "Electrical")),
        Anchor::After(between(src, "out b", "Electrical")),
        Anchor::Before(between(src, "r: f64", ";")),
        Anchor::After(between(src, "r: f64", ";")),
        Anchor::Inside(analog_close),
        Anchor::Before(between(src, "let y", ";")),
        Anchor::After(between(src, "let y", ";")),
        Anchor::Before(between(src, "0 => 1", ",")),
        Anchor::Inside(fn_close),
        Anchor::Inside(Span::new(src.len(), src.len())),
    ];
    let (got, verbatim) = anchors(src);
    assert_eq!(got, expected);
    assert!(verbatim.is_empty(), "every comment of this source has an anchor: {verbatim:?}");
}

/// CLI-23 AC 7 (C184): a comment inside an expression is at no anchor, so the
/// statement holding it is `Verbatim`; the innermost member or statement is
/// chosen, and every comment inside a verbatim node is that node's, even one
/// that would have an anchor. Every comment has exactly one anchor, and each
/// `Verbatim` one lies in exactly one verbatim node.
#[test]
fn an_unreachable_comment_marks_its_statement_verbatim() {
    let src = "\
fn f(a: f64, b: f64, c: bool) -> f64 {
    let x = a; // after
    let y = a /* mid */ + b;
    if c {
        let z = a /* in */ + 1.0;
    }
    let w = { // after an open brace
        a // after a node inside a verbatim statement
    };
    y
}
";
    let let_y = between(src, "let y", ";");
    let let_z = between(src, "let z", ";");
    let let_w = between(src, "let w", "};");
    let (got, verbatim) = anchors(src);
    assert_eq!(
        got,
        vec![
            Anchor::After(between(src, "let x", ";")),
            Anchor::Verbatim(let_y),
            Anchor::Verbatim(let_z),
            Anchor::Verbatim(let_w),
            Anchor::Verbatim(let_w),
        ]
    );
    assert_eq!(verbatim, vec![let_y, let_z, let_w]);

    // The count: one anchor per comment; a verbatim comment lies in exactly one verbatim node.
    let comments = Comments::of(src);
    assert_eq!(got.len(), comments.list().len());
    for (c, anchor) in comments.list().iter().zip(&got) {
        let holders: Vec<&Span> = verbatim.iter().filter(|v| v.start <= c.span.start && c.span.end <= v.end).collect();
        match anchor {
            Anchor::Verbatim(v) => assert_eq!(holders, vec![v], "{}", c.text),
            _ => assert!(holders.is_empty(), "{} is anchored and in a verbatim node", c.text),
        }
    }
}
