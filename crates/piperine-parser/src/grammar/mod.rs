//! Recursive-descent + Pratt parser: preprocessed token stream → [`crate::ast`].
//!
//! Follows `veriloga.ungram` from OpenVAF-Reloaded as the grammar reference.
//! Context-sensitive disambiguation (type vs identifier, instance vs net decl)
//! is resolved with bounded lookahead — the reason the parser is hand-written.
//! Grammar is split across submodules: [`item`], [`stmt`], [`expr`].

mod expr;
mod item;
mod stmt;

use crate::ast::*;
use crate::lexer::{Lexed, Tok};

pub(crate) type PResult<T> = Result<T, String>;

pub fn parse_tokens(tokens: &[Lexed]) -> PResult<SourceFile> {
    let mut p = Parser { toks: tokens, pos: 0 };
    let mut items = Vec::new();
    while !p.at_end() {
        items.push(p.item()?);
    }
    Ok(SourceFile { items })
}

pub(crate) struct Parser<'a> {
    toks: &'a [Lexed],
    pos: usize,
}

/// Net-type keywords (Verilog-AMS LRM / OpenVAF token table).
const NET_TYPES: &[&str] =
    &["reg", "wreal", "wire", "uwire", "wand", "wor", "ground", "tri", "supply0", "supply1"];

/// Port-direction keywords.
const DIRECTIONS: &[&str] = &["input", "output", "inout", "terminal"];

impl<'a> Parser<'a> {
    // ── token cursor ────────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|l| &l.tok)
    }

    fn peek_at(&self, off: usize) -> Option<&Tok> {
        self.toks.get(self.pos + off).map(|l| &l.tok)
    }

    fn span_start(&self) -> usize {
        self.toks.get(self.pos).map_or(0, |l| l.start)
    }

    fn prev_end(&self) -> usize {
        self.pos.checked_sub(1).map_or(0, |i| self.toks[i].end)
    }

    fn bump(&mut self) -> &'a Lexed {
        let t = &self.toks[self.pos];
        self.pos += 1;
        t
    }

    fn at(&self, t: &Tok) -> bool {
        self.peek() == Some(t)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        let hit = self.at(t);
        if hit { self.pos += 1; }
        hit
    }

    fn expect(&mut self, t: &Tok) -> PResult<()> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(format!("expected {:?}, found {:?}", t, self.peek()))
        }
    }

    fn kw_at(&self, off: usize, kw: &str) -> bool {
        matches!(self.peek_at(off), Some(Tok::Ident(s)) if s == kw)
    }

    fn at_kw(&self, kw: &str) -> bool {
        self.kw_at(0, kw)
    }

    fn at_any_kw(&self, kws: &[&str]) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if kws.contains(&s.as_str()))
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        let hit = self.at_kw(kw);
        if hit { self.pos += 1; }
        hit
    }

    fn expect_kw(&mut self, kw: &str) -> PResult<()> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            Err(format!("expected keyword `{kw}`, found {:?}", self.peek()))
        }
    }

    fn ident(&mut self) -> PResult<String> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.pos += 1;
                Ok(s)
            }
            other => Err(format!("expected identifier, found {other:?}")),
        }
    }

    fn name(&mut self) -> PResult<Name> {
        Ok(Name(self.ident()?))
    }

    // ── attributes, paths ───────────────────────────────────────────────

    fn attrs(&mut self) -> PResult<Vec<Attr>> {
        let mut attrs = Vec::new();
        while self.eat(&Tok::AttrStart) {
            while !self.at(&Tok::AttrEnd) {
                let name = self.name()?;
                let val = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
                attrs.push(Attr { name, val });
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::AttrEnd)?;
        }
        Ok(attrs)
    }

    fn path(&mut self) -> PResult<Path> {
        let mut path = Path { qualifier: None, segment: self.path_segment()? };
        while self.at(&Tok::Dot) && matches!(self.peek_at(1), Some(Tok::Ident(_))) {
            self.bump();
            let segment = self.path_segment()?;
            path = Path { qualifier: Some(Box::new(path)), segment };
        }
        Ok(path)
    }

    fn path_segment(&mut self) -> PResult<PathSegment> {
        if self.eat_kw("root") {
            Ok(PathSegment::Root)
        } else {
            Ok(PathSegment::Ident(self.ident()?))
        }
    }

    // ── types, directions ───────────────────────────────────────────────

    fn at_dir(&self) -> bool {
        self.at_any_kw(DIRECTIONS)
    }

    fn dir_at(&self, off: usize) -> bool {
        matches!(self.peek_at(off), Some(Tok::Ident(s)) if DIRECTIONS.contains(&s.as_str()))
    }

    fn direction(&mut self) -> PResult<Direction> {
        let dir = match self.peek() {
            Some(Tok::Ident(s)) => match s.as_str() {
                "inout"    => Direction::Inout,
                "input"    => Direction::Input,
                "output"   => Direction::Output,
                other => return Err(format!("expected port direction, found {other:?}")),
            },
            other => return Err(format!("expected port direction, found {other:?}")),
        };
        self.pos += 1;
        Ok(dir)
    }

    /// A genuine primitive-type keyword (no `Ident Ident` heuristic). The type
    /// words that `type_()` resolves to integer/real/string.
    fn at_primitive_type_kw(&self) -> bool {
        self.at_any_kw(&[
            "integer", "int", "logic", "bit", "reg", "byte", "shortint", "longint",
            "real", "realtime", "time", "shortreal", "string",
        ])
    }

    /// Scan ahead from the cursor: is there an `=` before the next `;`?
    /// Distinguishes an initialized custom-typed var (`state_t s = X;`) from a
    /// discipline net decl (`electrical a, b;`) — both are `Ident Ident`.
    fn assign_before_semi(&self) -> bool {
        let mut p = self.pos;
        while let Some(t) = self.toks.get(p) {
            match t.tok {
                Tok::Semi => return false,
                Tok::Assign => return true,
                _ => p += 1,
            }
        }
        false
    }

    fn is_type_kw(&self) -> bool {
        if self.at_any_kw(&["integer", "real", "string"]) {
            true
        } else if let Some(Tok::Ident(_)) = self.peek() {
            matches!(self.peek_at(1), Some(Tok::Ident(_)))
        } else {
            false
        }
    }

    fn type_(&mut self) -> PResult<Type> {
        // Integer family (incl. SV/Verilog digital type words — aliased to integer,
        // not left as a silent `void` custom type).
        if self.at_any_kw(&["integer", "int", "logic", "bit", "reg",
                            "byte", "shortint", "longint"]) {
            self.pos += 1; Ok(Type::Integer)
        }
        // Real family (time/realtime are continuous reals in the analog world).
        else if self.at_any_kw(&["real", "realtime", "time", "shortreal"]) {
            self.pos += 1; Ok(Type::Real)
        }
        else if self.eat_kw("string") { Ok(Type::String) }
        else if let Some(Tok::Ident(_)) = self.peek() { Ok(Type::Custom(self.name()?)) }
        else { Err(format!("expected a type, found {:?}", self.peek())) }
    }

    fn skip_net_type(&mut self) {
        if self.at_any_kw(NET_TYPES) { self.pos += 1; }
    }

    // ── ranges, declarator lists ────────────────────────────────────────

    /// Return the index just past a balanced `[...]` starting at `p`, or `p` unchanged.
    fn idx_after_range(&self, mut p: usize) -> usize {
        if !matches!(self.toks.get(p).map(|t| &t.tok), Some(Tok::LBrack)) {
            return p;
        }
        let mut depth = 0;
        while let Some(t) = self.toks.get(p) {
            match t.tok {
                Tok::LBrack => depth += 1,
                Tok::RBrack => { depth -= 1; if depth == 0 { return p + 1; } }
                _ => {}
            }
            p += 1;
        }
        p
    }

    fn skip_range(&mut self) {
        if self.at(&Tok::LBrack) {
            self.pos = self.idx_after_range(self.pos);
        }
    }

    /// Parse optional `[msb:lsb]` or `[size]`.
    fn parse_range(&mut self) -> PResult<Option<BitRange>> {
        if !self.eat(&Tok::LBrack) { return Ok(None); }
        let msb = self.expr()?;
        let lsb = if self.eat(&Tok::Colon) { self.expr()? } else { msb.clone() };
        self.expect(&Tok::RBrack)?;
        Ok(Some(BitRange { msb, lsb }))
    }

    fn name_list(&mut self) -> PResult<Vec<Name>> {
        let mut names = vec![self.name()?];
        self.skip_range();
        while self.eat(&Tok::Comma) {
            names.push(self.name()?);
            self.skip_range();
        }
        Ok(names)
    }

    /// Parse a comma-separated list of declarators, stopping before a direction keyword.
    fn declarator_list(&mut self) -> PResult<Vec<Declarator>> {
        let mut decls = vec![self.declarator()?];
        while self.at(&Tok::Comma) && !self.dir_at(1) {
            self.bump();
            decls.push(self.declarator()?);
        }
        Ok(decls)
    }

    fn declarator(&mut self) -> PResult<Declarator> {
        let name = self.name()?;
        let range = self.parse_range()?;
        Ok(Declarator { name, range })
    }
}
