//! Lexer for Verilog-AMS LRM 2.4 chapter 2 (lexical conventions). Written
//! from `VAMS-LRM-2-4.md` §2.2-2.8 only (no third-party compiler consulted
//! — `tests/purity.rs` R1 gate). Tokenizes the seven lexical-token classes
//! §2.2 lists (white space, comment, operator, number, string, identifier,
//! keyword) — keyword-vs-identifier classification is deferred to the
//! Annex B keyword table (`keywords.rs`), since §2.8.2 defines a keyword as
//! "a predefined simple identifier", lexically identical to any other.
//!
//! Newlines are kept as their own token (not folded into whitespace) — the
//! preprocessor (§10) needs line boundaries for `` `define``/`` `ifdef``
//! rescanning, and this is the newline-preserving property the design
//! (design.md §4.1) requires of this module.

use crate::error::{ParseError, ParseErrorKind, Span, UnderscorePosition};

/// One lexical token with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Lexed {
    pub tok: Tok,
    pub span: Span,
}

/// A base-4 (2/4/8/16) number literal, LRM §2.6.1 Syntax 2-2. `digits` is
/// the raw digit text after the base letter, preserved verbatim — it may
/// contain `x`/`X`/`z`/`Z`/`?` (four-state) and `_` separators. Resolving
/// four-state digits into a concrete numeric type is a const-evaluation
/// concern (elaboration), not a lexical one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasedNumber {
    pub size: Option<u32>,
    pub signed: bool,
    pub base: NumberBase,
    pub digits: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberBase {
    Decimal,
    Binary,
    Octal,
    Hex,
}

/// A number token, LRM §2.6 Syntax 2-2.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberLit {
    /// Unsized decimal integer, e.g. `42`.
    Integer(i64),
    /// A based integer literal `[size]'[s]base digits`, e.g. `8'hFF`.
    Based(BasedNumber),
    /// A real (floating point) constant, LRM §2.6.2. The exponent form and
    /// the Table 2-1 SI scale-factor form are both folded into a single
    /// `f64` at lex time (the scale factor is part of the `real_number`
    /// grammar production itself, not a separate token).
    Real(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    /// LRM §2.8.1 — leading `\` and terminating white space are not part
    /// of the identifier's value.
    EscapedIdent(String),
    /// `$name` — LRM §2.8.3, system task/function identifier.
    SystemIdent(String),
    /// `` `name `` — LRM §2.8.4, compiler directive identifier.
    Directive(String),
    Number(NumberLit),
    Str(String),
    /// A maximal-munch operator/punctuation lexeme (LRM §2.5: operators are
    /// 1-3 character sequences; exact per-operator semantics belong to
    /// chapter 4's grammar, not the lexer).
    Punct(&'static str),
    /// Kept distinct from other white space so the preprocessor (§10) can
    /// see line boundaries for macro/conditional rescanning.
    Newline,
    LineComment(String),
    BlockComment(String),
    Eof,
}

/// Longest-first table of Verilog-AMS/1364-2005 operator and punctuation
/// lexemes. Maximal munch: the lexer always prefers the longest match.
const PUNCT_TABLE: &[&str] = &[
    // 3-character
    "<<<", ">>>", "<->", "===", "!==",
    // 2-character
    "<+", "->", "<=", ">=", "==", "!=", "&&", "||", "**", "<<", ">>", "~&", "~|", "~^", "^~",
    "+:", "-:", "::",
    // §2.9.1's attribute-instance delimiters. Tokenizing these as their own
    // 2-character lexemes (rather than relying on the grammar layer to
    // disambiguate `(`+`*` / `*`+`)` from multiplication) is what lets
    // `a * b` parse as ordinary multiplication *inside* an attribute value
    // (`(* size = a*b *)`) while `*)` still closes the attribute instance —
    // maximal munch only merges the trailing `*` into `*)` when a `)`
    // immediately follows it, exactly the position where multiplication's
    // right operand can never be.
    "(*", "*)",
    // 1-character
    "+", "-", "*", "/", "%", "<", ">", "=", "!", "&", "|", "^", "~", "?", ":", ";", ",", ".",
    "(", ")", "[", "]", "{", "}", "@", "#",
];

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { src, bytes: src.as_bytes(), pos: 0 }
    }

    /// Tokenize the entire input, stopping at the first lexical error.
    pub fn tokenize(mut self) -> Result<Vec<Lexed>, ParseError> {
        let mut out = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = matches!(tok.tok, Tok::Eof);
            out.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(out)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn next_token(&mut self) -> Result<Lexed, ParseError> {
        self.skip_inline_whitespace();

        let start = self.pos;
        let Some(b) = self.peek() else {
            return Ok(Lexed { tok: Tok::Eof, span: Span::new(start, start) });
        };

        match b {
            b'\n' => {
                self.pos += 1;
                Ok(Lexed { tok: Tok::Newline, span: Span::new(start, self.pos) })
            }
            b'\r' => {
                self.pos += 1;
                if self.peek() == Some(b'\n') {
                    self.pos += 1;
                }
                Ok(Lexed { tok: Tok::Newline, span: Span::new(start, self.pos) })
            }
            b'/' if self.peek_at(1) == Some(b'/') => self.lex_line_comment(start),
            b'/' if self.peek_at(1) == Some(b'*') => self.lex_block_comment(start),
            b'"' => self.lex_string(start),
            b'\\' => self.lex_escaped_identifier(start),
            b'$' => self.lex_prefixed_ident(start, b'$', Tok::SystemIdent as fn(String) -> Tok),
            b'`' => self.lex_prefixed_ident(start, b'`', Tok::Directive as fn(String) -> Tok),
            b'0'..=b'9' => self.lex_number(start),
            b'\'' => self.lex_based_number_no_size(start),
            b if is_ident_start(b) => self.lex_ident(start),
            _ => self.lex_punct(start),
        }
    }

    /// Spaces, tabs and formfeeds only — newlines are their own token.
    fn skip_inline_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == 0x0c {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn lex_line_comment(&mut self, start: usize) -> Result<Lexed, ParseError> {
        self.pos += 2; // consume `//`
        let text_start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b'\r' {
                break;
            }
            self.pos += 1;
        }
        let text = self.src[text_start..self.pos].to_string();
        Ok(Lexed { tok: Tok::LineComment(text), span: Span::new(start, self.pos) })
    }

    fn lex_block_comment(&mut self, start: usize) -> Result<Lexed, ParseError> {
        self.pos += 2; // consume `/*`
        let text_start = self.pos;
        loop {
            match self.peek() {
                None => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnterminatedComment,
                        Span::new(start, self.pos),
                    ));
                }
                Some(b'*') if self.peek_at(1) == Some(b'/') => {
                    let text = self.src[text_start..self.pos].to_string();
                    self.pos += 2;
                    return Ok(Lexed {
                        tok: Tok::BlockComment(text),
                        span: Span::new(start, self.pos),
                    });
                }
                Some(_) => self.pos += 1,
            }
        }
    }

    /// LRM §2.7, Table 2-2 escape sequences.
    fn lex_string(&mut self, start: usize) -> Result<Lexed, ParseError> {
        self.pos += 1; // consume opening `"`
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') | Some(b'\r') => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnterminatedString,
                        Span::new(start, self.pos),
                    ));
                }
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(Lexed { tok: Tok::Str(value), span: Span::new(start, self.pos) });
                }
                Some(b'\\') => {
                    let esc_start = self.pos;
                    self.pos += 1;
                    match self.peek() {
                        Some(b'n') => {
                            value.push('\n');
                            self.pos += 1;
                        }
                        Some(b't') => {
                            value.push('\t');
                            self.pos += 1;
                        }
                        Some(b'\\') => {
                            value.push('\\');
                            self.pos += 1;
                        }
                        Some(b'"') => {
                            value.push('"');
                            self.pos += 1;
                        }
                        Some(c) if c.is_ascii_digit() && c <= b'7' => {
                            let mut digits = String::new();
                            for _ in 0..3 {
                                match self.peek() {
                                    Some(d) if d.is_ascii_digit() && d <= b'7' => {
                                        digits.push(d as char);
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            let code = u8::from_str_radix(&digits, 8).unwrap_or(0);
                            value.push(code as char);
                        }
                        Some(c) => {
                            return Err(ParseError::new(
                                ParseErrorKind::InvalidEscape { found: c as char },
                                Span::new(esc_start, self.pos + 1),
                            ));
                        }
                        None => {
                            return Err(ParseError::new(
                                ParseErrorKind::UnterminatedString,
                                Span::new(start, self.pos),
                            ));
                        }
                    }
                }
                Some(c) => {
                    value.push(c as char);
                    self.pos += 1;
                }
            }
        }
    }

    /// LRM §2.8.1 — starts with `\`, ends with white space (space, tab,
    /// newline, formfeed). The backslash and terminating white space are
    /// not part of the identifier's value.
    fn lex_escaped_identifier(&mut self, start: usize) -> Result<Lexed, ParseError> {
        self.pos += 1; // consume `\`
        let text_start = self.pos;
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' || b == 0x0c {
                break;
            }
            self.pos += 1;
        }
        let name = self.src[text_start..self.pos].to_string();
        Ok(Lexed { tok: Tok::EscapedIdent(name), span: Span::new(start, self.pos) })
    }

    /// `$name` (§2.8.3) or `` `name `` (§2.8.4) — a prefix byte followed by
    /// a simple-identifier-shaped name.
    fn lex_prefixed_ident(
        &mut self,
        start: usize,
        prefix: u8,
        make: fn(String) -> Tok,
    ) -> Result<Lexed, ParseError> {
        self.pos += 1; // consume the prefix byte
        let text_start = self.pos;
        while let Some(b) = self.peek() {
            if is_ident_continue(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == text_start {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedCharacter { found: prefix as char },
                Span::new(start, self.pos),
            ));
        }
        let name = self.src[text_start..self.pos].to_string();
        Ok(Lexed { tok: make(name), span: Span::new(start, self.pos) })
    }

    fn lex_ident(&mut self, start: usize) -> Result<Lexed, ParseError> {
        while let Some(b) = self.peek() {
            if is_ident_continue(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = self.src[start..self.pos].to_string();
        Ok(Lexed { tok: Tok::Ident(text), span: Span::new(start, self.pos) })
    }

    /// Entry point for anything starting with a digit: either a plain
    /// decimal integer, a based integer with a size prefix (`8'hFF`), or a
    /// real number (`14.72`, `39e8`, `24.7K`).
    fn lex_number(&mut self, start: usize) -> Result<Lexed, ParseError> {
        let digits_start = self.pos;
        self.consume_decimal_digits();

        // `[size] ' base digits` — based integer with an explicit size.
        if self.peek() == Some(b'\'') {
            let size_text = self.src[digits_start..self.pos].replace('_', "");
            let size: u32 = size_text.parse().map_err(|_| {
                ParseError::new(
                    ParseErrorKind::InvalidNumberLiteral { text: size_text.clone() },
                    Span::new(start, self.pos),
                )
            })?;
            return self.lex_based_digits(start, Some(size));
        }

        // Real number: decimal point form or scale-factor/exponent form.
        if self.peek() == Some(b'.') {
            if self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
                return self.lex_real_after_int_part(start);
            }
            if self.peek_at(1) == Some(b'_') {
                // LRMC-C1-04 (§2.6.2): "...except as the first character
                // of the constant or the first character after the
                // decimal point" — `1._5` is illegal, not a silent
                // reinterpretation as `1`, `.`, `_5`.
                let underscore_pos = self.pos + 1;
                return Err(ParseError::new(
                    ParseErrorKind::IllegalUnderscorePlacement(
                        UnderscorePosition::AfterDecimalPoint,
                    ),
                    Span::new(underscore_pos, underscore_pos + 1),
                ));
            }
        }
        // Integer digits directly followed by an exponent (`39e8`) or a
        // Table 2-1 scale factor (`7k`) — a real number with no decimal
        // point (Syntax 2-2's second and third `real_number` alternatives).
        if let Some((_, consumed_to)) = self.try_exponent_or_scale(self.pos) {
            self.pos = consumed_to;
            let real_text = self.src[start..self.pos].to_string();
            let value = parse_real_literal(&real_text).ok_or_else(|| {
                ParseError::new(
                    ParseErrorKind::InvalidNumberLiteral { text: real_text.clone() },
                    Span::new(start, self.pos),
                )
            })?;
            return Ok(Lexed {
                tok: Tok::Number(NumberLit::Real(value)),
                span: Span::new(start, self.pos),
            });
        }

        // Plain unsized decimal integer.
        let text = self.src[digits_start..self.pos].replace('_', "");
        let value: i64 = text.parse().map_err(|_| {
            ParseError::new(
                ParseErrorKind::InvalidNumberLiteral { text: text.clone() },
                Span::new(start, self.pos),
            )
        })?;
        Ok(Lexed { tok: Tok::Number(NumberLit::Integer(value)), span: Span::new(start, self.pos) })
    }

    /// `'base digits` with no leading size — legal per Syntax 2-2 (`size`
    /// is optional).
    fn lex_based_number_no_size(&mut self, start: usize) -> Result<Lexed, ParseError> {
        self.lex_based_digits(start, None)
    }

    /// Consumes `'[s]base digits` starting at the current `'`.
    fn lex_based_digits(
        &mut self,
        start: usize,
        size: Option<u32>,
    ) -> Result<Lexed, ParseError> {
        self.pos += 1; // consume `'`
        let mut signed = false;
        if matches!(self.peek(), Some(b's') | Some(b'S')) {
            signed = true;
            self.pos += 1;
        }
        let base = match self.peek() {
            Some(b'd') | Some(b'D') => NumberBase::Decimal,
            Some(b'b') | Some(b'B') => NumberBase::Binary,
            Some(b'o') | Some(b'O') => NumberBase::Octal,
            Some(b'h') | Some(b'H') => NumberBase::Hex,
            Some(c) => {
                return Err(ParseError::new(
                    ParseErrorKind::InvalidNumberLiteral {
                        text: format!("'{}{c}", if signed { "s" } else { "" }),
                    },
                    Span::new(start, self.pos + 1),
                ));
            }
            None => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedEof { expected: "a base letter (d/b/o/h)".into() },
                    Span::new(start, self.pos),
                ));
            }
        };
        self.pos += 1; // consume base letter
        // LRMC-C1-03 (§2.6.1 Example 2, `` 'D 3 ``): "The unsigned number
        // token shall immediately follow the base format, optionally
        // preceded by white space." Only inline whitespace — a newline
        // does not count (the digit run still can't span a line break),
        // so the following "at least one digit" check still fires if the
        // base letter is immediately followed by a newline.
        self.skip_inline_whitespace();
        let digits_start = self.pos;
        if self.peek() == Some(b'_') {
            // LRMC-C1-04 (§2.6.1): "The underscore character shall be
            // legal anywhere in a number except as the first character"
            // — `` 'h_FF `` is illegal, not silently `'hFF` with the
            // separator dropped.
            return Err(ParseError::new(
                ParseErrorKind::IllegalUnderscorePlacement(UnderscorePosition::FirstBasedDigit),
                Span::new(self.pos, self.pos + 1),
            ));
        }
        while let Some(b) = self.peek() {
            if is_based_digit(base, b) {
                self.pos += 1;
            } else if b.is_ascii_alphanumeric() {
                // LRMC-C1-01: a character that could plausibly continue a
                // digit run (alphanumeric) but is not one of the base's
                // legal digits (per §2.6.1's `*_digit` productions) — fail
                // loud naming the base and the offending digit, rather
                // than silently ending the literal early.
                return Err(ParseError::new(
                    ParseErrorKind::IllegalBasedDigit {
                        base: base_letter(base),
                        digit: b as char,
                    },
                    Span::new(self.pos, self.pos + 1),
                ));
            } else {
                break;
            }
        }
        if self.pos == digits_start {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedEof { expected: "at least one digit after the base".into() },
                Span::new(start, self.pos),
            ));
        }
        let digits = self.src[digits_start..self.pos].to_string();
        Ok(Lexed {
            tok: Tok::Number(NumberLit::Based(BasedNumber { size, signed, base, digits })),
            span: Span::new(start, self.pos),
        })
    }

    /// After an integer part and a confirmed `.digit`, consume the
    /// fractional part and an optional exponent/scale-factor suffix.
    fn lex_real_after_int_part(&mut self, start: usize) -> Result<Lexed, ParseError> {
        self.pos += 1; // consume `.`
        self.consume_decimal_digits();
        let suffix = self.try_exponent_or_scale(self.pos);
        let end = match &suffix {
            Some((_, consumed_to)) => *consumed_to,
            None => self.pos,
        };
        self.pos = end;
        let text = self.src[start..end].to_string();
        let value = parse_real_literal(&text).ok_or_else(|| {
            ParseError::new(
                ParseErrorKind::InvalidNumberLiteral { text: text.clone() },
                Span::new(start, end),
            )
        })?;
        Ok(Lexed { tok: Tok::Number(NumberLit::Real(value)), span: Span::new(start, end) })
    }

    fn consume_decimal_digits(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// If the byte at `at` starts an exponent (`e`/`E` optionally followed
    /// by a sign and digits) or a Table 2-1 scale-factor letter, returns
    /// the suffix text and the position just past it. Otherwise `None`.
    fn try_exponent_or_scale(&self, at: usize) -> Option<(String, usize)> {
        let b = *self.bytes.get(at)?;
        if b == b'e' || b == b'E' {
            let mut p = at + 1;
            if matches!(self.bytes.get(p), Some(b'+') | Some(b'-')) {
                p += 1;
            }
            let digits_start = p;
            while matches!(self.bytes.get(p), Some(c) if c.is_ascii_digit() || *c == b'_') {
                p += 1;
            }
            if p == digits_start {
                return None;
            }
            return Some((self.src[at..p].to_string(), p));
        }
        if scale_factor_value(b).is_some() {
            // Scale factor must not be followed by another identifier
            // character, or this is actually the start of an identifier
            // (e.g. `1m3` is not a valid real number token boundary case
            // this lexer needs to worry about beyond "the next char isn't
            // part of the same token").
            return Some((self.src[at..at + 1].to_string(), at + 1));
        }
        None
    }

    fn lex_punct(&mut self, start: usize) -> Result<Lexed, ParseError> {
        for candidate in PUNCT_TABLE {
            let len = candidate.len();
            if self.src.as_bytes()[self.pos..].starts_with(candidate.as_bytes()) {
                self.pos += len;
                return Ok(Lexed { tok: Tok::Punct(candidate), span: Span::new(start, self.pos) });
            }
        }
        let found = self.src[start..].chars().next().unwrap();
        Err(ParseError::new(
            ParseErrorKind::UnexpectedCharacter { found },
            Span::new(start, start + found.len_utf8()),
        ))
    }
}

fn scale_factor_value(b: u8) -> Option<f64> {
    Some(match b {
        b'T' => 1e12,
        b'G' => 1e9,
        b'M' => 1e6,
        b'K' | b'k' => 1e3,
        b'm' => 1e-3,
        b'u' => 1e-6,
        b'n' => 1e-9,
        b'p' => 1e-12,
        b'f' => 1e-15,
        b'a' => 1e-18,
        _ => return None,
    })
}

/// Parses a full real-number token's source text (LRM §2.6.2): a decimal
/// point form with an optional exponent/scale suffix, an underscore-tolerant
/// mantissa, and the exponent digits.
fn parse_real_literal(text: &str) -> Option<f64> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();

    // Scale-factor suffix: a single trailing letter from Table 2-1.
    if let Some(last) = cleaned.chars().last()
        && let Some(scale) = scale_factor_value(last as u8)
    {
        let mantissa = &cleaned[..cleaned.len() - 1];
        let base: f64 = mantissa.parse().ok()?;
        return Some(base * scale);
    }
    cleaned.parse().ok()
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// LRM §2.6.1's per-base digit legality (`binary_digit`/`octal_digit`/
/// `decimal_digit`/`hex_digit` productions), plus the always-legal
/// four-state digits (`x_digit`/`z_digit`) and the `_` separator, legal in
/// every base per those same productions' `{ '_' | ... }` repetition.
fn is_based_digit(base: NumberBase, b: u8) -> bool {
    if b == b'_' || matches!(b, b'x' | b'X' | b'z' | b'Z' | b'?') {
        return true;
    }
    match base {
        NumberBase::Binary => matches!(b, b'0' | b'1'),
        NumberBase::Octal => (b'0'..=b'7').contains(&b),
        NumberBase::Decimal => b.is_ascii_digit(),
        NumberBase::Hex => b.is_ascii_hexdigit(),
    }
}

/// The base letter as written in an error message (lowercased), for
/// `ParseErrorKind::IllegalBasedDigit`.
fn base_letter(base: NumberBase) -> char {
    match base {
        NumberBase::Decimal => 'd',
        NumberBase::Binary => 'b',
        NumberBase::Octal => 'o',
        NumberBase::Hex => 'h',
    }
}
