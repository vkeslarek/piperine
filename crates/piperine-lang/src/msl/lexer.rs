//! MSL lexer: source text to spanned tokens (`docs/spec/appendix_a_lexical.md`).
//!
//! A private `logos` automaton (`Raw`) recognizes the lexical shapes; [`Lexer`]
//! and [`NumberLiteral`] classify them: keywords against [`KEYWORDS`], number
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
    /// The keyword spelled `s`, if it is one.
    pub fn from_str(s: &str) -> Option<Kw> {
        KEYWORDS.iter().find(|(w, _)| *w == s).map(|(_, k)| *k)
    }

    /// The keyword's spelling (`?` for one missing from [`KEYWORDS`]).
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
    /// The punctuation's spelling.
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
    /// `r#word` for one of the seven added keywords: the word, no prefix.
    RawIdent(String),
    Kw(Kw),
    /// An integer literal; the text, type suffix included, is kept.
    Int(String),
    /// A float literal: the value (SI suffix applied) and the original text.
    Float { value: f64, raw: String },
    /// A string literal: the text between the quotes, escapes unprocessed. A
    /// raw string (`r#"..."#`) holds its body with backslash and quote escaped,
    /// so the text reads back as the same value, and the number of its `#`.
    Str { text: String, raw_hashes: Option<u8> },
    /// `///` doc comment: the text after the marker, verbatim.
    DocOuter(String),
    /// `//!` doc comment: the text after the marker, verbatim.
    DocInner(String),
    /// `/** .. */` doc comment: the text between the markers, verbatim.
    DocOuterBlock(String),
    /// `/*! .. */` doc comment: the text between the markers, verbatim.
    DocInnerBlock(String),
    Punct(Punct),
    /// Stands for a malformed token already reported by a diagnostic.
    Error,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Ident(s) => write!(f, "identifier `{s}`"),
            Token::RawIdent(s) => write!(f, "identifier `r#{s}`"),
            Token::Kw(k) => write!(f, "`{}`", k.as_str()),
            Token::Int(s) => write!(f, "`{s}`"),
            Token::Float { raw, .. } => write!(f, "`{raw}`"),
            Token::Str { .. } => f.write_str("string literal"),
            Token::DocOuter(_) | Token::DocInner(_) | Token::DocOuterBlock(_) | Token::DocInnerBlock(_) => {
                f.write_str("doc comment")
            },
            Token::Punct(p) => write!(f, "`{}`", p.as_str()),
            Token::Error => f.write_str("invalid token"),
        }
    }
}

/// The lexical shapes `logos` recognizes; [`Lexer::lex`] turns them into [`Token`]s.
#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\r\n\x0c]+")]
enum Raw {
    #[token("//", Raw::line_comment)]
    LineComment,
    #[token("/*", Raw::block_comment)]
    BlockComment,
    #[token("\"", Raw::string)]
    Str,
    #[token("b\"", Raw::string)]
    ByteStr,
    #[regex(r"b'(\\.[^'\n]*|[^'\\\n])'")]
    ByteChar,
    #[regex(r"'(\\.[^'\n]*|[^'\\\n])'")]
    Char,
    #[regex(r"'[A-Za-z_][A-Za-z0-9_]*")]
    Lifetime,
    // Rust's identifiers: `XID_Start` or `_`, then `XID_Continue`.
    #[regex(r"[\p{XID_Start}_][\p{XID_Continue}]*")]
    Ident,
    // `r#word`: a raw identifier (only the seven added keywords are accepted).
    #[regex(r"r#[\p{XID_Start}_][\p{XID_Continue}]*")]
    RawIdent,
    // `r"..."`, `r#"..."#`: the callback finds the closing quote and hashes.
    #[regex(r##"r#*""##, Raw::raw_string)]
    RawStr,
    // `br"..."`, `br#"..."#`: a byte string, a dropped class.
    #[regex(r##"br#*""##, Raw::raw_string)]
    RawByteStr,
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
impl Raw {
    /// Consume the rest of a line comment, up to but excluding the newline.
    fn line_comment(lexer: &mut logos::Lexer<Raw>) {
        let length = lexer.remainder().find('\n').unwrap_or(lexer.remainder().len());
        lexer.bump(length);
    }

    /// Consume a (nestable) block comment after its `/*`; `false` when the file
    /// ends first, with the lexer left at end of file.
    fn block_comment(lexer: &mut logos::Lexer<Raw>) -> bool {
        let bytes = lexer.remainder().as_bytes();
        let (mut depth, mut index) = (1usize, 0usize);
        while index < bytes.len() {
            if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    lexer.bump(index);
                    return true;
                }
            } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
                depth += 1;
                index += 2;
            } else {
                index += 1;
            }
        }
        lexer.bump(bytes.len());
        false
    }

    /// Consume a string body after its opening quote; `false` when the file ends
    /// first, with the lexer left at end of file.
    fn string(lexer: &mut logos::Lexer<Raw>) -> bool {
        let bytes = lexer.remainder().as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            match bytes[index] {
                b'\\' => index += 2,
                b'"' => {
                    lexer.bump(index + 1);
                    return true;
                }
                _ => index += 1,
            }
        }
        lexer.bump(bytes.len());
        false
    }

    /// Consume a raw string body after its `r#*"` opener; `false` when the file
    /// ends before the closing quote and the same number of hashes.
    fn raw_string(lexer: &mut logos::Lexer<Raw>) -> bool {
        let hashes = lexer.slice().bytes().filter(|byte| *byte == b'#').count();
        let closer: String = std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
        match lexer.remainder().find(&closer) {
            Some(at) => {
                lexer.bump(at + closer.len());
                true
            }
            None => {
                lexer.bump(lexer.remainder().len());
                false
            }
        }
    }
}

/// The lexer: source text in, spanned tokens, diagnostics and the plain
/// comments out. It owns what one run collects and the classification of each
/// lexical shape the `logos` automaton ([`Raw`]) recognizes.
pub struct Lexer<'source> {
    /// The whole source text, byte order mark included.
    source: &'source str,
    /// The tokens lexed so far.
    tokens: Vec<(Token, Span)>,
    /// The diagnostics reported so far.
    diagnostics: Vec<Diagnostic>,
    /// The span of every plain comment so far.
    comments: Vec<Span>,
}

impl<'source> Lexer<'source> {
    /// Lex `source`. Malformed tokens yield a diagnostic and, where a value was
    /// expected, a [`Token::Error`] placeholder; lexing always runs to the end.
    pub fn lex(source: &str) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
        let (tokens, diagnostics, _) = Lexer::lex_with_comments(source);
        (tokens, diagnostics)
    }

    /// [`Lexer::lex`], plus the span of every plain comment (a `//` or `/* */` that is
    /// no doc comment), in source order: the tokens and diagnostics are `lex`'s.
    pub(crate) fn lex_with_comments(source: &str) -> (Vec<(Token, Span)>, Vec<Diagnostic>, Vec<Span>) {
        let mut lexer = Lexer { source, tokens: Vec::new(), diagnostics: Vec::new(), comments: Vec::new() };
        lexer.run();
        (lexer.tokens, lexer.diagnostics, lexer.comments)
    }

    /// Lex raw bytes: invalid UTF-8 is one diagnostic and no tokens.
    pub fn lex_bytes(bytes: &[u8]) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
        match std::str::from_utf8(bytes) {
            Ok(source) => Lexer::lex(source),
            Err(error) => {
                let at = error.valid_up_to();
                let end = error.error_len().map_or(bytes.len(), |length| at + length);
                let diagnostic = Diagnostic::new(
                    "E0900",
                    "source is not valid UTF-8",
                    Span::new(at, end),
                    "invalid byte sequence",
                )
                .with_help("MSL source text is UTF-8");
                (Vec::new(), vec![diagnostic])
            }
        }
    }

    /// Run the automaton over the whole source, one lexical shape at a time.
    fn run(&mut self) {
        // A byte order mark opens the file and is not a token.
        let offset = if self.source.starts_with('\u{FEFF}') { '\u{FEFF}'.len_utf8() } else { 0 };
        let mut automaton = Raw::lexer(&self.source[offset..]);
        while let Some(result) = automaton.next() {
            let range = automaton.span();
            let span = Span::new(range.start + offset, range.end + offset);
            let text = automaton.slice();
            match result {
                Ok(Raw::Punct(Punct::Dot)) if self.closes_float(span) => self.float_closed_by_dot(span),
                Ok(Raw::Punct(punct)) => self.tokens.push((Token::Punct(punct), span)),
                Ok(Raw::Ident) => self.tokens.push((Lexer::classify_identifier(text), span)),
                Ok(Raw::RawIdent) => self.raw_identifier(&text[2..], span),
                Ok(Raw::Number) => self.number(text, span),
                Ok(Raw::RawStr) => self.raw_string(text, span),
                Ok(Raw::RawByteStr) => self.dropped("b\"", span),
                Ok(Raw::Str) => self.string(text, span),
                Ok(Raw::LineComment) => self.line_comment(text, span),
                Ok(Raw::BlockComment) => self.block_comment(text, span),
                Ok(Raw::Lifetime) => {
                    let label_colon = self.lifetime(text, span, automaton.remainder());
                    automaton.bump(label_colon);
                }
                Ok(Raw::ByteStr | Raw::ByteChar | Raw::Char) => self.dropped(text, span),
                Err(()) => self.failure(span),
            }
        }
    }

    /// Whether the `.` at `span` closes the float literal `1.`: it touches a
    /// plain decimal integer (not a tuple index, `x.0.`) and is followed by no
    /// `.`, `_` or identifier start (`1..2`, `1.max(2)` keep their dot).
    fn closes_float(&self, span: Span) -> bool {
        let Some((Token::Int(digits), integer_span)) = self.tokens.last() else { return false };
        let index = self.tokens.len() >= 2
            && matches!(&self.tokens[self.tokens.len() - 2], (Token::Punct(Punct::Dot), s) if s.end == integer_span.start);
        integer_span.end == span.start
            && !index
            && digits.bytes().all(|byte| byte.is_ascii_digit() || byte == b'_')
            && !self.source[span.end..].starts_with(|c: char| c == '.' || c == '_' || c.is_alphabetic())
    }

    /// `1.`: the integer just lexed and the dot at `span` are one float literal, as in Rust.
    fn float_closed_by_dot(&mut self, span: Span) {
        let Some((Token::Int(digits), integer_span)) = self.tokens.pop() else { return };
        let token = Token::Float { value: NumberLiteral::parse_float(&digits, 0), raw: format!("{digits}.") };
        let whole = Span::new(integer_span.start, span.end);
        self.diagnostics.extend(NumberLiteral::range_diagnostic(&token, whole));
        self.tokens.push((token, whole));
    }

    /// A keyword, the lone `_`, or an identifier.
    fn classify_identifier(text: &str) -> Token {
        if text == "_" {
            Token::Punct(Punct::Underscore)
        } else if let Some(keyword) = Kw::from_str(text) {
            Token::Kw(keyword)
        } else {
            Token::Ident(text.to_string())
        }
    }

    /// The seven keywords A.1 adds: the only words `r#` can spell as identifiers.
    const ADDED_KEYWORDS: [&'static str; 7] = ["module", "nature", "signal", "on", "out", "flip", "after"];

    /// `r#word`: an identifier for an added keyword, `E0900` for any other word.
    fn raw_identifier(&mut self, word: &str, span: Span) {
        if Lexer::ADDED_KEYWORDS.contains(&word) {
            self.tokens.push((Token::RawIdent(word.to_string()), span));
            return;
        }
        let list = Lexer::ADDED_KEYWORDS.map(|word| format!("`{word}`")).join(", ");
        self.diagnostics.push(
            Diagnostic::new(
                "E0900",
                format!("`r#{word}` is not a raw identifier: `r#` is accepted only for {list}"),
                span,
                "not a raw identifier",
            )
            .with_help("write the word without `r#`; Rust's own keywords stay reserved"),
        );
        self.tokens.push((Token::Error, span));
    }

    /// A number lexeme: its token, and `E0909` when the value is out of range.
    fn number(&mut self, text: &str, span: Span) {
        match NumberLiteral::new(text).classify(span) {
            Ok(token) => {
                self.diagnostics.extend(NumberLiteral::range_diagnostic(&token, span));
                self.tokens.push((token, span));
            }
            Err(diagnostic) => {
                self.diagnostics.push(diagnostic);
                self.tokens.push((Token::Error, span));
            }
        }
    }

    /// `r#"..."#`: a string whose body is kept with backslash and quote escaped.
    fn raw_string(&mut self, text: &str, span: Span) {
        let open = text.find('"').map_or(1, |index| index + 1);
        let hashes = open - 2;
        let body = &text[open..text.len() - 1 - hashes];
        let raw_hashes = Some(u8::try_from(hashes).unwrap_or(u8::MAX));
        self.tokens.push((Token::Str { text: Lexer::cook(body), raw_hashes }, span));
    }

    /// The cooked spelling of a raw string's body: backslash and quote escaped,
    /// so the token's text reads back as the same value.
    fn cook(body: &str) -> String {
        body.replace('\\', "\\\\").replace('"', "\\\"")
    }

    /// A string literal: its text unprocessed, one `E0900` per undefined escape.
    fn string(&mut self, text: &str, span: Span) {
        let body = &text[1..text.len() - 1];
        for bad in Lexer::unescape(body).1 {
            let at = Span::new(span.start + 1 + bad.start, span.start + 1 + bad.end);
            self.diagnostics.push(
                Diagnostic::new(
                    "E0900",
                    format!("undefined escape `{}` in a string literal", &body[bad.clone()]),
                    at,
                    "not an escape of the language",
                )
                .with_help("the escapes are `\\n \\r \\t \\\\ \\0 \\' \\\"`, `\\xHH` up to `\\x7F` and `\\u{..}`"),
            );
        }
        self.tokens.push((Token::Str { text: body.to_string(), raw_hashes: None }, span));
    }

    /// A `//` comment: an outer or inner doc comment is a token, any other is a plain comment.
    fn line_comment(&mut self, text: &str, span: Span) {
        let line = text.strip_suffix('\r').unwrap_or(text);
        if line.starts_with("///") && !line.starts_with("////") {
            self.tokens.push((Token::DocOuter(line[3..].to_string()), span));
        } else if line.starts_with("//!") {
            self.tokens.push((Token::DocInner(line[3..].to_string()), span));
        } else {
            self.comments.push(span);
        }
    }

    /// A `/* */` comment. Rust's rule: `/**` and `/*!` open a doc comment, except
    /// `/***` (and longer) and the empty `/**/`.
    fn block_comment(&mut self, text: &str, span: Span) {
        let body = || text[3..text.len() - 2].to_string();
        if text.starts_with("/**") && !text.starts_with("/***") && text.len() > 4 {
            self.tokens.push((Token::DocOuterBlock(body()), span));
        } else if text.starts_with("/*!") {
            self.tokens.push((Token::DocInnerBlock(body()), span));
        } else {
            self.comments.push(span);
        }
    }

    /// A lifetime: the lifetime of a reference and a loop label are dropped with
    /// one `E0905` and leave no token, any other is an error token. The number of
    /// bytes of `rest` to skip: a loop label's colon, otherwise zero.
    fn lifetime(&mut self, text: &str, span: Span, rest: &str) -> usize {
        if self.tokens.last().is_some_and(|(token, _)| *token == Token::Punct(Punct::And)) {
            // The lifetime of a reference (`&'a T`, `&'static str`): it is part of the removed
            // reference form, so one E0905 and no token; the reference reports itself.
            self.diagnostics.push(Lexer::dropped_class(text, span));
            return 0;
        }
        if let Some(colon) = self.loop_label(rest) {
            // A loop label (`'a: loop`, `break 'a`): one E0905, and the label and its
            // colon leave no token, so the parse goes on.
            self.diagnostics.push(
                Diagnostic::new("E0905", "loop labels are not part of the language", span, "dropped token class")
                    .with_help("restructure the loop: a labelled `break` or `continue` has no equivalent"),
            );
            return colon;
        }
        self.dropped(text, span);
        0
    }

    /// Whether the lifetime just lexed is a loop label: it follows `break` or
    /// `continue`, or it is followed by `:` and a loop (`'a: loop`, `'a: while`,
    /// `'a: for`, `'a: {`). The number of bytes of `rest` to skip: the label's
    /// colon, or zero.
    fn loop_label(&self, rest: &str) -> Option<usize> {
        if matches!(self.tokens.last(), Some((Token::Kw(Kw::Break | Kw::Continue), _))) {
            return Some(0);
        }
        let after_space = rest.trim_start();
        let colon = rest.len() - after_space.len();
        let body = after_space.strip_prefix(':').filter(|body| !body.starts_with(':'))?;
        let word = body.trim_start();
        let starts_loop = ["loop", "while", "for"].iter().any(|keyword| {
            word.strip_prefix(keyword).is_some_and(|after| !after.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
        }) || word.starts_with('{');
        starts_loop.then_some(colon + 1)
    }

    /// A token of a dropped class: one `E0905` and an error token.
    fn dropped(&mut self, text: &str, span: Span) {
        self.diagnostics.push(Lexer::dropped_class(text, span));
        self.tokens.push((Token::Error, span));
    }

    /// The `E0905` of a byte string, byte or character literal, or lifetime.
    fn dropped_class(text: &str, span: Span) -> Diagnostic {
        let what = if text.starts_with("b\"") {
            "byte string literal"
        } else if text.starts_with("b'") {
            "byte literal"
        } else if text.starts_with('\'') && text.ends_with('\'') && text.len() > 1 {
            "character literal"
        } else {
            "lifetime"
        };
        Diagnostic::new("E0905", format!("{what} is not part of the language"), span, "dropped token class")
            .with_help("`char` is not a value type and there are no lifetimes: nothing is lent, every value is copied")
    }

    /// A token `logos` could not match, or a callback rejected: an unterminated
    /// comment or string, a stray quote, or an unexpected character.
    fn failure(&mut self, span: Span) {
        let rest = &self.source[span.start..];
        if rest.starts_with("/*") {
            self.diagnostics.push(
                Diagnostic::new(
                    "E0900",
                    "unterminated block comment",
                    Span::new(span.start, span.start + 2),
                    "this comment is never closed",
                )
                .with_help("close it with `*/`"),
            );
        } else if rest.starts_with('"') || rest.starts_with("b\"") || rest.starts_with('r') || rest.starts_with("br") {
            // A string, byte string or raw string whose closing quote never came:
            // the opener is the quote or the `r#*"` run.
            let open = rest.find('"').map_or(1, |index| index + 1);
            self.diagnostics.push(
                Diagnostic::new(
                    "E0900",
                    "unterminated string literal",
                    Span::new(span.start, span.start + open),
                    "this string is never closed",
                )
                .with_help("close it with `\"`"),
            );
            self.tokens.push((Token::Error, span));
        } else if rest.starts_with('\'') || rest.starts_with("b'") {
            self.diagnostics.push(Lexer::dropped_class("'", span));
            self.tokens.push((Token::Error, span));
        } else {
            let character = rest.chars().next().unwrap_or('\0');
            self.diagnostics.push(Diagnostic::new(
                "E0900",
                format!("unexpected character `{character}`"),
                Span::new(span.start, span.start + character.len_utf8()),
                "not a token of the language",
            ));
            self.tokens.push((Token::Error, span));
        }
    }

    /// Process the escapes of a string body: the value, and the byte range of each
    /// escape Rust does not define (`\q`, `\x9`, `\x80`, `\u{110000}`). A
    /// backslash before a newline skips the newline and the whitespace after it.
    pub fn unescape(body: &str) -> (String, Vec<std::ops::Range<usize>>) {
        let bytes = body.as_bytes();
        let (mut value, mut bad, mut index) = (String::with_capacity(body.len()), Vec::new(), 0);
        while index < bytes.len() {
            if bytes[index] != b'\\' {
                let Some(character) = body[index..].chars().next() else { break };
                value.push(character);
                index += character.len_utf8();
                continue;
            }
            let start = index;
            index += 1;
            let Some(&escape) = bytes.get(index) else {
                bad.push(start..index);
                break;
            };
            index += 1;
            match escape {
                b'n' => value.push('\n'),
                b'r' => value.push('\r'),
                b't' => value.push('\t'),
                b'\\' => value.push('\\'),
                b'0' => value.push('\0'),
                b'\'' => value.push('\''),
                b'"' => value.push('"'),
                b'\n' | b'\r' => {
                    while index < bytes.len() && matches!(bytes[index], b' ' | b'\t' | b'\n' | b'\r') {
                        index += 1;
                    }
                }
                b'x' => {
                    let count = bytes[index..].iter().take(2).take_while(|byte| byte.is_ascii_hexdigit()).count();
                    index += count;
                    match (count, u8::from_str_radix(&body[index - count..index], 16)) {
                        (2, Ok(byte)) if byte <= 0x7f => value.push(byte as char),
                        _ => bad.push(start..index),
                    }
                }
                b'u' if bytes.get(index) == Some(&b'{') => {
                    let close = body[index..].find('}').map(|offset| index + offset);
                    let end = close.map_or(index + 1, |close| close + 1);
                    let digits = close.map(|close| body[index + 1..close].replace('_', ""));
                    match digits.and_then(|digits| u32::from_str_radix(&digits, 16).ok()).and_then(char::from_u32) {
                        Some(character) => value.push(character),
                        None => bad.push(start..end),
                    }
                    index = end;
                }
                _ => {
                    // The escape is the backslash and the char after it.
                    index = start + 1 + body[start + 1..].chars().next().map_or(0, char::len_utf8);
                    bad.push(start..index);
                }
            }
        }
        (value, bad)
    }
}

/// SI suffixes of Appendix A.2 and their decimal exponents.
const SI_SUFFIXES: &[(char, i32)] = &[
    ('T', 12), ('G', 9), ('M', 6), ('k', 3), ('m', -3),
    ('u', -6), ('n', -9), ('p', -12), ('f', -15), ('a', -18),
];

/// The text of a number literal (Appendix A.2): its classification into a
/// [`Token`], and the value, suffix and range of what it spells.
pub struct NumberLiteral<'text> {
    /// The lexeme, type or SI suffix included.
    text: &'text str,
}

impl<'text> NumberLiteral<'text> {
    /// The literal spelled `text`.
    pub fn new(text: &'text str) -> Self {
        NumberLiteral { text }
    }

    /// The width and signedness of an integer type suffix: `uN` and `iN` for `N` in
    /// `1..=128` (§1 rule 5), and `usize` and `isize` (64 bits).
    fn integer_type(suffix: &str) -> Option<(bool, u32)> {
        let signed = suffix.starts_with('i');
        let width = match suffix {
            "usize" | "isize" => 64,
            _ => {
                let digits = suffix.strip_prefix(['u', 'i'])?;
                if digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                digits.parse::<u32>().ok().filter(|width| (1..=128).contains(width))?
            }
        };
        Some((signed, width))
    }

    /// The three parts of an integer literal's text: its radix, its digits
    /// (underscores included) and its type suffix.
    fn integer_parts(&self) -> (u32, &'text str, &'text str) {
        let text = self.text;
        let (radix, body) = match text.get(..2) {
            Some("0x" | "0X") => (16, &text[2..]),
            Some("0o" | "0O") => (8, &text[2..]),
            Some("0b" | "0B") => (2, &text[2..]),
            _ => (10, text),
        };
        let end = body.find(|c: char| !(c == '_' || c.is_digit(radix))).unwrap_or(body.len());
        (radix, &body[..end], &body[end..])
    }

    /// The value of an integer literal: `None` past its suffix type's range
    /// (`iN` admits `2^(N-1)`, the magnitude of its minimum: the minus sign is an
    /// operator, A.2 rule 7) or, unsuffixed, past `u128::MAX`.
    pub fn int_value(&self) -> Option<u128> {
        let (radix, digits, suffix) = self.integer_parts();
        let max = match NumberLiteral::integer_type(suffix) {
            Some((false, width)) => u128::MAX >> (128 - width),
            Some((true, width)) => 1u128 << (width - 1),
            None => u128::MAX,
        };
        digits
            .chars()
            .filter(|c| *c != '_')
            .try_fold(0u128, |sum, c| sum.checked_mul(radix as u128)?.checked_add(c.to_digit(radix)? as u128))
            .filter(|value| *value <= max)
    }

    /// The type suffix of an integer literal's text, empty when there is none.
    pub fn int_suffix_text(&self) -> &'text str {
        self.integer_parts().2
    }

    /// The index after the run of digits and underscores that starts at `index`.
    fn skip_digits(bytes: &[u8], mut index: usize) -> usize {
        while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'_') {
            index += 1;
        }
        index
    }

    /// The suffix of a float literal's text (`f32`, `f64` or an SI letter), empty
    /// when there is none: what follows the digits, the fraction and the exponent.
    pub fn float_suffix(&self) -> &'text str {
        let bytes = self.text.as_bytes();
        let mut index = NumberLiteral::skip_digits(bytes, 0);
        if index < bytes.len() && bytes[index] == b'.' {
            index = NumberLiteral::skip_digits(bytes, index + 1);
        }
        if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
            let digits_from = if matches!(bytes.get(index + 1), Some(b'+' | b'-')) { index + 2 } else { index + 1 };
            if bytes.get(digits_from).is_some_and(u8::is_ascii_digit) {
                index = NumberLiteral::skip_digits(bytes, digits_from);
            }
        }
        &self.text[index..]
    }

    /// The value of a float literal from its text (SI suffix applied, a `f32` or
    /// `f64` suffix ignored): what the lexer computed when it read it. A text
    /// that is no float literal gives `NaN`.
    pub fn float_value(&self) -> f64 {
        let suffix = self.float_suffix();
        let number = &self.text[..self.text.len() - suffix.len()];
        let exponent = match suffix {
            "" | "f32" | "f64" => 0,
            _ => match NumberLiteral::si_exponent(suffix) {
                Some(exponent) => exponent,
                None => return f64::NAN,
            },
        };
        NumberLiteral::parse_float(number, exponent)
    }

    /// The decimal exponent of a one-letter SI suffix, `None` for anything else.
    fn si_exponent(suffix: &str) -> Option<i32> {
        let letter = suffix.chars().next()?;
        let (_, exponent) = SI_SUFFIXES.iter().find(|(symbol, _)| *symbol == letter)?;
        (suffix.chars().count() == 1).then_some(*exponent)
    }

    /// The nearest `f64` to `number` scaled by `10^exponent`: the exact decimal
    /// product, parsed once, never `number * 1e-6` (A.2 rule 6).
    fn parse_float(number: &str, exponent: i32) -> f64 {
        let clean: String = number.chars().filter(|c| *c != '_').collect();
        let text = if exponent == 0 { clean } else { format!("{clean}e{exponent}") };
        text.parse().unwrap_or(f64::NAN)
    }

    /// `E0909` for a literal out of range: a suffixed float whose `f64` is infinite (an unsuffixed
    /// float is read in the format of its position, so the checker owns its range, A.8 rule 8), or an integer past
    /// its suffix type (`iN` admits `2^(N-1)`: the minus sign is an operator, A.2
    /// rule 7) or, unsuffixed, past 128 bits.
    fn range_diagnostic(token: &Token, span: Span) -> Option<Diagnostic> {
        let (text, what) = match token {
            Token::Float { value, raw } if value.is_infinite() && !NumberLiteral::new(raw).float_suffix().is_empty() => {
                (raw, "a float literal beyond the range of `f64`".to_string())
            }
            Token::Int(text) => {
                let literal = NumberLiteral::new(text);
                if literal.int_value().is_some() {
                    return None;
                }
                (text, format!("an integer literal beyond the range of `{}`", match literal.int_suffix_text() {
                    "" => "u128",
                    suffix => suffix,
                }))
            }
            _ => return None,
        };
        Some(
            Diagnostic::new("E0909", format!("`{text}` is out of range: {what}"), span, "literal out of range")
                .with_help("write a value the type holds, or a wider type suffix"),
        )
    }

    /// Classify a number lexeme per Appendix A.2: plain integer or float, a
    /// Rust-suffixed literal, or an SI literal; anything else is `E0902` (a
    /// malformed SI literal) or `E0900` (an unknown suffix).
    fn classify(&self, span: Span) -> Result<Token, Diagnostic> {
        match self.text.get(..2) {
            Some("0x" | "0X") => self.classify_with_base(span, |c| c.is_ascii_hexdigit() || c == '_'),
            Some("0o" | "0O") => self.classify_with_base(span, |c| ('0'..='7').contains(&c) || c == '_'),
            Some("0b" | "0B") => self.classify_with_base(span, |c| c == '0' || c == '1' || c == '_'),
            _ => self.classify_decimal(span),
        }
    }

    /// A non-decimal literal: digits, then whatever is left is a suffix.
    fn classify_with_base(&self, span: Span, is_digit: fn(char) -> bool) -> Result<Token, Diagnostic> {
        let body = &self.text[2..];
        let digits_end = body.find(|c: char| !is_digit(c)).unwrap_or(body.len());
        let suffix = &body[digits_end..];
        if digits_end == 0 {
            Err(self.unknown_suffix(span))
        } else if suffix.is_empty() || NumberLiteral::integer_type(suffix).is_some() {
            Ok(Token::Int(self.text.to_string()))
        } else if suffix.starts_with(NumberLiteral::is_si_letter) {
            Err(self.malformed_si("a non-decimal literal takes no SI suffix", span))
        } else {
            Err(self.unknown_suffix(span))
        }
    }

    /// The byte length of the number in a decimal lexeme (digits, fraction,
    /// exponent), whether it has a fraction and whether it has an exponent.
    fn scan_decimal(&self) -> (usize, bool, bool) {
        let bytes = self.text.as_bytes();
        let mut index = NumberLiteral::skip_digits(bytes, 0);
        let mut has_fraction = false;
        if index + 1 < bytes.len() && bytes[index] == b'.' && bytes[index + 1].is_ascii_digit() {
            index = NumberLiteral::skip_digits(bytes, index + 1);
            has_fraction = true;
        }
        let mut has_exponent = false;
        if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
            let mut digits_from = index + 1;
            if digits_from < bytes.len() && matches!(bytes[digits_from], b'+' | b'-') {
                digits_from += 1;
            }
            if digits_from < bytes.len() && bytes[digits_from].is_ascii_digit() {
                index = NumberLiteral::skip_digits(bytes, digits_from);
                has_exponent = true;
            }
        }
        (index, has_fraction, has_exponent)
    }

    /// A decimal literal: `DEC_DIGITS ('.' DEC_DIGITS)? ([eE][+-]?DEC_DIGITS)? suffix`.
    fn classify_decimal(&self, span: Span) -> Result<Token, Diagnostic> {
        let (end, has_fraction, has_exponent) = self.scan_decimal();
        let number = &self.text[..end];
        let suffix = &self.text[end..];
        let float = || Token::Float { value: NumberLiteral::parse_float(number, 0), raw: self.text.to_string() };
        if suffix.is_empty() {
            return Ok(if has_fraction || has_exponent { float() } else { Token::Int(self.text.to_string()) });
        }
        if suffix == "f32" || suffix == "f64" {
            return Ok(float());
        }
        if NumberLiteral::integer_type(suffix).is_some() {
            return if has_fraction || has_exponent {
                Err(self.unknown_suffix(span))
            } else {
                Ok(Token::Int(self.text.to_string()))
            };
        }
        self.classify_si(number, suffix, has_exponent, span)
    }

    /// A decimal literal whose suffix is no type suffix: an SI literal, or an error.
    fn classify_si(&self, number: &str, suffix: &str, has_exponent: bool, span: Span) -> Result<Token, Diagnostic> {
        let mut letters = suffix.chars();
        match letters.next().and_then(|letter| SI_SUFFIXES.iter().find(|(symbol, _)| *symbol == letter)) {
            Some((_, exponent)) if suffix.chars().count() == 1 => {
                if has_exponent {
                    Err(self.malformed_si("an SI literal has no exponent", span))
                } else {
                    Ok(Token::Float { value: NumberLiteral::parse_float(number, *exponent), raw: self.text.to_string() })
                }
            }
            // An SI letter then a type suffix (`5mf64`) is a malformed SI
            // literal; any other letters (`1meg`, `1kHz`) are an unknown suffix.
            Some(_) if NumberLiteral::is_type_suffix(letters.as_str().trim_start_matches('_')) => {
                if has_exponent {
                    Err(self.malformed_si("an SI literal has no exponent", span))
                } else {
                    Err(self.malformed_si("an SI literal has no type suffix", span))
                }
            }
            _ => Err(self.unknown_suffix(span)),
        }
    }

    /// Whether `suffix` is `f32`, `f64` or an integer type.
    fn is_type_suffix(suffix: &str) -> bool {
        suffix == "f32" || suffix == "f64" || NumberLiteral::integer_type(suffix).is_some()
    }

    /// Whether `letter` is an SI suffix letter.
    fn is_si_letter(letter: char) -> bool {
        SI_SUFFIXES.iter().any(|(symbol, _)| *symbol == letter)
    }

    /// `E0902`: a malformed SI literal, for the reason `why`.
    fn malformed_si(&self, why: &str, span: Span) -> Diagnostic {
        Diagnostic::new("E0902", format!("malformed SI literal `{}`: {why}", self.text), span, "malformed SI literal")
            .with_help("an SI suffix goes on a decimal literal without exponent or type suffix, e.g. `2.2u`")
    }

    /// `E0900`: a suffix that is neither a type nor an SI letter.
    fn unknown_suffix(&self, span: Span) -> Diagnostic {
        Diagnostic::new("E0900", format!("invalid literal `{}`", self.text), span, "unknown literal suffix")
    }
}
