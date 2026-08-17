//! The shared token cursor every `grammar/{expr,stmt,item}.rs` production
//! method is written against. One `Grammar` per parse; `expr.rs`/`stmt.rs`/
//! `item.rs` each contribute `impl Grammar` blocks (MD-13 rule 4: files
//! named by what they parse, not by "helpers").

use crate::ast::Attr;
use crate::error::{ParseError, ParseErrorKind, PreprocessedSpan};
use crate::lexer::{Lexed, Tok};

/// Wraps a token stream with whitespace/comment tokens already filtered
/// out (the preprocessor/lexer keep newlines and comments for their own
/// needs; the grammar layer works over meaningful tokens only) and drives
/// every `parse_*` production method in `expr.rs`/`stmt.rs`/`item.rs`.
pub struct Grammar {
    toks: Vec<Lexed>,
    pos: usize,
    /// `///` doc-comment runs, keyed by the buffer offset of the first
    /// meaningful token that follows the run — see [`Self::doc_at`].
    docs: std::collections::HashMap<usize, String>,
}

impl Grammar {
    /// Each production method's doc comment names the Annex A production
    /// it implements (VAMS-23's conformance gate indexes these names).
    pub fn new(toks: Vec<Lexed>) -> Self {
        let docs = collect_doc_runs(&toks);
        let toks: Vec<Lexed> = toks
            .into_iter()
            .filter(|l| !matches!(l.tok, Tok::Newline | Tok::LineComment(_) | Tok::BlockComment(_)))
            .collect();
        Self { toks, pos: 0, docs }
    }

    /// The `///` doc-comment run immediately preceding the token at
    /// `span`, if any — the text with each line's leading `///` and
    /// surrounding whitespace stripped, joined by newlines.
    ///
    /// **Why a side table instead of keeping comments in the stream:**
    /// every production method in `expr.rs`/`stmt.rs`/`item.rs` is written
    /// against a comment-free token stream (positional `peek`/`bump`);
    /// leaving `LineComment`s in would force each one to skip them by
    /// hand — a wide, error-prone change to a hand-written parser
    /// `CLAUDE.md` explicitly flags as ripple-prone. Attaching them out of
    /// band keeps token indexing byte-identical and makes the lookup a
    /// single map hit at the one place each declaration is constructed.
    /// (Same "derived side artifact" shape `Design::flat_modules` and
    /// `SourceProvenance` already use.)
    ///
    /// Callers pass the span of the declaration's **first** token, taken
    /// with `peek_span()` before consuming anything — attribute instances
    /// (`(* ... *)`) included, so a doc comment above an attributed
    /// declaration still lands on it.
    pub(crate) fn doc_at(&self, span: PreprocessedSpan) -> Option<String> {
        self.docs.get(&span.start).cloned()
    }

    pub(crate) fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }

    pub(crate) fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    pub(crate) fn peek_span(&self) -> PreprocessedSpan {
        self.toks[self.pos.min(self.toks.len() - 1)].span
    }

    pub(crate) fn bump(&mut self) -> Lexed {
        let l = self.toks[self.pos.min(self.toks.len() - 1)].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        l
    }

    pub(crate) fn at_punct(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::Punct(x) if *x == p)
    }

    pub(crate) fn eat_punct(&mut self, p: &str) -> bool {
        if self.at_punct(p) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_punct(&mut self, p: &str) -> Result<PreprocessedSpan, ParseError> {
        if self.at_punct(p) {
            Ok(self.bump().span)
        } else {
            Err(self.unexpected(format!("'{p}'")))
        }
    }

    /// `word` matches a `Tok::Ident` whose text equals `word` (keywords are
    /// lexically identifiers, LRM §2.8.2 — see `keywords.rs`).
    pub(crate) fn at_kw(&self, word: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == word)
    }

    pub(crate) fn eat_kw(&mut self, word: &str) -> bool {
        if self.at_kw(word) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_kw(&mut self, word: &str) -> Result<PreprocessedSpan, ParseError> {
        if self.at_kw(word) {
            Ok(self.bump().span)
        } else {
            Err(self.unexpected(format!("'{word}'")))
        }
    }

    pub(crate) fn expect_ident(&mut self) -> Result<(String, PreprocessedSpan), ParseError> {
        match self.peek().clone() {
            Tok::Ident(s) | Tok::EscapedIdent(s) => {
                let span = self.bump().span;
                Ok((s, span))
            }
            _ => Err(self.unexpected("an identifier".to_string())),
        }
    }

    /// `{ attribute_instance }` — zero or more `(* attr_spec {,
    /// attr_spec} *)` groups, Annex A.9.1, §2.9.1 Syntax 2-11/2-12. Shared
    /// by every grammar file whose productions are attribute-bearing sites
    /// (the `ast::Attr`-carrying node kinds — see `ast/item.rs`'s per-site
    /// rationale).
    pub(crate) fn parse_attr_instances(&mut self) -> Result<Vec<Attr>, ParseError> {
        let mut attrs = vec![];
        while self.eat_punct("(*") {
            loop {
                let (name, span) = self.expect_ident()?;
                let value = if self.eat_punct("=") { Some(self.parse_expr()?) } else { None };
                attrs.push(Attr { span, name, value });
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct("*)")?;
        }
        Ok(attrs)
    }

    pub(crate) fn peek_at_next(&self) -> &Tok {
        self.peek_at(1)
    }

    /// Scans past zero or more `(* ... *)` attribute-instance groups
    /// (without consuming them) and returns the identifier text
    /// immediately following, if any. Lets a dispatcher recognize
    /// `(* attr *) branch ...` the same way it recognizes `branch ...`,
    /// while leaving the attributes themselves for the chosen production's
    /// own `parse_attr_instances()` call to consume.
    pub(crate) fn kw_after_attrs(&self) -> Option<&str> {
        let mut idx = self.pos;
        while matches!(self.toks.get(idx).map(|l| &l.tok), Some(Tok::Punct("(*"))) {
            idx += 1;
            while !matches!(self.toks.get(idx).map(|l| &l.tok), Some(Tok::Punct("*)")) | None) {
                idx += 1;
            }
            idx += 1;
        }
        match self.toks.get(idx).map(|l| &l.tok) {
            Some(Tok::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Lookahead `offset` tokens past the current position — needed to
    /// dispatch module items that share a leading identifier (net
    /// declaration vs. module instantiation, §Annex A.1.4) on more than
    /// one token of context.
    pub(crate) fn peek_at(&self, offset: usize) -> &Tok {
        let idx = (self.pos + offset).min(self.toks.len() - 1);
        &self.toks[idx].tok
    }

    pub(crate) fn unexpected(&self, expected: String) -> ParseError {
        let found = describe(self.peek());
        ParseError::new(ParseErrorKind::UnexpectedToken { expected, found }, self.peek_span())
    }

    pub(crate) fn eof_error(&self, expected: String) -> ParseError {
        ParseError::new(ParseErrorKind::UnexpectedEof { expected }, self.peek_span())
    }
}

/// Builds [`Grammar::doc_at`]'s lookup table from the raw (unfiltered)
/// token stream: every run of consecutive `///` line comments is attached
/// to the first meaningful token after it.
///
/// The lexer stores a line comment's text with the leading `//` already
/// consumed, so a `///` line arrives here as a `LineComment` whose text
/// starts with `/` — that leading slash is what distinguishes a doc
/// comment from an ordinary `//` one, and it is stripped along with
/// surrounding whitespace.
///
/// A run is broken by a **blank line**, a plain `//`/`/* */` comment, or
/// any meaningful token. A `///` with no text after it is *not* a blank
/// line — it is a paragraph break inside the run, and continues it (same
/// rule rustdoc uses). This distinction is what keeps a file header from
/// silently becoming the first declaration's documentation:
///
/// ```text
/// /// This file implements a resistor library.   <- file header
///                                                <- blank line ends the run
/// module Resistor(p, n);                         <- gets no doc, correctly
/// ```
///
/// A run with no following declaration (a trailing doc at end of file) is
/// simply dropped.
fn collect_doc_runs(toks: &[Lexed]) -> std::collections::HashMap<usize, String> {
    let mut docs = std::collections::HashMap::new();
    let mut pending: Vec<String> = Vec::new();
    // A doc line's own terminating newline is expected; a *second*
    // consecutive one is a blank line, which ends the run.
    let mut newlines_since_doc_line = 0usize;
    for l in toks {
        match &l.tok {
            Tok::LineComment(text) if text.starts_with('/') => {
                pending.push(text[1..].trim().to_string());
                newlines_since_doc_line = 0;
            }
            Tok::Newline => {
                newlines_since_doc_line += 1;
                if newlines_since_doc_line > 1 {
                    pending.clear();
                }
            }
            Tok::LineComment(_) | Tok::BlockComment(_) => pending.clear(),
            _ => {
                if !pending.is_empty() {
                    docs.insert(l.span.start, std::mem::take(&mut pending).join("\n"));
                }
                newlines_since_doc_line = 0;
            }
        }
    }
    docs
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) | Tok::EscapedIdent(s) => format!("'{s}'"),
        Tok::SystemIdent(s) => format!("'${s}'"),
        Tok::Directive(s) => format!("'`{s}'"),
        Tok::Number(_) => "a number literal".to_string(),
        Tok::Str(_) => "a string literal".to_string(),
        Tok::Punct(p) => format!("'{p}'"),
        Tok::Newline => "a newline".to_string(),
        Tok::LineComment(_) | Tok::BlockComment(_) => "a comment".to_string(),
        Tok::Eof => "end of file".to_string(),
    }
}
