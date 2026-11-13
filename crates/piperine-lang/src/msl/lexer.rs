//! MSL lexer: source text to spanned tokens (`docs/spec/appendix_a_lexical.md`).
//!
//! A private `logos` automaton (`Raw`) recognizes the lexical shapes; plain
//! functions below classify them: keywords against [`KEYWORDS`], number
//! literals against Appendix A.2, doc comments against A.4. Contextual
//! keywords (`analog`, `digital`, `potential`, `flow`, `value`, `resolve`)
//! stay [`Token::Ident`]; the parser decides by position.
//!
//! `>` is always a single [`Punct::Gt`] token (so `Vec<Vec<T>>` needs no
//! splitting); the parser joins `>>`, `>=` and `>>=` from adjacent spans.

use logos::Logos;

use super::diag::Diagnostic;
use super::span::Span;

/// Reserved words, spelled as in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kw {
    // Rust 2024 strict keywords.
    As, Break, Const, Continue, Crate, Else, Enum, Extern, False, Fn, For, If, Impl,
    In, Let, Loop, Match, Mod, Move, Mut, Pub, Ref, Return, SelfLower, SelfUpper,
    Static, Struct, Super, Trait, True, Type, Unsafe, Use, Where, While, Async, Await,
    Dyn,
    // Rust 2024 reserved for future use.
    Abstract, Become, Box, Do, Final, Macro, Override, Priv, Typeof, Unsized, Virtual,
    Yield, Try, Gen,
    // Added by MSL (Appendix A.1).
    Module, Nature, Signal, On, Out, Flip, After,
}

/// Every reserved word and its spelling: the one place the two meet.
pub const KEYWORDS: &[(&str, Kw)] = &[
    ("as", Kw::As), ("break", Kw::Break), ("const", Kw::Const), ("continue", Kw::Continue),
    ("crate", Kw::Crate), ("else", Kw::Else), ("enum", Kw::Enum), ("extern", Kw::Extern),
    ("false", Kw::False), ("fn", Kw::Fn), ("for", Kw::For), ("if", Kw::If),
    ("impl", Kw::Impl), ("in", Kw::In), ("let", Kw::Let), ("loop", Kw::Loop),
    ("match", Kw::Match), ("mod", Kw::Mod), ("move", Kw::Move), ("mut", Kw::Mut),
    ("pub", Kw::Pub), ("ref", Kw::Ref), ("return", Kw::Return), ("self", Kw::SelfLower),
    ("Self", Kw::SelfUpper), ("static", Kw::Static), ("struct", Kw::Struct),
    ("super", Kw::Super), ("trait", Kw::Trait), ("true", Kw::True), ("type", Kw::Type),
    ("unsafe", Kw::Unsafe), ("use", Kw::Use), ("where", Kw::Where), ("while", Kw::While),
    ("async", Kw::Async), ("await", Kw::Await), ("dyn", Kw::Dyn),
    ("abstract", Kw::Abstract), ("become", Kw::Become), ("box", Kw::Box), ("do", Kw::Do),
    ("final", Kw::Final), ("macro", Kw::Macro), ("override", Kw::Override),
    ("priv", Kw::Priv), ("typeof", Kw::Typeof), ("unsized", Kw::Unsized),
    ("virtual", Kw::Virtual), ("yield", Kw::Yield), ("try", Kw::Try), ("gen", Kw::Gen),
    ("module", Kw::Module), ("nature", Kw::Nature), ("signal", Kw::Signal),
    ("on", Kw::On), ("out", Kw::Out), ("flip", Kw::Flip), ("after", Kw::After),
];

impl Kw {
    pub fn from_str(s: &str) -> Option<Kw> {
        KEYWORDS.iter().find(|(w, _)| *w == s).map(|(_, k)| *k)
    }

    pub fn as_str(self) -> &'static str {
        KEYWORDS.iter().find(|(_, k)| *k == self).map(|(w, _)| *w).unwrap_or("?")
    }
}

/// Punctuation and operators. `+>` ([`Punct::PlusGt`]) and `<-`
/// ([`Punct::LtMinus`]) are Appendix A.5's two operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Punct {
    Plus, Minus, Star, Slash, Percent, Caret, Not, And, Or, AndAnd, OrOr, Shl,
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq, CaretEq, AndEq, OrEq, ShlEq,
    Eq, EqEq, Ne, Gt, Lt, Le, At, Underscore, Dot, DotDot, DotDotDot, DotDotEq,
    Comma, Semi, Colon, ColonColon, Arrow, FatArrow, Pound, Dollar, Question, Tilde,
    LParen, RParen, LBracket, RBracket, LBrace, RBrace,
    PlusGt, LtMinus,
}

impl Punct {
    pub fn as_str(self) -> &'static str {
        use Punct::*;
        match self {
            Plus => "+", Minus => "-", Star => "*", Slash => "/", Percent => "%",
            Caret => "^", Not => "!", And => "&", Or => "|", AndAnd => "&&", OrOr => "||",
            Shl => "<<", PlusEq => "+=", MinusEq => "-=", StarEq => "*=", SlashEq => "/=",
            PercentEq => "%=", CaretEq => "^=", AndEq => "&=", OrEq => "|=", ShlEq => "<<=",
            Eq => "=", EqEq => "==", Ne => "!=", Gt => ">", Lt => "<", Le => "<=", At => "@",
            Underscore => "_", Dot => ".", DotDot => "..", DotDotDot => "...",
            DotDotEq => "..=", Comma => ",", Semi => ";", Colon => ":", ColonColon => "::",
            Arrow => "->", FatArrow => "=>", Pound => "#", Dollar => "$", Question => "?",
            Tilde => "~", LParen => "(", RParen => ")", LBracket => "[", RBracket => "]",
            LBrace => "{", RBrace => "}", PlusGt => "+>", LtMinus => "<-",
        }
    }
}

/// One lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// An identifier, contextual keywords included.
    Ident(String),
    Kw(Kw),
    /// The `Unknown` literal (A.3).
    Unknown,
    /// An integer literal; the text, type suffix included, is kept.
    Int(String),
    /// A float literal: the value (SI suffix applied) and the original text.
    Float { value: f64, raw: String },
    /// A string literal: the text between the quotes, escapes unprocessed.
    Str(String),
    /// The lifetime `'static`, the only one MSL accepts.
    StaticLifetime,
    /// `///` doc comment: the text after the marker, verbatim.
    DocOuter(String),
    /// `//!` doc comment: the text after the marker, verbatim.
    DocInner(String),
    Punct(Punct),
    /// Stands for a malformed token already reported by a diagnostic.
    Error,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Ident(s) => write!(f, "identifier `{s}`"),
            Token::Kw(k) => write!(f, "`{}`", k.as_str()),
            Token::Unknown => f.write_str("`Unknown`"),
            Token::Int(s) => write!(f, "`{s}`"),
            Token::Float { raw, .. } => write!(f, "`{raw}`"),
            Token::Str(_) => f.write_str("string literal"),
            Token::StaticLifetime => f.write_str("`'static`"),
            Token::DocOuter(_) | Token::DocInner(_) => f.write_str("doc comment"),
            Token::Punct(p) => write!(f, "`{}`", p.as_str()),
            Token::Error => f.write_str("invalid token"),
        }
    }
}

/// The lexical shapes `logos` recognizes; [`lex`] turns them into [`Token`]s.
#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\r\n\x0c]+")]
enum Raw {
    #[token("//", line_comment)]
    LineComment,
    #[token("/*", block_comment)]
    BlockComment,
    #[token("\"", string)]
    Str,
    #[token("b\"", string)]
    ByteStr,
    #[regex(r"b'(\\.[^'\n]*|[^'\\\n])'")]
    ByteChar,
    #[regex(r"'(\\.[^'\n]*|[^'\\\n])'")]
    Char,
    #[regex(r"'[A-Za-z_][A-Za-z0-9_]*")]
    Lifetime,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    #[regex(r"[0-9][0-9_]*(\.[0-9][0-9_]*)?([eE][+-]?[0-9][0-9_]*)?[A-Za-z_][A-Za-z0-9_]*")]
    #[regex(r"[0-9][0-9_]*(\.[0-9][0-9_]*)?([eE][+-]?[0-9][0-9_]*)?")]
    Number,

    #[token("+", |_| Punct::Plus)] #[token("-", |_| Punct::Minus)]
    #[token("*", |_| Punct::Star)] #[token("/", |_| Punct::Slash)]
    #[token("%", |_| Punct::Percent)] #[token("^", |_| Punct::Caret)]
    #[token("!", |_| Punct::Not)] #[token("&", |_| Punct::And)]
    #[token("|", |_| Punct::Or)] #[token("&&", |_| Punct::AndAnd)]
    #[token("||", |_| Punct::OrOr)] #[token("<<", |_| Punct::Shl)]
    #[token("+=", |_| Punct::PlusEq)] #[token("-=", |_| Punct::MinusEq)]
    #[token("*=", |_| Punct::StarEq)] #[token("/=", |_| Punct::SlashEq)]
    #[token("%=", |_| Punct::PercentEq)] #[token("^=", |_| Punct::CaretEq)]
    #[token("&=", |_| Punct::AndEq)] #[token("|=", |_| Punct::OrEq)]
    #[token("<<=", |_| Punct::ShlEq)] #[token("=", |_| Punct::Eq)]
    #[token("==", |_| Punct::EqEq)] #[token("!=", |_| Punct::Ne)]
    #[token(">", |_| Punct::Gt)] #[token("<", |_| Punct::Lt)]
    #[token("<=", |_| Punct::Le)] #[token("@", |_| Punct::At)]
    #[token(".", |_| Punct::Dot)] #[token("..", |_| Punct::DotDot)]
    #[token("...", |_| Punct::DotDotDot)] #[token("..=", |_| Punct::DotDotEq)]
    #[token(",", |_| Punct::Comma)] #[token(";", |_| Punct::Semi)]
    #[token(":", |_| Punct::Colon)] #[token("::", |_| Punct::ColonColon)]
    #[token("->", |_| Punct::Arrow)] #[token("=>", |_| Punct::FatArrow)]
    #[token("#", |_| Punct::Pound)] #[token("$", |_| Punct::Dollar)]
    #[token("?", |_| Punct::Question)] #[token("~", |_| Punct::Tilde)]
    #[token("(", |_| Punct::LParen)] #[token(")", |_| Punct::RParen)]
    #[token("[", |_| Punct::LBracket)] #[token("]", |_| Punct::RBracket)]
    #[token("{", |_| Punct::LBrace)] #[token("}", |_| Punct::RBrace)]
    #[token("+>", |_| Punct::PlusGt)] #[token("<-", |_| Punct::LtMinus)]
    Punct(Punct),
}

/// Consume the rest of a line comment, up to but excluding the newline.
fn line_comment(lex: &mut logos::Lexer<Raw>) {
    let n = lex.remainder().find('\n').unwrap_or(lex.remainder().len());
    lex.bump(n);
}

/// Consume a (nestable) block comment after its `/*`; `false` when the file
/// ends first, with the lexer left at end of file.
fn block_comment(lex: &mut logos::Lexer<Raw>) -> bool {
    let rest = lex.remainder();
    let bytes = rest.as_bytes();
    let (mut depth, mut i) = (1usize, 0usize);
    while i < bytes.len() {
        if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                lex.bump(i);
                return true;
            }
        } else if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    lex.bump(bytes.len());
    false
}

/// Consume a string body after its opening quote; `false` when the file ends
/// first, with the lexer left at end of file.
fn string(lex: &mut logos::Lexer<Raw>) -> bool {
    let bytes = lex.remainder().as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => {
                lex.bump(i + 1);
                return true;
            }
            _ => i += 1,
        }
    }
    lex.bump(bytes.len());
    false
}

/// Lex `src`. Malformed tokens yield a diagnostic and, where a value was
/// expected, a [`Token::Error`] placeholder; lexing always runs to the end.
pub fn lex(src: &str) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diags = Vec::new();
    let mut lexer = Raw::lexer(src);
    while let Some(result) = lexer.next() {
        let range = lexer.span();
        let span = Span::new(range.start, range.end);
        let text = lexer.slice();
        match result {
            Ok(Raw::Punct(p)) => tokens.push((Token::Punct(p), span)),
            Ok(Raw::Ident) => tokens.push((classify_ident(text), span)),
            Ok(Raw::Number) => match classify_number(text, span) {
                Ok(tok) => tokens.push((tok, span)),
                Err(d) => {
                    diags.push(d);
                    tokens.push((Token::Error, span));
                }
            },
            Ok(Raw::Str) => tokens.push((Token::Str(text[1..text.len() - 1].to_string()), span)),
            Ok(Raw::LineComment) => {
                let line = text.strip_suffix('\r').unwrap_or(text);
                if line.starts_with("///") && !line.starts_with("////") {
                    tokens.push((Token::DocOuter(line[3..].to_string()), span));
                } else if line.starts_with("//!") {
                    tokens.push((Token::DocInner(line[3..].to_string()), span));
                }
            }
            Ok(Raw::BlockComment) => {}
            Ok(Raw::Lifetime) if text == "'static" => tokens.push((Token::StaticLifetime, span)),
            Ok(Raw::ByteStr | Raw::ByteChar | Raw::Char | Raw::Lifetime) => {
                diags.push(dropped_class(text, span));
                tokens.push((Token::Error, span));
            }
            Err(()) => {
                let (d, token) = lex_failure(src, span);
                diags.push(d);
                if let Some(t) = token {
                    tokens.push((t, span));
                }
            }
        }
    }
    (tokens, diags)
}

/// Lex raw bytes: invalid UTF-8 is one diagnostic and no tokens.
pub fn lex_bytes(bytes: &[u8]) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
    match std::str::from_utf8(bytes) {
        Ok(src) => lex(src),
        Err(e) => {
            let at = e.valid_up_to();
            let end = e.error_len().map_or(bytes.len(), |n| at + n);
            let d = Diagnostic::new(
                "E0900",
                "source is not valid UTF-8",
                Span::new(at, end),
                "invalid byte sequence",
            )
            .with_help("MSL source text is UTF-8");
            (Vec::new(), vec![d])
        }
    }
}

fn classify_ident(text: &str) -> Token {
    if text == "_" {
        Token::Punct(Punct::Underscore)
    } else if text == "Unknown" {
        Token::Unknown
    } else if let Some(k) = Kw::from_str(text) {
        Token::Kw(k)
    } else {
        Token::Ident(text.to_string())
    }
}

fn dropped_class(text: &str, span: Span) -> Diagnostic {
    let what = if text.starts_with("b\"") {
        "byte string literal"
    } else if text.starts_with("b'") {
        "byte literal"
    } else if text.starts_with('\'') && text.ends_with('\'') && text.len() > 1 {
        "character literal"
    } else {
        "lifetime other than `'static`"
    };
    Diagnostic::new("E0905", format!("{what} is not part of the language"), span, "dropped token class")
        .with_help("`char` is not a value type; the only accepted lifetime is `'static`, in `&'static str`")
}

/// A token `logos` could not match, or a callback rejected: an unterminated
/// comment or string, a stray quote, or an unexpected character.
fn lex_failure(src: &str, span: Span) -> (Diagnostic, Option<Token>) {
    let rest = &src[span.start..];
    if rest.starts_with("/*") {
        let d = Diagnostic::new(
            "E0900",
            "unterminated block comment",
            Span::new(span.start, span.start + 2),
            "this comment is never closed",
        )
        .with_help("close it with `*/`");
        (d, None)
    } else if rest.starts_with('"') || rest.starts_with("b\"") {
        let open = if rest.starts_with('"') { 1 } else { 2 };
        let d = Diagnostic::new(
            "E0900",
            "unterminated string literal",
            Span::new(span.start, span.start + open),
            "this string is never closed",
        )
        .with_help("close it with `\"`");
        (d, Some(Token::Error))
    } else if rest.starts_with('\'') || rest.starts_with("b'") {
        (dropped_class("'", span), Some(Token::Error))
    } else {
        let ch = rest.chars().next().unwrap_or('\0');
        let d = Diagnostic::new(
            "E0900",
            format!("unexpected character `{ch}`"),
            Span::new(span.start, span.start + ch.len_utf8()),
            "not a token of the language",
        );
        (d, Some(Token::Error))
    }
}

/// SI suffixes of Appendix A.2 and their decimal exponents.
const SI_SUFFIXES: &[(char, i32)] = &[
    ('T', 12), ('G', 9), ('M', 6), ('k', 3), ('m', -3),
    ('u', -6), ('n', -9), ('p', -12), ('f', -15), ('a', -18),
];

const INT_SUFFIXES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// Classify a number lexeme per Appendix A.2: plain integer or float, a
/// Rust-suffixed literal, or an SI literal; anything else is `E0902` (a
/// malformed SI literal) or `E0900` (an unknown suffix).
fn classify_number(text: &str, span: Span) -> Result<Token, Diagnostic> {
    let malformed_si = |why: &str| {
        Diagnostic::new("E0902", format!("malformed SI literal `{text}`: {why}"), span, "malformed SI literal")
            .with_help("an SI suffix goes on a decimal literal without exponent or type suffix, e.g. `2.2u`")
    };

    // Non-decimal bases: digits, then whatever is left is a suffix.
    let radix_digits: Option<fn(char) -> bool> = match text.get(..2) {
        Some("0x" | "0X") => Some(|c| c.is_ascii_hexdigit() || c == '_'),
        Some("0o" | "0O") => Some(|c| ('0'..='7').contains(&c) || c == '_'),
        Some("0b" | "0B") => Some(|c| c == '0' || c == '1' || c == '_'),
        _ => None,
    };
    if let Some(is_digit) = radix_digits {
        let body = &text[2..];
        let digits_end = body.find(|c: char| !is_digit(c)).unwrap_or(body.len());
        let suffix = &body[digits_end..];
        return if digits_end == 0 {
            Err(unknown_suffix(text, span))
        } else if suffix.is_empty() || INT_SUFFIXES.contains(&suffix) {
            Ok(Token::Int(text.to_string()))
        } else if suffix.starts_with(si_letter) {
            Err(malformed_si("a non-decimal literal takes no SI suffix"))
        } else {
            Err(unknown_suffix(text, span))
        };
    }

    // Decimal: DEC_DIGITS ('.' DEC_DIGITS)? ([eE][+-]?DEC_DIGITS)? suffix.
    let b = text.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        while *i < b.len() && (b[*i].is_ascii_digit() || b[*i] == b'_') {
            *i += 1;
        }
    };
    digits(&mut i);
    let mut has_frac = false;
    if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
        i += 1;
        digits(&mut i);
        has_frac = true;
    }
    let mut has_exp = false;
    if i < b.len() && matches!(b[i], b'e' | b'E') {
        let mut j = i + 1;
        if j < b.len() && matches!(b[j], b'+' | b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            i = j;
            digits(&mut i);
            has_exp = true;
        }
    }
    let number = &text[..i];
    let suffix = &text[i..];

    if suffix.is_empty() {
        return Ok(if has_frac || has_exp {
            Token::Float { value: parse_f64(number, 0), raw: text.to_string() }
        } else {
            Token::Int(text.to_string())
        });
    }
    if suffix == "f32" || suffix == "f64" {
        return Ok(Token::Float { value: parse_f64(number, 0), raw: text.to_string() });
    }
    if INT_SUFFIXES.contains(&suffix) {
        return if has_frac || has_exp {
            Err(unknown_suffix(text, span))
        } else {
            Ok(Token::Int(text.to_string()))
        };
    }
    let mut chars = suffix.chars();
    match chars.next().and_then(|c| SI_SUFFIXES.iter().find(|(s, _)| *s == c)) {
        Some((_, exp)) if suffix.chars().count() == 1 => {
            if has_exp {
                Err(malformed_si("an SI literal has no exponent"))
            } else {
                Ok(Token::Float { value: parse_f64(number, *exp), raw: text.to_string() })
            }
        }
        Some(_) if has_exp => Err(malformed_si("an SI literal has no exponent")),
        Some(_) => Err(malformed_si("an SI literal has no type suffix")),
        None => Err(unknown_suffix(text, span)),
    }
}

fn si_letter(c: char) -> bool {
    SI_SUFFIXES.iter().any(|(s, _)| *s == c)
}

fn unknown_suffix(text: &str, span: Span) -> Diagnostic {
    Diagnostic::new("E0900", format!("invalid literal `{text}`"), span, "unknown literal suffix")
}

/// The nearest `f64` to `number` scaled by `10^exp10`: the exact decimal
/// product, parsed once, never `number * 1e-6` (A.2 rule 6).
fn parse_f64(number: &str, exp10: i32) -> f64 {
    let clean: String = number.chars().filter(|c| *c != '_').collect();
    let text = if exp10 == 0 { clean } else { format!("{clean}e{exp10}") };
    text.parse().unwrap_or(f64::NAN)
}
