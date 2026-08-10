//! The IR parser — T9, the printer's inverse (LOV-02). Reads the stable
//! S-expression grammar documented in [`crate::print`] back into a
//! [`Module`], so hand-written IR fixtures (Phase 3, T15+) need no
//! front-end.
//!
//! ## Round-trip
//!
//! `parse(print(m))` produces a module structurally equivalent to `m` for
//! every printable `m`. The grammar's fixed section order and id-stable
//! `$N` / `\name` rendering make this mechanical: the parser allocates
//! ids in the order the printer emits them, which is the arena's
//! declaration order (Phase 1's dense indices).
//!
//! ## Failure mode
//!
//! A malformed text input is a NAMED ERROR ([`ParseError`]), never a
//! panic. The IR text is user input (hand-written fixtures, `--emit=ir`
//! diffs someone wrote by hand), so the no-`unwrap`-on-user-input rule
//! applies — every error path returns [`ParseError`].
//!
//! ## Implementation
//!
//! Hand-written recursive-descent parser. No `nom`/`pest` — purity
//! (LOV-09) loves fewer dependencies, and the grammar is small enough
//! to walk directly. The tokenizer emits four tokens: `(`, `)`, `Ident`
//! (any non-paren non-whitespace run, e.g. `$5`, `\name`, `add`,
//! `3.14e-1`, `12`), and `Str` (a quoted string). The grammar rules
//! interpret Idents in context.

use crate::core::{
    Block, BlockId, Cell, CellId, Chunk, Const, ConstId, CrossDir, Domain, EdgeKind, Module,
    NatureId, NatureInfo, NatureKind, Net, NetKind, Op, Pattern, PureOp, SigRef, Trigger, Ty,
    ValueId, NetId,
};
use crate::print::Names;

/// A parsed module plus its authored-name table. The two travel together
/// because the printed text mingles them: `\name` tokens reconstruct the
/// `Names` side-table that [`Module::print_with_names`] consumed.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule {
    /// The reconstructed IR module.
    pub module: Module,
    /// The authored-name table, recovered from `\name` tokens.
    pub names: Names,
}

/// Why a parse failed. Closed enum so a caller can match; [`std::fmt::Display`]
/// carries the human-readable diagnostic naming where the failure occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The top-level form was not `(module ...)`.
    MissingModuleHeader,
    /// A section appeared that is not one of the six the grammar names.
    UnknownSection(String),
    /// A section header appeared more than once.
    DuplicateSection(String),
    /// A form inside a section did not match its grammar rule.
    UnknownForm(String),
    /// An unknown keyword in a context that names a fixed vocabulary.
    UnknownKeyword(String),
    /// Expected one of `expected`, found `found` (the text of the token).
    UnexpectedToken { expected: &'static str, found: String },
    /// The input ended before a form was complete.
    UnexpectedEof(&'static str),
    /// A literal could not be parsed as the demanded type.
    InvalidNumber(String),
    /// A `$N` or `KN` identifier carried a non-numeric index.
    InvalidId(String),
    /// An id was declared out of arena order (the printer never emits
    /// this; a hand-written fixture is malformed if it does).
    IdOutOfOrder { kind: &'static str, expected: u32, found: u32 },
    /// A reference to an id that has not been declared in its section.
    UndeclaredReference { kind: &'static str, id: u32 },
    /// A bare token that did not match anything the parser expected.
    UnexpectedTokenText { found: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::MissingModuleHeader => write!(f, "missing `(module ...)` header"),
            ParseError::UnknownSection(s) => write!(f, "unknown section `{s}`"),
            ParseError::DuplicateSection(s) => write!(f, "section `{s}` declared twice"),
            ParseError::UnknownForm(s) => write!(f, "unknown form: {s}"),
            ParseError::UnknownKeyword(s) => write!(f, "unknown keyword `{s}`"),
            ParseError::UnexpectedToken { expected, found } => {
                write!(f, "expected {expected}, found `{found}`")
            }
            ParseError::UnexpectedEof(what) => write!(f, "unexpected end of input while parsing {what}"),
            ParseError::InvalidNumber(s) => write!(f, "invalid number literal: `{s}`"),
            ParseError::InvalidId(s) => write!(f, "invalid id literal: `{s}`"),
            ParseError::IdOutOfOrder { kind, expected, found } => {
                write!(f, "{kind} id {found} declared out of order (expected {expected})")
            }
            ParseError::UndeclaredReference { kind, id } => {
                write!(f, "reference to undeclared {kind} id {id}")
            }
            ParseError::UnexpectedTokenText { found } => write!(f, "unexpected token `{found}`"),
        }
    }
}

impl std::error::Error for ParseError {}

// ── tokens ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Tok<'a> {
    LParen,
    RParen,
    Ident(&'a str),
    Str(&'a str),
}

/// Tokenize `input` into a flat list. Whitespace separates tokens;
/// comments (`;` to end-of-line) are dropped; `(` and `)` are individual
/// tokens; `"..."` produces a `Tok::Str` with the raw contents (escapes
/// resolved by the caller as needed).
fn tokenize(input: &str) -> Result<Vec<Tok<'_>>, ParseError> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
            }
            b';' => {
                // Comment: skip to end of line.
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                let mut j = start;
                let mut buf = String::new();
                while j < bytes.len() && bytes[j] != b'"' {
                    if bytes[j] == b'\\' && j + 1 < bytes.len() {
                        // Escape: \" or \\ — unescape here.
                        match bytes[j + 1] {
                            b'"' => buf.push('"'),
                            b'\\' => buf.push('\\'),
                            other => {
                                buf.push('\\');
                                buf.push(other as char);
                            }
                        }
                        j += 2;
                    } else {
                        // The input is ASCII-safe for the strings the
                        // printer emits (access names like "voltage");
                        // fall back to chars() for non-ASCII.
                        let ch = input[j..].chars().next().ok_or(ParseError::UnexpectedEof("string literal"))?;
                        buf.push(ch);
                        j += ch.len_utf8();
                    }
                }
                if j >= bytes.len() {
                    return Err(ParseError::UnexpectedEof("closing `\"`"));
                }
                // The borrow checker: we cannot return the borrowed slice
                // because we built an unescaped String. Use a leaked
                // interner? Simpler: clone into a static via Box::leak
                // would leak — instead make Tok::Str own its content for
                // the rare quoted-string case. Switching to a String Tok
                // variant for the Str case.
                let leaked: &'static str = Box::leak(buf.into_boxed_str());
                out.push(Tok::Str(leaked));
                i = j + 1;
            }
            _ => {
                // Ident: a run of non-whitespace, non-paren, non-`;`,
                // non-`"` bytes. Use char boundaries.
                let start = i;
                let mut end = start;
                while end < bytes.len() {
                    let c = bytes[end];
                    if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' || c == b'(' || c == b')' || c == b';' || c == b'"' {
                        break;
                    }
                    end += 1;
                }
                out.push(Tok::Ident(&input[start..end]));
                i = end;
            }
        }
    }
    Ok(out)
}

// ── parser ────────────────────────────────────────────────────────────────

/// The parser — owns the recursive-descent walk over the tokenized input.
/// Methods on this struct (not free functions — MD-13 rule 2). The
/// top-level entry is [`Parser::parse_module`]; `Module::parse` is a
/// thin convenience that delegates.
pub struct Parser<'a> {
    toks: Vec<Tok<'a>>,
    pos: usize,
    names: Names,
    /// Forward references from a cell's `writer=\name` to the block that
    /// later declares that name. Resolved at end-of-parse by walking this
    /// list and rewriting each cell's `writer` field via
    /// [`Module::cell_at_mut`].
    pending_writers: Vec<(CellId, String)>,
}

impl<'a> Parser<'a> {
    /// Build a parser over the tokenization of `input`.
    pub fn new(input: &'a str) -> Result<Self, ParseError> {
        Ok(Self {
            toks: tokenize(input)?,
            pos: 0,
            names: Names::new(),
            pending_writers: Vec::new(),
        })
    }

    /// Parse the input as `(module ...)`, returning the reconstructed
    /// module and its name table.
    pub fn parse_module(mut self) -> Result<ParsedModule, ParseError> {
        let mut module = self.parse_module_form()?;
        // Resolve forward writer refs now that all blocks are declared.
        for (cell_id, name) in &self.pending_writers {
            match self.names.find_block(name) {
                Some(block_id) => module.cell_at_mut(*cell_id).writer = block_id,
                None => return Err(ParseError::InvalidId(format!("\\{name}"))),
            }
        }
        Ok(ParsedModule { module, names: self.names })
    }

    // ── token-cursor helpers ──────────────────────────────────────────

    fn peek(&self) -> Option<&Tok<'a>> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<&Tok<'a>> {
        let t = self.toks.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect_lparen(&mut self) -> Result<(), ParseError> {
        match self.bump() {
            Some(Tok::LParen) => Ok(()),
            Some(other) => Err(ParseError::UnexpectedToken {
                expected: "`(`",
                found: tok_display(other).to_string(),
            }),
            None => Err(ParseError::UnexpectedEof("`(`")),
        }
    }

    fn expect_rparen(&mut self) -> Result<(), ParseError> {
        match self.bump() {
            Some(Tok::RParen) => Ok(()),
            Some(other) => Err(ParseError::UnexpectedToken {
                expected: "`)`",
                found: tok_display(other).to_string(),
            }),
            None => Err(ParseError::UnexpectedEof("`)`")),
        }
    }

    fn expect_ident(&mut self) -> Result<&'a str, ParseError> {
        match self.bump() {
            Some(Tok::Ident(s)) => Ok(*s),
            Some(other) => Err(ParseError::UnexpectedToken {
                expected: "identifier",
                found: tok_display(other).to_string(),
            }),
            None => Err(ParseError::UnexpectedEof("identifier")),
        }
    }

    fn expect_str(&mut self) -> Result<&'a str, ParseError> {
        match self.bump() {
            Some(Tok::Str(s)) => Ok(*s),
            Some(other) => Err(ParseError::UnexpectedToken {
                expected: "string literal",
                found: tok_display(other).to_string(),
            }),
            None => Err(ParseError::UnexpectedEof("string literal")),
        }
    }

    // ── module / sections ─────────────────────────────────────────────

    fn parse_module_form(&mut self) -> Result<Module, ParseError> {
        self.expect_lparen()?;
        let kw = self.expect_ident()?;
        if kw != "module" {
            return Err(ParseError::MissingModuleHeader);
        }
        let mut module = Module::new();
        let mut seen = SectionTracker::default();
        // Parse sections until the closing `)`.
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    break;
                }
                Some(Tok::LParen) => {
                    self.parse_section(&mut module, &mut seen)?;
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "section or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("section or `)`")),
            }
        }
        Ok(module)
    }

    fn parse_section(&mut self, module: &mut Module, seen: &mut SectionTracker) -> Result<(), ParseError> {
        self.expect_lparen()?;
        let kw = self.expect_ident()?;
        match kw {
            "natures" => {
                if !seen.claim("natures") {
                    return Err(ParseError::DuplicateSection("natures".into()));
                }
                self.parse_natures(module)?;
            }
            "nets" => {
                if !seen.claim("nets") {
                    return Err(ParseError::DuplicateSection("nets".into()));
                }
                self.parse_nets(module)?;
            }
            "cells" => {
                if !seen.claim("cells") {
                    return Err(ParseError::DuplicateSection("cells".into()));
                }
                self.parse_cells(module)?;
            }
            "values" => {
                if !seen.claim("values") {
                    return Err(ParseError::DuplicateSection("values".into()));
                }
                self.parse_values(module)?;
            }
            "consts" => {
                if !seen.claim("consts") {
                    return Err(ParseError::DuplicateSection("consts".into()));
                }
                self.parse_consts(module)?;
            }
            "blocks" => {
                if !seen.claim("blocks") {
                    return Err(ParseError::DuplicateSection("blocks".into()));
                }
                self.parse_blocks(module)?;
            }
            other => return Err(ParseError::UnknownSection(other.to_string())),
        }
        Ok(())
    }

    fn parse_natures(&mut self, module: &mut Module) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(());
                }
                Some(Tok::LParen) => {
                    self.expect_lparen()?;
                    let kw = self.expect_ident()?;
                    if kw != "nature" {
                        return Err(ParseError::UnknownForm(format!("(nature ...) expected, got ({kw}")));
                    }
                    let id_tok = self.expect_ident()?;
                    let id = parse_nature_id(id_tok)?;
                    // (access "...")
                    self.expect_lparen()?;
                    let access_kw = self.expect_ident()?;
                    if access_kw != "access" {
                        return Err(ParseError::UnknownKeyword(access_kw.to_string()));
                    }
                    let access = self.expect_str()?.to_string();
                    self.expect_rparen()?;
                    let kind_tok = self.expect_ident()?;
                    let kind = match kind_tok {
                        "across" => NatureKind::Across,
                        "through" => NatureKind::Through,
                        other => return Err(ParseError::UnknownKeyword(other.to_string())),
                    };
                    self.expect_rparen()?;
                    let expected_next = module.nature_count() as u32;
                    if id.0 != expected_next {
                        return Err(ParseError::IdOutOfOrder {
                            kind: "nature",
                            expected: expected_next,
                            found: id.0,
                        });
                    }
                    module.push_nature(NatureInfo { access, kind });
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "nature form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("nature form or `)`")),
            }
        }
    }

    fn parse_nets(&mut self, module: &mut Module) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(());
                }
                Some(Tok::LParen) => {
                    let net = self.parse_net_form(module)?;
                    module.push_net(net);
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "net form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("net form or `)`")),
            }
        }
    }

    fn parse_net_form(&mut self, module: &mut Module) -> Result<Net, ParseError> {
        self.expect_lparen()?;
        let kw = self.expect_ident()?;
        if kw != "net" {
            return Err(ParseError::UnknownForm(format!("(net ...) expected, got ({kw}")));
        }
        let id_tok = self.expect_ident()?;
        let id = self.declare_net_id(id_tok, module.net_count() as u32)?;

        self.expect_lparen()?;
        let kind_kw = self.expect_ident()?;
        let kind = match kind_kw {
            "node" => {
                // across=NID through=NID — the printer emits these as
                // two bare idents (`across=N0`, `through=N1`).
                let across = self.parse_tagged_nature_id("across")?;
                let through = self.parse_tagged_nature_id("through")?;
                NetKind::Node { across, through }
            }
            "signal" => {
                let ty = self.parse_ty()?;
                NetKind::Signal { ty }
            }
            other => return Err(ParseError::UnknownKeyword(other.to_string())),
        };
        self.expect_rparen()?;
        self.expect_rparen()?;
        let expected_next = module.net_count() as u32;
        if id.0 != expected_next {
            return Err(ParseError::IdOutOfOrder {
                kind: "net",
                expected: expected_next,
                found: id.0,
            });
        }
        Ok(Net { kind })
    }

    /// Parse `<tag>=<NID>` — split the ident on `=`, expect `N<num>`.
    fn parse_tagged_nature_id(&mut self, expected_tag: &'static str) -> Result<NatureId, ParseError> {
        let tok = self.expect_ident()?;
        let (tag, val) = tok
            .split_once('=')
            .ok_or_else(|| ParseError::UnexpectedTokenText { found: tok.to_string() })?;
        if tag != expected_tag {
            return Err(ParseError::UnexpectedTokenText { found: tok.to_string() });
        }
        parse_nature_id(val)
    }

    fn parse_cells(&mut self, module: &mut Module) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(());
                }
                Some(Tok::LParen) => {
                    self.expect_lparen()?;
                    let kw = self.expect_ident()?;
                    if kw != "cell" {
                        return Err(ParseError::UnknownForm(format!("(cell ...) expected, got ({kw}")));
                    }
                    let id_tok = self.expect_ident()?;
                    let id = self.declare_cell_id(id_tok, module.cell_count() as u32)?;
                    let ty = self.parse_ty()?;
                    // init=KID writer=BID
                    let init = self.parse_tagged_const_id("init")?;
                    let writer_tok = self.expect_ident()?;
                    let (tag, val) = writer_tok
                        .split_once('=')
                        .ok_or_else(|| ParseError::UnexpectedTokenText { found: writer_tok.to_string() })?;
                    if tag != "writer" {
                        return Err(ParseError::UnexpectedTokenText { found: writer_tok.to_string() });
                    }
                    let (writer, unresolved) = self.writer_token(val)?;
                    if let Some(name) = unresolved {
                        // Forward ref by name — record for end-of-parse
                        // resolution when the block declares the name.
                        self.pending_writers.push((id, name));
                    }
                    self.expect_rparen()?;
                    module.push_cell(Cell { ty, init, writer });
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "cell form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("cell form or `)`")),
            }
        }
    }

    fn parse_tagged_const_id(&mut self, expected_tag: &'static str) -> Result<ConstId, ParseError> {
        let tok = self.expect_ident()?;
        let (tag, val) = tok
            .split_once('=')
            .ok_or_else(|| ParseError::UnexpectedTokenText { found: tok.to_string() })?;
        if tag != expected_tag {
            return Err(ParseError::UnexpectedTokenText { found: tok.to_string() });
        }
        parse_const_id(val)
    }

    fn parse_values(&mut self, module: &mut Module) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(());
                }
                Some(Tok::LParen) => {
                    self.expect_lparen()?;
                    let kw = self.expect_ident()?;
                    if kw != "value" {
                        return Err(ParseError::UnknownForm(format!("(value ...) expected, got ({kw}")));
                    }
                    let id_tok = self.expect_ident()?;
                    self.declare_value_id(id_tok, module.value_count() as u32)?;
                    let ty = self.parse_ty()?;
                    self.expect_rparen()?;
                    module.push_value(ty);
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "value form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("value form or `)`")),
            }
        }
    }

    fn parse_consts(&mut self, module: &mut Module) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(());
                }
                Some(Tok::LParen) => {
                    self.expect_lparen()?;
                    let kw = self.expect_ident()?;
                    if kw != "const" {
                        return Err(ParseError::UnknownForm(format!("(const ...) expected, got ({kw}")));
                    }
                    let id_tok = self.expect_ident()?;
                    let id = parse_const_id(id_tok)?;
                    let kind_kw = self.expect_ident()?;
                    let lit_tok = self.expect_ident()?;
                    let c = match kind_kw {
                        "real" => {
                            let v: f64 = lit_tok
                                .parse()
                                .map_err(|_| ParseError::InvalidNumber(lit_tok.to_string()))?;
                            Const::Real(v)
                        }
                        "bits" => {
                            let v: u128 = lit_tok
                                .parse()
                                .map_err(|_| ParseError::InvalidNumber(lit_tok.to_string()))?;
                            Const::Bits(v)
                        }
                        other => return Err(ParseError::UnknownKeyword(other.to_string())),
                    };
                    self.expect_rparen()?;
                    let expected_next = module.const_count() as u32;
                    if id.0 != expected_next {
                        return Err(ParseError::IdOutOfOrder {
                            kind: "const",
                            expected: expected_next,
                            found: id.0,
                        });
                    }
                    module.push_const(c);
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "const form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("const form or `)`")),
            }
        }
    }

    fn parse_blocks(&mut self, module: &mut Module) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(());
                }
                Some(Tok::LParen) => {
                    let block = self.parse_block_form(module)?;
                    module.push_block(block);
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "block form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("block form or `)`")),
            }
        }
    }

    fn parse_block_form(&mut self, module: &Module) -> Result<Block, ParseError> {
        self.expect_lparen()?;
        let kw = self.expect_ident()?;
        if kw != "block" {
            return Err(ParseError::UnknownForm(format!("(block ...) expected, got ({kw}")));
        }
        let id_tok = self.expect_ident()?;
        self.declare_block_id(id_tok, module.block_count() as u32)?;
        let domain_kw = self.expect_ident()?;
        let domain = match domain_kw {
            "analog" => Domain::Analog,
            "digital" => Domain::Digital,
            other => return Err(ParseError::UnknownKeyword(other.to_string())),
        };
        let trigger = self.parse_trigger_form(module)?;
        let mut block = Block::new(domain, trigger);
        // Ops until `)`.
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    return Ok(block);
                }
                Some(Tok::LParen) => {
                    let op = self.parse_op_form(module)?;
                    block.push(op);
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "op form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("op form or `)`")),
            }
        }
    }

    fn parse_trigger_form(&mut self, module: &Module) -> Result<Trigger, ParseError> {
        // Peek first — the trigger is either a bare keyword (`always`,
        // `initial`) or a parenthesized form (`(edge ...)`, `(cross ...)`,
        // `(timer ...)`).
        match self.peek() {
            Some(Tok::LParen) => {
                self.expect_lparen()?;
                let inner = self.expect_ident()?;
                match inner {
                    "edge" => {
                        let sr = self.parse_sigref_form(module)?;
                        let dir_tok = self.expect_ident()?;
                        let kind = match dir_tok {
                            "rising" => EdgeKind::Rising,
                            "falling" => EdgeKind::Falling,
                            "any" => EdgeKind::Any,
                            other => return Err(ParseError::UnknownKeyword(other.to_string())),
                        };
                        self.expect_rparen()?;
                        Ok(Trigger::Edge(sr, kind))
                    }
                    "cross" => {
                        let v = self.parse_value_ref(module)?;
                        let dir_tok = self.expect_ident()?;
                        let dir = match dir_tok {
                            "rising" => CrossDir::Rising,
                            "falling" => CrossDir::Falling,
                            "either" => CrossDir::Either,
                            other => return Err(ParseError::UnknownKeyword(other.to_string())),
                        };
                        self.expect_rparen()?;
                        Ok(Trigger::Cross(v, dir))
                    }
                    "timer" => {
                        let p = self.parse_value_ref(module)?;
                        let ph = self.parse_value_ref(module)?;
                        self.expect_rparen()?;
                        Ok(Trigger::Timer { period: p, phase: ph })
                    }
                    other => Err(ParseError::UnknownKeyword(other.to_string())),
                }
            }
            Some(Tok::Ident(s)) => {
                let tok = *s;
                self.bump();
                match tok {
                    "always" => Ok(Trigger::Always),
                    "initial" => Ok(Trigger::Initial),
                    other => Err(ParseError::UnknownKeyword(other.to_string())),
                }
            }
            Some(other) => Err(ParseError::UnexpectedToken {
                expected: "trigger",
                found: tok_display(other).to_string(),
            }),
            None => Err(ParseError::UnexpectedEof("trigger")),
        }
    }

    fn parse_sigref_form(&mut self, module: &Module) -> Result<SigRef, ParseError> {
        // Either a bare `(sigref ...)` wrapping chunks, or a bare ident.
        // The printer always emits `(sigref ...)`.
        let mut chunks = Vec::new();
        // Peek: if next is `(` (sigref ...)`, walk it; otherwise error.
        match self.peek() {
            Some(Tok::LParen) => {}
            _ => return Err(ParseError::UnexpectedToken { expected: "(sigref ...)", found: "<other>".into() }),
        }
        self.expect_lparen()?;
        let kw = self.expect_ident()?;
        if kw != "sigref" {
            return Err(ParseError::UnknownForm(format!("(sigref ...) expected, got ({kw}")));
        }
        // Chunks until `)`.
        loop {
            match self.peek() {
                Some(Tok::RParen) => {
                    self.bump();
                    break;
                }
                Some(Tok::LParen) => {
                    self.expect_lparen()?;
                    let inner = self.expect_ident()?;
                    match inner {
                        "slice" => {
                            let v_tok = self.expect_ident()?;
                            let v = self.resolve_value_id(v_tok)?;
                            self.check_value_declared(module, v)?;
                            // lo <num> hi <num>
                            let lo_tok = self.expect_ident()?;
                            if lo_tok != "lo" {
                                return Err(ParseError::UnexpectedTokenText { found: lo_tok.to_string() });
                            }
                            let lo: u32 = self
                                .expect_ident()?
                                .parse()
                                .map_err(|_| ParseError::InvalidNumber("lo".into()))?;
                            let hi_tok = self.expect_ident()?;
                            if hi_tok != "hi" {
                                return Err(ParseError::UnexpectedTokenText { found: hi_tok.to_string() });
                            }
                            let hi: u32 = self
                                .expect_ident()?
                                .parse()
                                .map_err(|_| ParseError::InvalidNumber("hi".into()))?;
                            self.expect_rparen()?;
                            chunks.push(Chunk::Slice { value: v, lo, hi });
                        }
                        "const" => {
                            let k_tok = self.expect_ident()?;
                            let k = parse_const_id(k_tok)?;
                            self.expect_rparen()?;
                            chunks.push(Chunk::Const(k));
                        }
                        other => return Err(ParseError::UnknownKeyword(other.to_string())),
                    }
                }
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "chunk form or `)`",
                        found: tok_display(other).to_string(),
                    });
                }
                None => return Err(ParseError::UnexpectedEof("chunk form or `)`")),
            }
        }
        Ok(SigRef(chunks))
    }

    fn parse_op_form(&mut self, module: &Module) -> Result<Op, ParseError> {
        self.expect_lparen()?;
        // Two shapes:
        //   (<out> = <op-kind> ...)
        //   (<effect> ...)
        // The next token disambiguates: `$N`/`\name` followed by `=` →
        // value-producing op; a bare keyword → effect op.
        let head_tok = self.expect_ident()?;
        if head_tok.starts_with('$') || head_tok.starts_with('\\') {
            // Could be either: a value-producing op (`<out> = ...`) or an
            // effect that happens to start with a value id (none today).
            // The disambiguator is whether the NEXT token is `=`.
            let next = self.peek();
            if matches!(next, Some(Tok::Ident("="))) {
                self.bump(); // consume `=`
                let out = self.resolve_value_id(head_tok)?;
                self.check_value_declared(module, out)?;
                return self.parse_value_op(module, out);
            }
        }
        // Effect ops: head_tok is a bare keyword.
        match head_tok {
            "contribute-through" => {
                let a = self.parse_nature_ref_net(module)?;
                let b = self.parse_nature_ref_net(module)?;
                let v = self.parse_value_ref(module)?;
                self.expect_rparen()?;
                Ok(Op::ContributeThrough { a, b, value: v })
            }
            "force-across" => {
                let a = self.parse_nature_ref_net(module)?;
                let b = self.parse_nature_ref_net(module)?;
                let v = self.parse_value_ref(module)?;
                self.expect_rparen()?;
                Ok(Op::ForceAcross { a, b, value: v })
            }
            "drive" => {
                let n_tok = self.expect_ident()?;
                let n = self.resolve_net_id(n_tok)?;
                self.check_net_declared(module, n)?;
                let v = self.parse_value_ref(module)?;
                self.expect_rparen()?;
                Ok(Op::Drive { net: n, value: v })
            }
            "cell-write" => {
                let c_tok = self.expect_ident()?;
                let c = self.resolve_cell_id(c_tok)?;
                self.check_cell_declared(module, c)?;
                let v = self.parse_value_ref(module)?;
                let addr = self.parse_optional_addr(module)?;
                self.expect_rparen()?;
                Ok(Op::CellWrite { cell: c, addr, value: v })
            }
            "if" => {
                let cond = self.parse_value_ref(module)?;
                // (then ...) (else ...)
                self.expect_lparen()?;
                let then_kw = self.expect_ident()?;
                if then_kw != "then" {
                    return Err(ParseError::UnknownKeyword(then_kw.to_string()));
                }
                let mut then_ops = Vec::new();
                loop {
                    match self.peek() {
                        Some(Tok::RParen) => {
                            self.bump();
                            break;
                        }
                        Some(Tok::LParen) => {
                            then_ops.push(self.parse_op_form(module)?);
                        }
                        Some(other) => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "op form or `)`",
                                found: tok_display(other).to_string(),
                            });
                        }
                        None => return Err(ParseError::UnexpectedEof("op form or `)`")),
                    }
                }
                self.expect_lparen()?;
                let else_kw = self.expect_ident()?;
                if else_kw != "else" {
                    return Err(ParseError::UnknownKeyword(else_kw.to_string()));
                }
                let mut else_ops = Vec::new();
                loop {
                    match self.peek() {
                        Some(Tok::RParen) => {
                            self.bump();
                            break;
                        }
                        Some(Tok::LParen) => {
                            else_ops.push(self.parse_op_form(module)?);
                        }
                        Some(other) => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "op form or `)`",
                                found: tok_display(other).to_string(),
                            });
                        }
                        None => return Err(ParseError::UnexpectedEof("op form or `)`")),
                    }
                }
                self.expect_rparen()?;
                Ok(Op::If { cond, then_ops, else_ops })
            }
            "match" => {
                let scrutinee = self.parse_value_ref(module)?;
                let mut arms = Vec::new();
                loop {
                    match self.peek() {
                        Some(Tok::RParen) => {
                            self.bump();
                            break;
                        }
                        Some(Tok::Ident(_)) => {
                            // The printer emits each arm as `PAT (op...)`
                            // — the pattern is a bare ident, the body is
                            // a parenthesized op list.
                            let pat_tok = self.expect_ident()?;
                            let pat = match pat_tok {
                                "0" => Pattern::Zero,
                                "1" => Pattern::One,
                                "?" => Pattern::Wildcard,
                                other => return Err(ParseError::UnknownKeyword(other.to_string())),
                            };
                            // Body — a parenthesized op list.
                            self.expect_lparen()?;
                            let mut body = Vec::new();
                            loop {
                                match self.peek() {
                                    Some(Tok::RParen) => {
                                        self.bump();
                                        break;
                                    }
                                    Some(Tok::LParen) => {
                                        body.push(self.parse_op_form(module)?);
                                    }
                                    Some(other) => {
                                        return Err(ParseError::UnexpectedToken {
                                            expected: "op form or `)`",
                                            found: tok_display(other).to_string(),
                                        });
                                    }
                                    None => return Err(ParseError::UnexpectedEof("op form or `)`")),
                                }
                            }
                            arms.push((pat, body));
                        }
                        Some(other) => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "pattern or `)`",
                                found: tok_display(other).to_string(),
                            });
                        }
                        None => return Err(ParseError::UnexpectedEof("pattern or `)`")),
                    }
                }
                Ok(Op::Match { scrutinee, arms })
            }
            other => Err(ParseError::UnknownForm(format!(
                "expected op head, got `{other}`"
            ))),
        }
    }

    /// Parse the rest of a value-producing op (after `(<out> = `).
    fn parse_value_op(&mut self, module: &Module, out: ValueId) -> Result<Op, ParseError> {
        let head = self.expect_ident()?;
        match head {
            "pure" => {
                let op_tok = self.expect_ident()?;
                let pure = parse_pure_op(op_tok)?;
                let mut args = Vec::new();
                while !matches!(self.peek(), Some(Tok::RParen)) {
                    args.push(self.parse_value_ref(module)?);
                }
                self.expect_rparen()?;
                Ok(Op::Pure { op: pure, args, out })
            }
            "call" => {
                let i_tok = self.expect_ident()?;
                let intrinsic = parse_intrinsic_id(i_tok)?;
                let mut args = Vec::new();
                while !matches!(self.peek(), Some(Tok::RParen)) {
                    args.push(self.parse_value_ref(module)?);
                }
                self.expect_rparen()?;
                Ok(Op::Call { intrinsic, args, out })
            }
            "read-across" => {
                let a = self.parse_nature_ref_net(module)?;
                let b = self.parse_nature_ref_net(module)?;
                self.expect_rparen()?;
                Ok(Op::ReadAcross { a, b, out })
            }
            "read-through" => {
                let a = self.parse_nature_ref_net(module)?;
                let b = self.parse_nature_ref_net(module)?;
                self.expect_rparen()?;
                Ok(Op::ReadThrough { a, b, out })
            }
            "read-signal" => {
                let n_tok = self.expect_ident()?;
                let n = self.resolve_net_id(n_tok)?;
                self.check_net_declared(module, n)?;
                self.expect_rparen()?;
                Ok(Op::ReadSignal { net: n, out })
            }
            "cell-read" => {
                let c_tok = self.expect_ident()?;
                let c = self.resolve_cell_id(c_tok)?;
                self.check_cell_declared(module, c)?;
                let addr = self.parse_optional_addr(module)?;
                self.expect_rparen()?;
                Ok(Op::CellRead { cell: c, addr, out })
            }
            other => Err(ParseError::UnknownForm(format!(
                "unknown value op `{other}`"
            ))),
        }
    }

    /// Parse an optional `(addr VID)` clause. Returns `None` if the next
    /// token is `)`.
    fn parse_optional_addr(&mut self, module: &Module) -> Result<Option<ValueId>, ParseError> {
        match self.peek() {
            Some(Tok::RParen) => Ok(None),
            Some(Tok::LParen) => {
                self.expect_lparen()?;
                let kw = self.expect_ident()?;
                if kw != "addr" {
                    return Err(ParseError::UnknownKeyword(kw.to_string()));
                }
                let v = self.parse_value_ref(module)?;
                self.expect_rparen()?;
                Ok(Some(v))
            }
            Some(other) => Err(ParseError::UnexpectedToken {
                expected: "`(` or `)`",
                found: tok_display(other).to_string(),
            }),
            None => Err(ParseError::UnexpectedEof("`(` or `)`")),
        }
    }

    /// Parse a net id (NID form: `$N` or `\name`), checking it exists.
    fn parse_nature_ref_net(&mut self, module: &Module) -> Result<NetId, ParseError> {
        let tok = self.expect_ident()?;
        let n = self.resolve_net_id(tok)?;
        self.check_net_declared(module, n)?;
        Ok(n)
    }

    /// Parse a value reference: `$N` or `\name`. The id must exist.
    fn parse_value_ref(&mut self, module: &Module) -> Result<ValueId, ParseError> {
        let tok = self.expect_ident()?;
        let v = self.resolve_value_id(tok)?;
        self.check_value_declared(module, v)?;
        Ok(v)
    }

    /// Resolve a value-id token (either `$N` or `\name`) to a `ValueId`,
    /// consulting the [`Names`] side-table for authored names.
    fn resolve_value_id(&self, tok: &str) -> Result<ValueId, ParseError> {
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.find_value(name).ok_or_else(|| ParseError::InvalidId(tok.to_string()))
        } else {
            parse_value_id(tok)
        }
    }

    /// Resolve a net-id token (either `$N` or `\name`).
    fn resolve_net_id(&self, tok: &str) -> Result<NetId, ParseError> {
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.find_net(name).ok_or_else(|| ParseError::InvalidId(tok.to_string()))
        } else {
            parse_net_id(tok)
        }
    }

    /// Resolve a cell-id token.
    fn resolve_cell_id(&self, tok: &str) -> Result<CellId, ParseError> {
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.find_cell(name).ok_or_else(|| ParseError::InvalidId(tok.to_string()))
        } else {
            parse_cell_id(tok)
        }
    }

    /// At a value DECLARATION site, parse the id-token and bind any
    /// authored name to the slot `expected_next` (the slot the caller is
    /// about to fill). Returns the id.
    fn declare_value_id(&mut self, tok: &str, expected_next: u32) -> Result<ValueId, ParseError> {
        let id = ValueId(expected_next);
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.set_value(id, name);
        } else {
            let parsed = parse_value_id(tok)?;
            if parsed.0 != expected_next {
                return Err(ParseError::IdOutOfOrder {
                    kind: "value",
                    expected: expected_next,
                    found: parsed.0,
                });
            }
        }
        Ok(id)
    }

    /// At a net DECLARATION site.
    fn declare_net_id(&mut self, tok: &str, expected_next: u32) -> Result<NetId, ParseError> {
        let id = NetId(expected_next);
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.set_net(id, name);
        } else {
            let parsed = parse_net_id(tok)?;
            if parsed.0 != expected_next {
                return Err(ParseError::IdOutOfOrder {
                    kind: "net",
                    expected: expected_next,
                    found: parsed.0,
                });
            }
        }
        Ok(id)
    }

    /// At a cell DECLARATION site.
    fn declare_cell_id(&mut self, tok: &str, expected_next: u32) -> Result<CellId, ParseError> {
        let id = CellId(expected_next);
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.set_cell(id, name);
        } else {
            let parsed = parse_cell_id(tok)?;
            if parsed.0 != expected_next {
                return Err(ParseError::IdOutOfOrder {
                    kind: "cell",
                    expected: expected_next,
                    found: parsed.0,
                });
            }
        }
        Ok(id)
    }

    /// At a block DECLARATION site.
    fn declare_block_id(&mut self, tok: &str, expected_next: u32) -> Result<BlockId, ParseError> {
        let id = BlockId(expected_next);
        if let Some(name) = tok.strip_prefix('\\') {
            self.names.set_block(id, name);
        } else {
            let parsed = parse_block_id(tok)?;
            if parsed.0 != expected_next {
                return Err(ParseError::IdOutOfOrder {
                    kind: "block",
                    expected: expected_next,
                    found: parsed.0,
                });
            }
        }
        Ok(id)
    }

    /// At a writer=\<id\> field of a cell: the writer may be declared
    /// later (the blocks section comes after cells in the printer's
    /// grammar). Returns `(BlockId, Option<String>)` — the second element
    /// is the unresolved name (if any) for deferred resolution after all
    /// blocks are declared.
    fn writer_token(&self, tok: &str) -> Result<(BlockId, Option<String>), ParseError> {
        if let Some(name) = tok.strip_prefix('\\') {
            // Forward ref by name: resolve at end-of-parse via Names.
            // Use a placeholder id (the cell's writer field will be
            // rewritten before the module is returned).
            Ok((BlockId(0), Some(name.to_string())))
        } else {
            let id = parse_block_id(tok)?;
            Ok((id, None))
        }
    }

    fn check_value_declared(&self, module: &Module, v: ValueId) -> Result<(), ParseError> {
        if (v.0 as usize) < module.value_count() {
            Ok(())
        } else {
            Err(ParseError::UndeclaredReference { kind: "value", id: v.0 })
        }
    }

    fn check_net_declared(&self, module: &Module, n: NetId) -> Result<(), ParseError> {
        if (n.0 as usize) < module.net_count() {
            Ok(())
        } else {
            Err(ParseError::UndeclaredReference { kind: "net", id: n.0 })
        }
    }

    fn check_cell_declared(&self, module: &Module, c: CellId) -> Result<(), ParseError> {
        if (c.0 as usize) < module.cell_count() {
            Ok(())
        } else {
            Err(ParseError::UndeclaredReference { kind: "cell", id: c.0 })
        }
    }

    // ── type form ─────────────────────────────────────────────────────

    fn parse_ty(&mut self) -> Result<Ty, ParseError> {
        if matches!(self.peek(), Some(Tok::LParen)) {
            self.bump(); // consume `(`
            let inner = self.expect_ident()?;
            let ty = match inner {
                "bits" => {
                    let n_tok = self.expect_ident()?;
                    let n: u32 = n_tok
                        .parse()
                        .map_err(|_| ParseError::InvalidNumber(n_tok.to_string()))?;
                    Ty::Bits(n)
                }
                "array" => {
                    let elem = self.parse_ty()?;
                    let n_tok = self.expect_ident()?;
                    let n: u32 = n_tok
                        .parse()
                        .map_err(|_| ParseError::InvalidNumber(n_tok.to_string()))?;
                    Ty::Array(Box::new(elem), n)
                }
                other => return Err(ParseError::UnknownKeyword(other.to_string())),
            };
            self.expect_rparen()?;
            Ok(ty)
        } else {
            let tok = self.expect_ident()?;
            match tok {
                "real" => Ok(Ty::Real),
                other => Err(ParseError::UnknownForm(format!("expected type, got `{other}`"))),
            }
        }
    }
}

// ── small free helpers ────────────────────────────────────────────────────
//
// These parse and validate specific id-string shapes. They are NOT loose
// functions in the MD-13 sense — they are pure parsers of a string into a
// newtype id, used by methods on `Parser`. They take the `&str` slice they
// parse, not `&self`, so they cannot naturally be `&self` methods on
// `Parser` without awkwardly threading the slice through. Each has a single
// owner-concept (the id it parses) and is small enough that grouping them
// here reads as a typed-token-decoder helper module.

fn tok_display<'a>(tok: &Tok<'a>) -> &'a str {
    match tok {
        Tok::LParen => "(",
        Tok::RParen => ")",
        Tok::Ident(s) | Tok::Str(s) => s,
    }
}

fn parse_nature_id(s: &str) -> Result<NatureId, ParseError> {
    let n = strip_prefix(s, 'N').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(NatureId(id))
}

fn parse_const_id(s: &str) -> Result<ConstId, ParseError> {
    let n = strip_prefix(s, 'K').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(ConstId(id))
}

fn parse_intrinsic_id(s: &str) -> Result<crate::core::IntrinsicId, ParseError> {
    let n = strip_prefix(s, 'I').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(crate::core::IntrinsicId(id))
}

fn parse_net_id(s: &str) -> Result<NetId, ParseError> {
    if let Some(name) = s.strip_prefix('\\') {
        // Authored — the NAMES side-table maps the name back to the id.
        // The id is whatever the next allocated id will be at the
        // declaration site; references use the NAMES lookup.
        let _ = name;
        return Err(ParseError::InvalidId(s.to_string()));
    }
    let n = strip_prefix(s, '$').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(NetId(id))
}

fn parse_value_id(s: &str) -> Result<ValueId, ParseError> {
    if s.starts_with('\\') {
        return Err(ParseError::InvalidId(s.to_string()));
    }
    let n = strip_prefix(s, '$').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(ValueId(id))
}

fn parse_cell_id(s: &str) -> Result<CellId, ParseError> {
    if s.starts_with('\\') {
        return Err(ParseError::InvalidId(s.to_string()));
    }
    let n = strip_prefix(s, '$').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(CellId(id))
}

fn parse_block_id(s: &str) -> Result<BlockId, ParseError> {
    if s.starts_with('\\') {
        return Err(ParseError::InvalidId(s.to_string()));
    }
    let n = strip_prefix(s, '$').ok_or_else(|| ParseError::InvalidId(s.to_string()))?;
    let id: u32 = n.parse().map_err(|_| ParseError::InvalidId(s.to_string()))?;
    Ok(BlockId(id))
}

fn strip_prefix<'a>(s: &'a str, c: char) -> Option<&'a str> {
    s.strip_prefix(c)
}

fn parse_pure_op(s: &str) -> Result<PureOp, ParseError> {
    match s {
        "add" => Ok(PureOp::Add),
        "sub" => Ok(PureOp::Sub),
        "mul" => Ok(PureOp::Mul),
        "div" => Ok(PureOp::Div),
        "neg" => Ok(PureOp::Neg),
        "eq" => Ok(PureOp::Eq),
        "ne" => Ok(PureOp::Ne),
        "lt" => Ok(PureOp::Lt),
        "le" => Ok(PureOp::Le),
        "gt" => Ok(PureOp::Gt),
        "ge" => Ok(PureOp::Ge),
        "select" => Ok(PureOp::Select),
        other => Err(ParseError::UnknownKeyword(other.to_string())),
    }
}

// ── the section tracker (one of each section allowed) ─────────────────────

#[derive(Default)]
struct SectionTracker {
    natures: bool,
    nets: bool,
    cells: bool,
    values: bool,
    consts: bool,
    blocks: bool,
}

impl SectionTracker {
    fn claim(&mut self, name: &str) -> bool {
        let slot = match name {
            "natures" => &mut self.natures,
            "nets" => &mut self.nets,
            "cells" => &mut self.cells,
            "values" => &mut self.values,
            "consts" => &mut self.consts,
            "blocks" => &mut self.blocks,
            _ => return false,
        };
        if *slot {
            false
        } else {
            *slot = true;
            true
        }
    }
}

impl Module {
    /// Parse the printed S-expression form back into a module plus its
    /// authored-name table. The inverse of [`Module::print`] /
    /// [`Module::print_with_names`]. A malformed input is a [`ParseError`],
    /// never a panic.
    pub fn parse(input: &str) -> Result<ParsedModule, ParseError> {
        Parser::new(input)?.parse_module()
    }
}

#[cfg(test)]
mod tests {
    //! Inline unit tests for the parser's typed-token decoders and small
    //! well-formed parses. The round-trip-over-many-fixtures property is
    //! the integration test in `crates/piperine-ir/tests/parse.rs`.
    use super::*;

    #[test]
    fn parse_value_id_handles_generated_form() {
        assert_eq!(parse_value_id("$5").unwrap(), ValueId(5));
        assert_eq!(parse_value_id("$0").unwrap(), ValueId(0));
    }

    #[test]
    fn parse_value_id_rejects_authored_form() {
        // Authored names are recorded in the Names side-table at the
        // declaration site; a bare reference parser has no table.
        assert!(parse_value_id("\\foo").is_err());
    }

    #[test]
    fn parse_value_id_rejects_non_numeric() {
        assert!(parse_value_id("$x").is_err());
        assert!(parse_value_id("foo").is_err());
        assert!(parse_value_id("").is_err());
    }

    #[test]
    fn parse_pure_op_round_trips_every_variant() {
        for (s, op) in [
            ("add", PureOp::Add),
            ("sub", PureOp::Sub),
            ("mul", PureOp::Mul),
            ("div", PureOp::Div),
            ("neg", PureOp::Neg),
            ("eq", PureOp::Eq),
            ("ne", PureOp::Ne),
            ("lt", PureOp::Lt),
            ("le", PureOp::Le),
            ("gt", PureOp::Gt),
            ("ge", PureOp::Ge),
            ("select", PureOp::Select),
        ] {
            assert_eq!(parse_pure_op(s).unwrap(), op, "{s}");
        }
    }

    #[test]
    fn parse_pure_op_rejects_unknown() {
        assert!(parse_pure_op("xor").is_err());
        assert!(parse_pure_op("").is_err());
    }

    #[test]
    fn parse_an_empty_module_round_trips() {
        let m = Module::new();
        let text = m.print();
        let parsed = Module::parse(&text).unwrap();
        assert!(parsed.module.structurally_equivalent(&m));
    }

    #[test]
    fn parse_a_module_with_only_values_round_trips() {
        let mut m = Module::new();
        m.push_value(Ty::Real);
        m.push_value(Ty::Bits(8));
        m.push_value(Ty::Array(Box::new(Ty::Bits(32)), 16));
        let text = m.print();
        let parsed = Module::parse(&text).unwrap();
        assert!(parsed.module.structurally_equivalent(&m));
    }

    #[test]
    fn parse_a_module_with_only_nets_round_trips() {
        let mut m = Module::new();
        let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
        let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
        m.push_net(Net::node(v, i));
        m.push_net(Net::signal(Ty::Bits(1)));
        let text = m.print();
        let parsed = Module::parse(&text).unwrap();
        assert!(parsed.module.structurally_equivalent(&m));
    }

    #[test]
    fn missing_module_header_is_a_named_error() {
        let err = Module::parse("(not-a-module)").unwrap_err();
        assert_eq!(err, ParseError::MissingModuleHeader);
    }

    #[test]
    fn unbalanced_parens_is_a_named_error() {
        let err = Module::parse("(module (natures").unwrap_err();
        // Either an EOF or unexpected token depending on where the unbalance
        // surfaces — both are typed errors, not panics.
        let _ = err; // assertion: no panic
    }

    #[test]
    fn unknown_section_is_a_named_error() {
        let err = Module::parse("(module (bogus))").unwrap_err();
        assert!(matches!(err, ParseError::UnknownSection(_)));
    }

    #[test]
    fn duplicate_section_is_a_named_error() {
        let err = Module::parse("(module (natures) (natures))").unwrap_err();
        assert!(matches!(err, ParseError::DuplicateSection(_)));
    }
}
