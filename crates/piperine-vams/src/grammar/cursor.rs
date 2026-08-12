//! The shared token cursor every `grammar/{expr,stmt,item}.rs` production
//! method is written against. One `Grammar` per parse; `expr.rs`/`stmt.rs`/
//! `item.rs` each contribute `impl Grammar` blocks (MD-13 rule 4: files
//! named by what they parse, not by "helpers").

use crate::ast::Attr;
use crate::error::{ParseError, ParseErrorKind, Span};
use crate::lexer::{Lexed, Tok};

/// Wraps a token stream with whitespace/comment tokens already filtered
/// out (the preprocessor/lexer keep newlines and comments for their own
/// needs; the grammar layer works over meaningful tokens only) and drives
/// every `parse_*` production method in `expr.rs`/`stmt.rs`/`item.rs`.
pub struct Grammar {
    toks: Vec<Lexed>,
    pos: usize,
}

impl Grammar {
    /// Each production method's doc comment names the Annex A production
    /// it implements (VAMS-23's conformance gate indexes these names).
    pub fn new(toks: Vec<Lexed>) -> Self {
        let toks: Vec<Lexed> = toks
            .into_iter()
            .filter(|l| !matches!(l.tok, Tok::Newline | Tok::LineComment(_) | Tok::BlockComment(_)))
            .collect();
        Self { toks, pos: 0 }
    }

    pub(crate) fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }

    pub(crate) fn peek_span(&self) -> Span {
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

    pub(crate) fn expect_punct(&mut self, p: &str) -> Result<Span, ParseError> {
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

    pub(crate) fn expect_kw(&mut self, word: &str) -> Result<Span, ParseError> {
        if self.at_kw(word) {
            Ok(self.bump().span)
        } else {
            Err(self.unexpected(format!("'{word}'")))
        }
    }

    pub(crate) fn expect_ident(&mut self) -> Result<(String, Span), ParseError> {
        match self.peek().clone() {
            Tok::Ident(s) | Tok::EscapedIdent(s) => {
                let span = self.bump().span;
                Ok((s, span))
            }
            _ => Err(self.unexpected("an identifier".to_string())),
        }
    }

    /// `{ attribute_instance }` — zero or more `(* attr_spec {,
    /// attr_spec} *)` groups, §2.9.1 Syntax 2-11/2-12. Shared by every
    /// grammar file whose productions are attribute-bearing sites (the
    /// `ast::Attr`-carrying node kinds — see `ast/item.rs`'s per-site
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
