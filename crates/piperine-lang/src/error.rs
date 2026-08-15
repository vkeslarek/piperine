//! Typed parse errors (E-STD, `.specs/features/verilog-ams-support/tasks.md`
//! Standing Constraint C8). One variant per failure mode, every variant
//! carrying structured fields and a byte [`Span`] — no free-string
//! catch-all, no `From<String>`/`From<&str>` escape hatch. This crate has
//! an empty `[dependencies]` table (purity contract, `tests/purity.rs`), so
//! `Display`/`std::error::Error` are implemented by hand instead of via
//! `thiserror`.
//!
//! Descriptiveness lives here, in the data; rendering (miette diagnostics,
//! `#[help]`/`#[label]` text, numbered codes) happens at the
//! `piperine-lang` boundary, which owns `From<ParseError> for ElabError`.

use std::fmt;

/// The two lexically illegal underscore positions LRM §2.6.1 ("legal
/// anywhere in a number except as the first character") and §2.6.2
/// ("...except as the first character of the constant or the first
/// character after the decimal point") carve out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderscorePosition {
    /// Immediately after a real number's decimal point (§2.6.2), e.g.
    /// `1._5`.
    AfterDecimalPoint,
    /// The first character of a based literal's digit run, immediately
    /// after the base letter — e.g. `` 'h_FF `` (§2.6.1: legal anywhere
    /// in a number "except as the first character").
    FirstBasedDigit,
}

/// A byte-offset range into the (preprocessed) source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A single parse-time failure. Every variant is a distinct cause with its
/// own structured fields — never a pre-formatted sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
}

impl ParseError {
    pub fn new(kind: ParseErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The distinct lexical/parse failure modes this crate can report.
///
/// Each variant carries the data needed to render a descriptive message —
/// `expected`/`found` structured fields, not a pre-baked sentence — so the
/// `piperine-lang` boundary can attach a specific `#[help]` and, where the
/// variant enforces an LRM rule, cite the clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// The lexer hit a byte that starts no valid token.
    UnexpectedCharacter { found: char },
    /// A string literal was not closed before end-of-line/end-of-file.
    UnterminatedString,
    /// A block comment (`/* ... */`) was not closed before end-of-file.
    UnterminatedComment,
    /// A backslash escape inside a string/identifier used an unknown code.
    InvalidEscape { found: char },
    /// A numeric literal's digits/suffix did not form a valid integer or
    /// real constant per LRM §2.6.
    InvalidNumberLiteral { text: String },
    /// A based-literal digit was illegal for its declared base (LRM
    /// §2.6.1's `binary_digit`/`octal_digit`/`decimal_digit`/`hex_digit`
    /// productions), e.g. `'o89` (8/9 are not octal digits) or `'d1F` (`F`
    /// is not a decimal digit). `base` is the base letter as written
    /// (`d`/`b`/`o`/`h`, lowercased); `digit` is the offending character.
    IllegalBasedDigit { base: char, digit: char },
    /// An underscore separator appeared in one of the two lexically
    /// illegal positions LRM §2.6.1/§2.6.2 carve out (elsewhere in a
    /// number, `_` is a legal, ignored separator).
    IllegalUnderscorePlacement(UnderscorePosition),
    /// The parser expected one of a set of token kinds and found something
    /// else. `expected` is a human-readable description of what was
    /// expected (e.g. `"';'"`, `"an expression"`), not a token enum name.
    UnexpectedToken { expected: String, found: String },
    /// The parser needed another token but the source ended.
    UnexpectedEof { expected: String },
    /// `` `include "path" `` named a file the resolver could not find.
    IncludeNotFound { path: String },
    /// `` `include `` formed a cycle: `path` is already being included by
    /// an enclosing `` `include ``.
    RecursiveInclude { path: String },
    /// An `` `ifdef ``/`` `ifndef `` (named by `directive`) was never
    /// closed by a matching `` `endif `` before the source ended.
    UnterminatedConditional { directive: String },
    /// An `` `else ``/`` `elsif ``/`` `endif `` (named by `directive`)
    /// appeared with no open `` `ifdef ``/`` `ifndef `` to match.
    UnmatchedConditionalDirective { directive: String },
    /// A `` `name `` was neither a known compiler directive nor a name
    /// previously introduced by `` `define ``.
    UndefinedMacro { name: String },
    /// A function-like macro invocation's argument count did not match its
    /// `` `define ``d parameter count.
    MacroArityMismatch { name: String, expected: usize, found: usize },
    /// A ch.10 compiler directive's argument did not match its LRM syntax
    /// (e.g. `` `default_transition `` without a parseable real-valued
    /// `transition_time`, LRM §10.3 Syntax 10-2).
    MalformedDirective { directive: String, reason: String },
    /// A `` `begin_keywords `` (§10.6, LRMC-C10-04) was never closed by a
    /// matching `` `end_keywords `` before the source ended.
    UnterminatedKeywords,
    /// An `` `end_keywords `` (§10.6) appeared with no open
    /// `` `begin_keywords `` to match.
    UnmatchedEndKeywords,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::UnexpectedCharacter { found } => {
                write!(f, "unexpected character `{found}`")
            }
            ParseErrorKind::UnterminatedString => write!(f, "unterminated string literal"),
            ParseErrorKind::UnterminatedComment => write!(f, "unterminated block comment"),
            ParseErrorKind::InvalidEscape { found } => {
                write!(f, "invalid escape sequence `\\{found}`")
            }
            ParseErrorKind::InvalidNumberLiteral { text } => {
                write!(f, "invalid number literal `{text}`")
            }
            ParseErrorKind::IllegalBasedDigit { base, digit } => {
                write!(f, "`{digit}` is not a legal digit for base `{base}`")
            }
            ParseErrorKind::IllegalUnderscorePlacement(pos) => {
                let where_ = match pos {
                    UnderscorePosition::AfterDecimalPoint => {
                        "immediately after a decimal point"
                    }
                    UnderscorePosition::FirstBasedDigit => {
                        "as the first digit of a based literal's digit run"
                    }
                };
                write!(f, "`_` is not legal {where_}")
            }
            ParseErrorKind::UnexpectedToken { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            ParseErrorKind::UnexpectedEof { expected } => {
                write!(f, "unexpected end of file, expected {expected}")
            }
            ParseErrorKind::IncludeNotFound { path } => {
                write!(f, "`include \"{path}\" could not be resolved")
            }
            ParseErrorKind::RecursiveInclude { path } => {
                write!(f, "`include \"{path}\" forms an include cycle")
            }
            ParseErrorKind::UnterminatedConditional { directive } => {
                write!(f, "`{directive} was never closed by a matching `endif")
            }
            ParseErrorKind::UnmatchedConditionalDirective { directive } => {
                write!(f, "`{directive} has no matching `ifdef/`ifndef")
            }
            ParseErrorKind::UndefinedMacro { name } => {
                write!(f, "`{name} is not a compiler directive or a defined macro")
            }
            ParseErrorKind::MacroArityMismatch { name, expected, found } => {
                write!(f, "`{name} expects {expected} argument(s), found {found}")
            }
            ParseErrorKind::MalformedDirective { directive, reason } => {
                write!(f, "`{directive} directive is malformed: {reason}")
            }
            ParseErrorKind::UnterminatedKeywords => {
                write!(f, "`begin_keywords was never closed by a matching `end_keywords")
            }
            ParseErrorKind::UnmatchedEndKeywords => {
                write!(f, "`end_keywords has no matching `begin_keywords")
            }
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at byte {}..{})", self.kind, self.span.start, self.span.end)
    }
}

impl std::error::Error for ParseError {}
