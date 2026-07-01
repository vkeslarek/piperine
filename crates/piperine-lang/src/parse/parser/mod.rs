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

pub struct Parser<'a> {
    toks: &'a [Lexed],
    pos: usize,
}

impl<'a> Parser<'a> {
    /// Creates a new parser over the given token slice.
    pub fn new(toks: &'a [Lexed]) -> Self {
        Self { toks, pos: 0 }
    }

    /// Returns the next token without consuming it, or `None` at end of input.
    pub(crate) fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|l| &l.tok)
    }

    /// Returns the token at `offset` positions ahead without consuming it.
    pub(crate) fn peek_at(&self, offset: usize) -> Option<&Tok> {
        self.toks.get(self.pos + offset).map(|l| &l.tok)
    }

    /// Consumes and returns `true` if the next token equals `tok`, else `false`.
    pub(crate) fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Consumes and returns `true` if the next token is an `Ident` with the given spelling.
    pub(crate) fn eat_ident(&mut self, expected: &str) -> bool {
        match self.peek() {
            Some(Tok::Ident(s)) if s == expected => {
                self.pos += 1;
                true
            }
            _ => false,
        }
    }

    /// Consumes the next token if it matches `tok`, or returns an error describing the mismatch.
    pub(crate) fn expect(&mut self, tok: &Tok) -> Result<(), String> {
        if self.eat(tok) {
            Ok(())
        } else {
            Err(format!("Expected {:?}, found {:?}", tok, self.peek()))
        }
    }

    /// Consumes the next token if it is an `Ident` with the given spelling, or returns an error.
    pub(crate) fn expect_ident_str(&mut self, expected: &str) -> Result<(), String> {
        if self.eat_ident(expected) {
            Ok(())
        } else {
            Err(format!("Expected `{}`, found {:?}", expected, self.peek()))
        }
    }

    /// Parses and consumes a single `Ident` token, returning its string value.
    pub(crate) fn parse_ident(&mut self) -> Result<String, String> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let res = s.clone();
                self.pos += 1;
                Ok(res)
            }
            _ => Err(format!("Expected identifier, found {:?}", self.peek())),
        }
    }

    // ─────────────────────────── §2  Compilation unit ────────────────────────

    /// Parses the entire token stream into a `SourceFile` AST.
    /// Dispatches to sub-parsers based on leading keywords (`mod`, `fn`, `use`, etc.).
    pub fn parse_file(&mut self) -> Result<SourceFile, String> {
        let mut items = Vec::new();
        while self.pos < self.toks.len() {
            if self.eat_ident("use") {
                let path = self.parse_path()?;
                self.expect(&Tok::Semi)?;
                items.push(Item::UseDecl(path));
            } else {
                let is_pub = self.eat_ident("pub");
                if self.eat_ident("mod") {
                    items.push(Item::ModDecl(self.parse_mod_decl(is_pub)?));
                } else if self.eat_ident("analog") {
                    items.push(Item::BehaviorDecl(
                        self.parse_behavior(is_pub, BehaviorKind::Analog)?,
                    ));
                } else if self.eat_ident("digital") {
                    items.push(Item::BehaviorDecl(
                        self.parse_behavior(is_pub, BehaviorKind::Digital)?,
                    ));
                } else if self.eat_ident("discipline") {
                    items.push(Item::DisciplineDecl(self.parse_discipline(is_pub)?));
                } else if self.eat_ident("bundle") {
                    items.push(Item::BundleDecl(self.parse_bundle(is_pub)?));
                } else if self.eat_ident("enum") {
                    items.push(Item::EnumDecl(self.parse_enum(is_pub)?));
                } else if self.eat_ident("capability") {
                    items.push(Item::CapabilityDecl(self.parse_capability(is_pub)?));
                } else if self.eat_ident("impl") {
                    items.push(Item::ImplDecl(self.parse_impl(is_pub)?));
                } else if self.eat_ident("fn") {
                    items.push(Item::FnDecl(self.parse_fn_decl(is_pub)?));
                } else {
                    return Err(format!("Unknown top-level item at {:?}", self.peek()));
                }
            }
        }
        Ok(SourceFile { items })
    }

    /// Parses a `::`-separated path of identifiers, e.g. `std::foo::Bar`.
    pub(crate) fn parse_path(&mut self) -> Result<Path, String> {
        let mut segments = vec![self.parse_ident()?];
        while self.eat(&Tok::DoubleColon) {
            segments.push(self.parse_ident()?);
        }
        Ok(Path { segments })
    }

}

mod expr;
mod items;
mod stmt;
