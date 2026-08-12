//! The shared token cursor every `grammar/{expr,stmt,item}.rs` production
//! method is written against. One `Grammar` per parse; `expr.rs`/`stmt.rs`/
//! `item.rs` each contribute `impl Grammar` blocks (MD-13 rule 4: files
//! named by what they parse, not by "helpers").

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
