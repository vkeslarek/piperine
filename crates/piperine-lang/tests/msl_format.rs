//! Scope: the comment-preserving formatter of MSL (msl-cli CLI-23):
//! `piperine_lang::msl::comments` (the plain comments beside the tokens and
//! their anchors) and `piperine_lang::msl::format` (a formatted text that
//! keeps every comment, the tree and its own fixed point). Expected texts are
//! written by hand from the acceptance criterion each test cites.

use piperine_lang::msl::Span;
use piperine_lang::msl::comments::{Comment, Comments};

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
