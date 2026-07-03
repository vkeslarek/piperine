//! # Parser
//!
//! A hand-written recursive-descent LL(1) parser that converts a token
//! sequence into a [`SourceFile`] AST.
//!
//! ## Phase contract
//!
//! **Input**: `&[Lexed]` — the output of [`Lexer::tokenize`].
//! **Output**: [`SourceFile`] — the root of the parse AST.
//!
//! ## What the parser does NOT check
//!
//! - **Name resolution**: type names, module names, capability names are
//!   stored as plain `String`s.
//! - **Semantic validity**: `<+` in a `mod` body, `cross` in a `digital`
//!   block, unmatched port counts — all deferred to elaboration.
//! - **Const-evaluability**: array dimensions may be arbitrary expressions.
//!
//! ## Grammar coverage
//!
//! Mirrors the PHDL grammar specification (§2–§8). Left-factoring notes are
//! inline at each non-terminal. The grammar is LL(1) — every choice is
//! resolved by one token of lookahead.

use super::ast::*;
use super::lexer::{Lexed, Tok};
pub use attributes::ParseAttributesExt;

#[derive(Clone)]
pub struct Parser<'a> {
    toks: &'a [Lexed],
    pos: usize,
    pub cursor_offset: Option<usize>,
    pub expectations: Vec<crate::parse::predict::ExpectedSyntax>,
    pub completion_triggered: bool,
}

pub trait Parse: Sized {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError>;
}

impl<'a> Parser<'a> {
    /// Creates a new parser over the given token slice.
    pub fn new(toks: &'a [Lexed]) -> Self {
        Self { toks, pos: 0, cursor_offset: None, expectations: Vec::new(), completion_triggered: false }
    }

    pub fn with_cursor(toks: &'a [Lexed], cursor_offset: usize) -> Self {
        Self { toks, pos: 0, cursor_offset: Some(cursor_offset), expectations: Vec::new(), completion_triggered: false }
    }

    /// Registers that the parser expected a certain syntax at the current position.
    pub fn current_span_start(&self) -> usize {
        self.toks.get(self.pos).map(|l| l.start).unwrap_or(0)
    }

    pub fn previous_span_end(&self) -> usize {
        if self.pos > 0 {
            self.toks.get(self.pos - 1).map(|l| l.end).unwrap_or(0)
        } else {
            0
        }
    }

    pub fn expected(&mut self, syntax: crate::parse::predict::ExpectedSyntax) {
        if self.cursor_offset.is_some() {
            if !self.expectations.contains(&syntax) {
                self.expectations.push(syntax);
            }
        }
    }

    fn check_cursor(&mut self) -> bool {
        if let Some(cursor) = self.cursor_offset {
            if let Some(t) = self.toks.get(self.pos) {
                if t.start >= cursor || (t.start <= cursor && cursor <= t.end) {
                    self.completion_triggered = true;
                    return true;
                }
            } else {
                // EOF and cursor is active
                self.completion_triggered = true;
                return true;
            }
        }
        false
    }

    /// Returns the next token without consuming it, or `None` at end of input.
    pub(crate) fn peek(&mut self) -> Option<&Tok> {
        if self.check_cursor() { return None; }
        self.toks.get(self.pos).map(|l| &l.tok)
    }

    /// Returns the token at `offset` positions ahead without consuming it.
    pub(crate) fn peek_at(&mut self, offset: usize) -> Option<&Tok> {
        // Simple peekahead doesn't trigger cursor logic to avoid false positives
        self.toks.get(self.pos + offset).map(|l| &l.tok)
    }

    /// Consumes and returns `true` if the next token equals `tok`, else `false`.
    pub(crate) fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.expectations.clear(); // Progress made
            self.pos += 1;
            true
        } else {
            self.expected(crate::parse::predict::ExpectedSyntax::Punctuation(tok.clone()));
            false
        }
    }

    /// Consumes and returns `true` if the next token is an `Ident` with the given spelling.
    pub(crate) fn eat_ident(&mut self, expected: &str) -> bool {
        match self.peek() {
            Some(Tok::Ident(s)) if s == expected => {
                self.expectations.clear(); // Progress made
                self.pos += 1;
                true
            }
            _ => {
                self.expected(crate::parse::predict::ExpectedSyntax::Keyword(expected.to_string()));
                false
            }
        }
    }

    pub(crate) fn make_error(&self, msg: String) -> crate::parse::error::ParseError {
        crate::parse::error::ParseError::Generic {
            message: msg,
            span: miette::SourceSpan::new(self.current_span_start().into(), 1usize.into()),
        }
    }

    /// Consumes the next token if it matches `tok`, or returns an error describing the mismatch.
    pub(crate) fn expect(&mut self, tok: &Tok) -> Result<(), crate::parse::error::ParseError> {
        if self.eat(tok) {
            Ok(())
        } else {
            let peeked = self.peek().cloned();
            Err(self.make_error(format!("Expected {:?}, found {:?}", tok, peeked)))
        }
    }

    /// Consumes the next token if it is an `Ident` with the given spelling, or returns an error.
    pub(crate) fn expect_ident_str(&mut self, expected: &str) -> Result<(), crate::parse::error::ParseError> {
        if self.eat_ident(expected) {
            Ok(())
        } else {
            let peeked = self.peek().cloned();
            Err(self.make_error(format!("Expected `{}`, found {:?}", expected, peeked)))
        }
    }

    /// Parses and consumes a single `Ident` token, returning its string value.
    pub(crate) fn parse_ident(&mut self) -> Result<String, crate::parse::error::ParseError> {
        self.expected(crate::parse::predict::ExpectedSyntax::Ident(crate::parse::predict::IdentRole::VariableName)); // default role
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let res = s.clone();
                self.expectations.clear();
                self.pos += 1;
                Ok(res)
            }
            _ => Err(format!("Expected identifier, found {:?}", self.peek()).into()),
        }
    }

    pub(crate) fn parse_ident_as(&mut self, role: crate::parse::predict::IdentRole) -> Result<String, crate::parse::error::ParseError> {
        self.expected(crate::parse::predict::ExpectedSyntax::Ident(role));
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let res = s.clone();
                self.expectations.clear();
                self.pos += 1;
                Ok(res)
            }
            _ => Err(format!("Expected identifier, found {:?}", self.peek()).into()),
        }
    }

    // ─────────────────────────── §2  Compilation unit ────────────────────────

    /// Parses the entire token stream into a `SourceFile` AST.
    /// Dispatches to sub-parsers based on leading keywords (`mod`, `fn`, `use`, etc.).

    pub(crate) fn sync_until(&mut self, predicate: impl Fn(&Tok) -> bool) {
        while let Some(t) = self.peek() {
            if predicate(t) {
                break;
            }
            self.pos += 1;
        }
    }

    pub fn parse_file(&mut self) -> (SourceFile, Vec<crate::parse::error::ParseError>) {
        let mut items = Vec::new();
        let mut errors = Vec::new();
        while self.pos < self.toks.len() && !self.completion_triggered {
            match Item::parse(self) {
                Ok(item) => items.push(item),
                Err(e) => {
                    errors.push(e);
                    self.sync_until(|t| {
                        if let Tok::Ident(s) = t {
                            matches!(s.as_str(), "mod" | "fn" | "discipline" | "bundle" | "enum" | "capability" | "impl" | "use" | "pub")
                        } else {
                            false
                        }
                    });
                    if !self.completion_triggered && self.pos < self.toks.len() {
                        // Prevent infinite loops if we hit a sync token but parsing still fails immediately
                        if let Some(t) = self.peek() {
                            if !matches!(t, Tok::Ident(s) if matches!(s.as_str(), "mod" | "fn" | "discipline" | "bundle" | "enum" | "capability" | "impl" | "use" | "pub")) {
                                self.pos += 1;
                            }
                        }
                    }
                }
            }
        }
        (SourceFile { items }, errors)
    }


    /// Parses a `::`-separated path of identifiers, e.g. `std::foo::Bar`.
    pub(crate) fn parse_path(&mut self) -> Result<Path, crate::parse::error::ParseError> {
        let mut segments = vec![self.parse_ident()?];
        while self.eat(&Tok::DoubleColon) {
            segments.push(self.parse_ident()?);
        }
        Ok(Path { segments })
    }

}

mod expr;
mod stmt;
pub mod attributes;

mod mod_decl;
mod types;
mod discipline_decl;
mod bundle_decl;
mod enum_decl;
mod capability_decl;
mod impl_decl;
mod fn_decl;
mod bench_decl;
mod block;
mod const_decl;
mod item;
