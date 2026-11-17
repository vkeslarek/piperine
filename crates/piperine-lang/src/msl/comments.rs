//! Plain comments beside the tokens.
//!
//! The lexer drops a plain comment (a `//` or `/* */` that is no doc comment):
//! the tree has no place for one. [`Comments::of`] lists them beside the
//! tokens, with their spans, so a formatter can put each back where the
//! source had it. Doc comments stay tokens and belong to the tree.

use super::lexer::lex_with_comments;
use super::span::Span;

/// One plain comment of a source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The comment's bytes in the source, markers included; a line comment
    /// ends before its line break (and before a `\r` closing the line).
    pub span: Span,
    /// The comment as written: `src[span]`.
    pub text: String,
    /// Only whitespace stands between the start of its line and the comment.
    pub starts_line: bool,
}

/// Every plain comment of a source text, in source order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comments {
    list: Vec<Comment>,
}

impl Comments {
    /// The plain comments of `src`. The tokens, the doc comments and the
    /// diagnostics of the lexer are not changed by this list.
    pub fn of(src: &str) -> Comments {
        let (_, _, spans) = lex_with_comments(src);
        let list = spans
            .into_iter()
            .map(|span| {
                let end = if src[span.range()].starts_with("//") && src[span.range()].ends_with('\r') {
                    span.end - 1
                } else {
                    span.end
                };
                let span = Span::new(span.start, end);
                let line_start = src[..span.start].rfind('\n').map_or(0, |i| i + 1);
                let starts_line = src[line_start..span.start].chars().all(|c| c == ' ' || c == '\t' || c == '\u{FEFF}');
                Comment { span, text: src[span.range()].to_string(), starts_line }
            })
            .collect();
        Comments { list }
    }

    /// The comments, in source order.
    pub fn list(&self) -> &[Comment] {
        &self.list
    }

    /// The comment texts, in source order.
    pub fn texts(&self) -> Vec<&str> {
        self.list.iter().map(|c| c.text.as_str()).collect()
    }
}
