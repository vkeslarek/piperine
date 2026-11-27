//! The formatter: a source text in the printer's layout, every plain comment kept.
//!
//! [`Formatter::format`] parses the text, anchors its comments
//! ([`Comments::attach`]) and runs the printer's layout node by node with
//! hooks that put each comment back: the `Before` comments on their own
//! lines before their node, the `After` comments after it on its last line,
//! the `Inside` comments before the closing delimiter, and the source text of
//! a `Verbatim` node re-indented. One blank line stands where the source had
//! one or more between two members, statements or items' comments; none
//! stands where it had none. `print` stays the canonical text of a tree;
//! `format` is what a person's file becomes.
//!
//! The guard: the formatter lexes its own output and refuses it (an
//! internal-error diagnostic, `P0004`, and no text) unless it holds the
//! source's comments, each once, in order, unchanged, and parses to the
//! source's tree. A formatter bug fails loud; it never deletes a comment.

use std::cell::RefCell;
use std::collections::HashMap;

use super::comments::{Anchor, Attached, Comment, Comments};
use super::diag::Diagnostic;
use super::lexer::Lexer;
use super::parser::SourceParser;
use super::print::{Close, Hooks, Printer, Shape};
use super::span::Span;

/// Formats MSL source text.
pub struct Formatter;

impl Formatter {
    /// The formatted text of `src`, or the parser's diagnostics when `src`
    /// does not parse, or one internal-error diagnostic when the formatter
    /// cannot keep every comment and the tree.
    pub fn format(src: &str) -> Result<String, Vec<Diagnostic>> {
        let parsed = SourceParser::parse(src);
        if !parsed.diagnostics.is_empty() {
            return Err(parsed.diagnostics);
        }
        let comments = Comments::of(src);
        let attached = comments.attach(&parsed.file, src);
        if let Some(c) = comments.list().iter().zip(attached.anchors()).find(|(_, a)| **a == Anchor::Unplaced).map(|(c, _)| c) {
            return Err(vec![Formatter::internal(c.span, "the formatter found no place for this comment")]);
        }
        let placement = Placement::new(src, comments.list(), &attached);
        let text = Printer::new(&placement).file(&parsed.file);
        let printed = placement.printed.into_inner();
        if let Some((c, n)) = comments.list().iter().zip(printed).find(|(_, n)| *n != 1) {
            let what = if n == 0 { "lost" } else { "repeated" };
            return Err(vec![Formatter::internal(c.span, &format!("the formatter {what} this comment"))]);
        }
        Formatter::guard(src, text)
    }

    /// Whether `src` is already formatted; the diagnostics of [`format`](Self::format) otherwise.
    pub fn check(src: &str) -> Result<bool, Vec<Diagnostic>> {
        Formatter::format(src).map(|text| text == src)
    }

    /// `formatted` when it holds the plain comments of `src`, each once, in
    /// order, unchanged, and parses to the tree of `src`; else one
    /// internal-error diagnostic and no text.
    pub fn guard(src: &str, formatted: String) -> Result<String, Vec<Diagnostic>> {
        let before = Comments::of(src);
        let after = Comments::of(&formatted);
        if before.texts() != after.texts() {
            let at = before
                .list()
                .iter()
                .zip(after.list().iter().map(Some).chain(std::iter::repeat(None)))
                .find(|(b, a)| a.is_none_or(|a| a.text != b.text))
                .map_or(Span::new(0, 0), |(b, _)| b.span);
            return Err(vec![Formatter::internal(at, "the formatted text would lose, add or reorder a comment")]);
        }
        let source = SourceParser::parse(src);
        let result = SourceParser::parse(&formatted);
        if !result.diagnostics.is_empty() || result.file != source.file {
            return Err(vec![Formatter::internal(Span::new(0, 0), "the formatted text would parse to another tree")]);
        }
        Ok(formatted)
    }

    fn internal(at: Span, what: &str) -> Diagnostic {
        Diagnostic::new("P0004", format!("internal error: {what}"), at, "this is a bug in the formatter, not in the source")
            .with_help("the file is left as it is; report the source that gives this error")
    }
}

/// The hooks that put comments back where [`Attached`] anchors them, and
/// count how often each comment is printed.
struct Placement<'a> {
    src: &'a str,
    comments: &'a [Comment],
    attached: &'a Attached,
    /// The source with every comment byte replaced by `c`: whitespace outside comments.
    mask: Vec<u8>,
    /// Spans no layout may touch: every token and comment holding a line break.
    protected: Vec<Span>,
    before: HashMap<Span, Vec<usize>>,
    after: HashMap<Span, Vec<usize>>,
    inside: HashMap<Span, Vec<usize>>,
    printed: RefCell<Vec<u32>>,
}

impl<'a> Placement<'a> {
    fn new(src: &'a str, comments: &'a [Comment], attached: &'a Attached) -> Placement<'a> {
        let mut mask = src.as_bytes().to_vec();
        for c in comments {
            mask[c.span.range()].fill(b'c');
        }
        let (tokens, _) = Lexer::lex(src);
        let protected = tokens
            .iter()
            .map(|(_, s)| *s)
            .chain(comments.iter().map(|c| c.span))
            .filter(|s| src[s.range()].contains('\n'))
            .collect();
        let mut placement = Placement {
            src,
            comments,
            attached,
            mask,
            protected,
            before: HashMap::new(),
            after: HashMap::new(),
            inside: HashMap::new(),
            printed: RefCell::new(vec![0; comments.len()]),
        };
        for (i, anchor) in attached.anchors().iter().enumerate() {
            let (map, at) = match anchor {
                Anchor::Before(at) => (&mut placement.before, at),
                Anchor::After(at) => (&mut placement.after, at),
                Anchor::Inside(at) => (&mut placement.inside, at),
                Anchor::Verbatim(_) | Anchor::Unplaced => continue,
            };
            map.entry(*at).or_default().push(i);
        }
        placement
    }

    /// Whether a whitespace-only line stands between `from` and `to` in the source.
    fn blank(&self, from: usize, to: usize) -> bool {
        let gap = &self.mask[from.min(to)..to];
        let lines: Vec<&[u8]> = gap.split(|b| *b == b'\n').collect();
        lines.len() > 2 && lines[1..lines.len() - 1].iter().any(|l| l.iter().all(|b| matches!(b, b' ' | b'\t' | b'\r')))
    }

    /// The text of comment `i`, counted as printed.
    fn comment(&self, i: usize) -> String {
        self.printed.borrow_mut()[i] += 1;
        Printer::escape(&self.comments[i].text)
    }

    /// The source text of the node at `extent`, re-indented: every line after
    /// the first loses up to the indentation of the node's first line, and
    /// trailing whitespace outside comments; line breaks inside a string or a
    /// comment are kept as written.
    fn reindented(&self, extent: Span) -> String {
        let src = self.src;
        let line_start = src[..extent.start].rfind('\n').map_or(0, |i| i + 1);
        let base = src[line_start..].chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let inner = |at: usize| self.protected.iter().any(|s| s.start < at && at < s.end);
        let mut lines = Vec::new();
        let mut start = extent.start;
        for (i, _) in src[extent.range()].match_indices('\n') {
            let at = extent.start + i;
            if !inner(at) {
                lines.push(Span::new(start, at));
                start = at + 1;
            }
        }
        lines.push(Span::new(start, extent.end));
        let mut out = Vec::with_capacity(lines.len());
        for (n, line) in lines.iter().enumerate() {
            let mut from = line.start;
            if n > 0 {
                from += src[line.range()].chars().take(base).take_while(|c| *c == ' ' || *c == '\t').count();
            }
            let trimmed = src[from..line.end].trim_end_matches([' ', '\t', '\r']);
            let mut to = from + trimmed.len();
            if self.protected.iter().chain(self.comments.iter().map(|c| &c.span)).any(|s| s.start < line.end && to < s.end) {
                to = line.end;
            }
            out.push(Printer::escape(&src[from..to]));
        }
        out.join("\n")
    }
}

impl Hooks for Placement<'_> {
    fn verbatim(&self, extent: Span) -> Option<String> {
        if !self.attached.verbatim().contains(&extent) {
            return None;
        }
        for (i, anchor) in self.attached.anchors().iter().enumerate() {
            if *anchor == Anchor::Verbatim(extent) {
                self.printed.borrow_mut()[i] += 1;
            }
        }
        Some(self.reindented(extent))
    }

    fn around(&self, extent: Span, text: String, prev: Option<Span>, shape: Shape) -> String {
        let blanks = shape != Shape::List;
        let mut out = String::new();
        let mut cursor = if shape == Shape::Lines { prev.map(|p| p.end) } else { None };
        for &i in self.before.get(&extent).into_iter().flatten() {
            let c = &self.comments[i];
            if blanks && cursor.is_some_and(|at| self.blank(at, c.span.start)) {
                out.push('\n');
            }
            out.push_str(&self.comment(i));
            out.push('\n');
            cursor = Some(c.span.end);
        }
        if blanks && cursor.is_some_and(|at| self.blank(at, extent.start)) {
            out.push('\n');
        }
        out.push_str(&text);
        for &i in self.after.get(&extent).into_iter().flatten() {
            out.push(' ');
            out.push_str(&self.comment(i));
        }
        out
    }

    fn inside(&self, close: Close, kids: &[Span], shape: Shape) -> Vec<String> {
        let Some(at) = self.attached.close(close) else { return Vec::new() };
        let mut cursor = kids.last().map(|k| k.end);
        let mut lines = Vec::new();
        for &i in self.inside.get(&at).into_iter().flatten() {
            let c = &self.comments[i];
            let gap = shape != Shape::List && cursor.is_some_and(|from| self.blank(from, c.span.start));
            lines.push(format!("{}{}", if gap { "\n" } else { "" }, self.comment(i)));
            cursor = Some(c.span.end);
        }
        lines
    }
}
