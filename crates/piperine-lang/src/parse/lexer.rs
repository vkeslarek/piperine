//! # Lexer
//!
//! Converts raw source text into a flat sequence of [`Lexed`] tokens.
//!
//! ## Phase contract
//!
//! **Input**: `&str` — raw UTF-8 PHDL source.
//! **Output**: `Vec<Lexed>` — tokens with byte-range spans.
//!
//! ## What the lexer does NOT do
//!
//! - It does **not** distinguish keywords from identifiers at the token level.
//!   All identifiers (including `mod`, `fn`, `for`, …) are emitted as
//!   `Tok::Ident`. The parser matches keyword spellings with [`eat_ident`].
//! - It does **not** validate that integer or real literals fit any particular
//!   type — that is a semantic concern.
//! - It does **not** track newlines for error messages (V1: byte offsets only).

use std::str::FromStr;

/// A single PHDL token.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    // ── Literals ─────────────────────────────────────────────────────────────
    Ident(String),
    SysCall(String), // $ident
    Real(f64),
    Int(u64),
    Quad(String), // 0q{0,1,X,Z}
    Str(String),

    // ── Punctuation ──────────────────────────────────────────────────────────
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Colon,
    DoubleColon,
    Dot,
    Assign,   // =
    FatArrow, // =>
    Arrow,    // ->
    DotDot,   // ..
    DotDotEq, // ..=

    // ── Operators ────────────────────────────────────────────────────────────
    Contrib, // <+
    Force,   // <-
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Not,
    And, // &&  (not in PHDL grammar, lexed for error clarity)
    Or,  // ||  (not in PHDL grammar, lexed for error clarity)
    BitAnd,
    BitOr,
    BitXor,
    At,
}

/// A token together with its source byte range.
#[derive(Debug, Clone)]
pub struct Lexed {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
}

/// Converts a PHDL source string into a token sequence.
///
/// Whitespace and comments (`//` line, `/* */` block) are skipped.
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let start = self.pos;
            while self.peek_char().map_or(false, |c| c.is_whitespace()) {
                self.advance();
            }
            if self.input[self.pos..].starts_with("//") {
                while let Some(c) = self.peek_char() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
            } else if self.input[self.pos..].starts_with("/*") {
                self.advance();
                self.advance();
                while let Some(c) = self.advance() {
                    if c == '*' && self.match_char('/') {
                        break;
                    }
                }
            }
            if self.pos == start {
                break;
            }
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Lexed>, String> {
        let mut tokens = Vec::new();
        self.skip_whitespace_and_comments();
        while self.pos < self.input.len() {
            let start = self.pos;
            let c = self.advance().unwrap();

            let tok = match c {
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                '[' => Tok::LBrack,
                ']' => Tok::RBrack,
                '{' => Tok::LBrace,
                '}' => Tok::RBrace,
                ',' => Tok::Comma,
                ';' => Tok::Semi,
                '@' => Tok::At,
                ':' => {
                    if self.match_char(':') { Tok::DoubleColon } else { Tok::Colon }
                }
                '.' => {
                    if self.match_char('.') {
                        if self.match_char('=') { Tok::DotDotEq } else { Tok::DotDot }
                    } else {
                        Tok::Dot
                    }
                }
                '=' => {
                    if self.match_char('=') { Tok::EqEq }
                    else if self.match_char('>') { Tok::FatArrow }
                    else { Tok::Assign }
                }
                '-' => {
                    if self.match_char('>') { Tok::Arrow } else { Tok::Minus }
                }
                '+' => Tok::Plus,
                '*' => Tok::Star,
                '/' => Tok::Slash,
                '%' => Tok::Percent,
                '!' => {
                    if self.match_char('=') { Tok::NotEq } else { Tok::Not }
                }
                '<' => {
                    if self.match_char('=') { Tok::Le }
                    else if self.match_char('+') { Tok::Contrib }
                    else if self.match_char('-') { Tok::Force }
                    else { Tok::Lt }
                }
                '>' => {
                    if self.match_char('=') { Tok::Ge } else { Tok::Gt }
                }
                '&' => {
                    if self.match_char('&') { Tok::And } else { Tok::BitAnd }
                }
                '|' => {
                    if self.match_char('|') { Tok::Or } else { Tok::BitOr }
                }
                '^' => Tok::BitXor,
                '$' => {
                    let mut ident = String::new();
                    while self.peek_char().map_or(false, |c| c.is_ascii_alphanumeric() || c == '_') {
                        ident.push(self.advance().unwrap());
                    }
                    Tok::SysCall(ident)
                }
                '"' => {
                    let mut s = String::new();
                    while let Some(c) = self.advance() {
                        if c == '"' {
                            break;
                        }
                        s.push(c);
                    }
                    Tok::Str(s)
                }
                c if c.is_ascii_digit() => self.lex_number(c, start)?,
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let mut ident = String::new();
                    ident.push(c);
                    while self.peek_char().map_or(false, |c| c.is_ascii_alphanumeric() || c == '_') {
                        ident.push(self.advance().unwrap());
                    }
                    Tok::Ident(ident)
                }
                _ => return Err(format!("Unexpected character '{}' at byte {}", c, start)),
            };

            tokens.push(Lexed { tok, start, end: self.pos });
            self.skip_whitespace_and_comments();
        }
        Ok(tokens)
    }

    fn lex_number(&mut self, first: char, start: usize) -> Result<Tok, String> {
        let mut num = String::new();
        num.push(first);
        let mut is_real = false;
        let mut radix: u32 = 10;

        if first == '0' {
            match self.peek_char() {
                Some('b') | Some('B') => { radix = 2; self.advance(); }
                Some('x') | Some('X') => { radix = 16; self.advance(); }
                Some('o') | Some('O') => { radix = 8; self.advance(); }
                Some('q') | Some('Q') => { radix = 4; self.advance(); }
                _ => {}
            }
        }

        loop {
            match self.peek_char() {
                None => break,
                Some(c) if radix == 4 => {
                    match c {
                        '0' | '1' | 'x' | 'X' | 'z' | 'Z' => { num.push(self.advance().unwrap()); }
                        '_' => { self.advance(); }
                        _ => break,
                    }
                }
                Some(c) if radix != 10 => {
                    if c.is_ascii_hexdigit() { num.push(self.advance().unwrap()); }
                    else if c == '_' { self.advance(); }
                    else { break; }
                }
                Some(c) if c.is_ascii_digit() || c == '_' => {
                    if c != '_' { num.push(self.advance().unwrap()); } else { self.advance(); }
                }
                Some('.') => {
                    // Don't consume '..' range operators.
                    let next = self.input[self.pos + 1..].chars().next();
                    if next == Some('.') { break; }
                    is_real = true;
                    num.push(self.advance().unwrap());
                }
                Some('e') | Some('E') => {
                    is_real = true;
                    num.push(self.advance().unwrap());
                    if matches!(self.peek_char(), Some('+') | Some('-')) {
                        num.push(self.advance().unwrap());
                    }
                }
                _ => break,
            }
        }

        if is_real {
            Ok(Tok::Real(
                f64::from_str(&num)
                    .map_err(|e| format!("Invalid real literal '{}': {}", num, e))?,
            ))
        } else if radix == 4 {
            let digits = &num[1..];
            if digits.is_empty() {
                return Err(format!("Empty quad literal at byte {}", start));
            }
            Ok(Tok::Quad(digits.to_string()))
        } else if radix == 10 {
            Ok(Tok::Int(
                u64::from_str(&num)
                    .map_err(|e| format!("Invalid integer literal '{}': {}", num, e))?,
            ))
        } else {
            let digits = &num[1..];
            if digits.is_empty() {
                return Err(format!(
                    "Empty {} literal at byte {}",
                    match radix { 2 => "binary", 8 => "octal", 16 => "hex", _ => "?" },
                    start
                ));
            }
            Ok(Tok::Int(
                u64::from_str_radix(digits, radix)
                    .map_err(|e| format!("Invalid integer literal '{}': {}", num, e))?,
            ))
        }
    }
}
