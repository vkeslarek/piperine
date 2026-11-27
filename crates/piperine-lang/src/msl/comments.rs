//! Plain comments beside the tokens.
//!
//! The lexer drops a plain comment (a `//` or `/* */` that is no doc comment):
//! the tree has no place for one. [`Comments::of`] lists them beside the
//! tokens, with their spans, so a formatter can put each back where the
//! source had it. Doc comments stay tokens and belong to the tree.
//!
//! [`Comments::attach`] gives each comment its [`Anchor`]: the node it
//! stands before, the node it follows on that node's last line, the
//! container it ends, or, when none of these reaches it, the member or
//! statement that holds it, printed as the source writes it.

use std::cell::RefCell;

use super::ast::File;
use super::lexer::{Lexer, Punct, Token};
use super::print::{Close, Hooks, Printer, Shape};
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
        let (_, _, spans) = Lexer::lex_with_comments(src);
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

/// Where a comment is printed when its file is formatted. A node is named by
/// its extent: its span widened to its doc comments and attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// On its own line, before the node: an item, a member, a statement, a
    /// match arm, an element of a list, a field, a variant.
    Before(Span),
    /// After the node, on the node's last line.
    After(Span),
    /// Before the closing delimiter at this span (the end of the file is the
    /// empty span at its length): the comment ends that container.
    Inside(Span),
    /// Inside this member or statement, which is printed as the source
    /// writes it, re-indented.
    Verbatim(Span),
    /// In no member or statement and at no anchor: the file cannot be formatted.
    Unplaced,
}

/// The anchors of a file's comments, one per comment of [`Comments::list`].
#[derive(Debug, Clone)]
pub struct Attached {
    anchors: Vec<Anchor>,
    verbatim: Vec<Span>,
    tokens: Vec<(Token, Span)>,
    len: usize,
}

impl Attached {
    /// The anchor of each comment, in the order of [`Comments::list`].
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// The members and statements printed as the source writes them, in
    /// source order; no two overlap.
    pub fn verbatim(&self) -> &[Span] {
        &self.verbatim
    }

    /// The span of the closing delimiter a container of the printer names:
    /// the `)`, `]` or `}` there, the end of the file, or `None`.
    pub(crate) fn close(&self, close: Close) -> Option<Span> {
        let delimiter = |t: &Token| matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace));
        let token = match close {
            Close::End => return Some(Span::new(self.len, self.len)),
            Close::Unknown => return None,
            Close::At(at) => self.tokens.iter().find(|(_, s)| s.start == at),
            Close::After(at) => self
                .tokens
                .iter()
                .find(|(t, s)| s.start >= at && !matches!(t, Token::Punct(Punct::Comma | Punct::Semi))),
        };
        token.filter(|(t, _)| delimiter(t)).map(|(_, s)| *s)
    }
}

/// The printer's walk of a tree, written down: every child of every
/// container, the members and statements among them, and every container
/// with its children.
#[derive(Default)]
struct Layout {
    kids: RefCell<Vec<Span>>,
    members: RefCell<Vec<Span>>,
    containers: RefCell<Vec<(Close, Vec<Span>)>>,
}

impl Hooks for Layout {
    fn verbatim(&self, extent: Span) -> Option<String> {
        self.members.borrow_mut().push(extent);
        None
    }

    fn around(&self, extent: Span, text: String, _: Option<Span>, _: Shape) -> String {
        self.kids.borrow_mut().push(extent);
        text
    }

    fn inside(&self, close: Close, kids: &[Span], _: Shape) -> Vec<String> {
        self.containers.borrow_mut().push((close, kids.to_vec()));
        Vec::new()
    }
}

impl Comments {
    /// Anchor every comment of `src` to a node of `file`, the tree `src`
    /// parses to: `After` the node whose last token (or the `,` or `;` right
    /// after it) the comment follows on the same line; else `Before` the node
    /// whose first token comes next; else `Inside` the container whose
    /// closing delimiter comes next, after all its children. A comment no
    /// anchor reaches makes the innermost member or statement holding it
    /// `Verbatim`, and every comment inside a verbatim node is that node's.
    pub fn attach(&self, file: &File, src: &str) -> Attached {
        let layout = Layout::default();
        Printer::new(&layout).file(file);
        let (tokens, _, _) = Lexer::lex_with_comments(src);
        let mut attached = Attached { anchors: Vec::new(), verbatim: Vec::new(), tokens, len: src.len() };
        let kids = layout.kids.into_inner();
        let members = layout.members.into_inner();
        let containers: Vec<(Option<Span>, Vec<Span>)> =
            layout.containers.into_inner().into_iter().map(|(c, k)| (attached.close(c), k)).collect();

        let mut marked = Vec::new();
        for c in &self.list {
            let anchor = attached.anchor(c, src, &kids, &containers).unwrap_or_else(|| {
                let holder = members
                    .iter()
                    .filter(|m| m.start <= c.span.start && c.span.end <= m.end)
                    .min_by_key(|m| m.len())
                    .copied();
                holder.map_or(Anchor::Unplaced, |m| {
                    marked.push(m);
                    Anchor::Verbatim(m)
                })
            });
            attached.anchors.push(anchor);
        }
        let contains = |outer: &Span, inner: &Span| outer.start <= inner.start && inner.end <= outer.end;
        let mut verbatim: Vec<Span> =
            marked.iter().filter(|m| !marked.iter().any(|o| o != *m && contains(o, m))).copied().collect();
        verbatim.sort_by_key(|s| s.start);
        verbatim.dedup();
        for (c, anchor) in self.list.iter().zip(attached.anchors.iter_mut()) {
            if let Some(v) = verbatim.iter().find(|v| contains(v, &c.span)) {
                *anchor = Anchor::Verbatim(*v);
            }
        }
        attached.verbatim = verbatim;
        attached
    }
}

impl Attached {
    /// The `After`, `Before` or `Inside` anchor of `c`, when one reaches it.
    fn anchor(&self, c: &Comment, src: &str, kids: &[Span], containers: &[(Option<Span>, Vec<Span>)]) -> Option<Anchor> {
        let n = self.tokens.partition_point(|(_, s)| s.start < c.span.end);
        let prev = n.checked_sub(1).map(|i| &self.tokens[i]);
        let next = self.tokens.get(n).map(|(_, s)| *s);
        let same_line = prev.is_some_and(|(_, p)| !src[p.end..c.span.start].contains('\n'));
        let outermost = |found: Vec<&Span>| found.into_iter().max_by_key(|k| k.len()).copied();

        if let (true, Some((token, p))) = (same_line, prev) {
            // The node ends at the previous token, or right before it when that is its `,` or `;`.
            let before_separator = matches!(token, Token::Punct(Punct::Comma | Punct::Semi))
                .then(|| n.checked_sub(2).map(|i| self.tokens[i].1.end))
                .flatten();
            let ends = kids.iter().filter(|k| k.end == p.end || Some(k.end) == before_separator).collect();
            if let Some(k) = outermost(ends) {
                return Some(Anchor::After(k));
            }
        }
        if let (false, Some(next)) = (same_line, next) {
            if let Some(k) = outermost(kids.iter().filter(|k| k.start == next.start).collect()) {
                return Some(Anchor::Before(k));
            }
        }
        let closes = next.unwrap_or(Span::new(self.len, self.len));
        containers
            .iter()
            .find(|(close, k)| *close == Some(closes) && k.iter().all(|k| k.end <= c.span.start))
            .map(|_| Anchor::Inside(closes))
    }
}
