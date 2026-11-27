//! Token-level parsing: the parser input over a token vector, token predicates, and the parsers
//! of one token (a keyword, a punctuation, an identifier, a literal).

use super::*;

/// The token-level half of the parser: its input and the parsers of a single token.
pub(crate) struct TokenInput;

impl TokenInput {
    /// The head of a reference form: `&`, `&&` or `&mut`, as one span. The form is removed (B.3.6); the
    /// parser reports it where it reads it and goes on with what follows.
    pub(crate) fn reference_head<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
        choice((TokenInput::punctuation(Punct::And), TokenInput::punctuation(Punct::AndAnd))).then_ignore(TokenInput::keyword(Kw::Mut).or_not()).map_with(|_, e| e.span())
    }

    /// `pub`, or `pub(crate)`, `pub(super)`, `pub(self)`, `pub(in path)`: the span
    /// of `pub`. The restricted forms are removed (B.3.6) and reported once.
    pub(crate) fn visibility<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
        let restriction = any()
            .filter(|t: &Token| matches!(t, Token::Kw(Kw::Crate | Kw::Super | Kw::SelfLower | Kw::In)))
            .then(any().filter(|t: &Token| !matches!(t, Token::Punct(Punct::RParen))).repeated())
            .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
            .map_with(|_, e| e.span());
        TokenInput::keyword(Kw::Pub).map_with(|_, e| e.span()).then(restriction.or_not()).validate(|(at, restricted): (Span, Option<Span>), _, emitter| {
            if let Some(group) = restricted {
                emitter.emit(ParseDiagnostics::removed("restricted visibility", Span::new(at.start, group.end)));
            }
            at
        })
    }

    // ---- token-level helpers ----------------------------------------------------------------------

    /// The punctuation `punct`.
    pub(crate) fn punctuation<'a, I: In<'a>>(punct: Punct) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
        just(Token::Punct(punct)).ignored()
    }

    /// The keyword `word`.
    pub(crate) fn keyword<'a, I: In<'a>>(word: Kw) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
        just(Token::Kw(word)).ignored()
    }

    /// An identifier, raw (`r#word`) or not.
    pub(crate) fn identifier<'a, I: In<'a>>() -> impl Parser<'a, I, Ident, Extra<'a>> + Clone {
        select! { Token::Ident(name) => (name, false), Token::RawIdent(name) => (name, true) }
            .map_with(|(name, raw), e| Ident { name, raw, span: Loc(e.span()) })
            .labelled("identifier")
    }

    /// An identifier or one of `self`, `Self`, `super`, `crate`: a path segment.
    pub(crate) fn path_identifier<'a, I: In<'a>>() -> impl Parser<'a, I, Ident, Extra<'a>> + Clone {
        let kw_seg = select! {
            Token::Kw(Kw::SelfLower) => "self",
            Token::Kw(Kw::SelfUpper) => "Self",
            Token::Kw(Kw::Super) => "super",
            Token::Kw(Kw::Crate) => "crate",
        }
        .map_with(|name, e| Ident { name: name.to_string(), raw: false, span: Loc(e.span()) });
        TokenInput::identifier().or(kw_seg).labelled("identifier")
    }

    /// Any literal: integer, float, string, `true`/`false`.
    pub(crate) fn literal<'a, I: In<'a>>() -> impl Parser<'a, I, Lit, Extra<'a>> + Clone {
        select! {
            Token::Int(s) => LitKind::Int(s),
            Token::Float { value, raw } => LitKind::Float { value, raw },
            Token::Str { text, raw_hashes } => LitKind::Str { text, raw_hashes },
            Token::Kw(Kw::True) => LitKind::Bool(true),
            Token::Kw(Kw::False) => LitKind::Bool(false),
        }
        .map_with(|kind, e| Lit { kind, span: Loc(e.span()) })
        .labelled("literal")
    }

    /// A literal as an expression.
    pub(crate) fn literal_expression<'a, I: In<'a>>() -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
        TokenInput::literal().map(|l| Expr { span: l.span, kind: ExprKind::Lit(l) })
    }

    /// A string literal token as a [`Lit`].
    pub(crate) fn string_literal<'a, I: In<'a>>() -> impl Parser<'a, I, Lit, Extra<'a>> + Clone {
        select! { Token::Str { text, raw_hashes } => LitKind::Str { text, raw_hashes } }
            .map_with(|kind, e| Lit { kind, span: Loc(e.span()) })
            .labelled("string literal")
    }

    /// Ordered choice over boxed alternatives. A tuple `choice` holds every
    /// alternative's temporaries in one stack frame (tens of KiB in a debug
    /// build, per nesting level); a vector of boxed parsers is tried in a loop
    /// and costs one small frame.
    pub(crate) fn first_of<'a, I: In<'a>, O: 'a>(alts: Vec<P<'a, I, O>>) -> P<'a, I, O> {
        choice(alts).boxed()
    }

    // ---- error recovery ---------------------------------------------------------------------------
    //
    // A construct that fails to parse is replaced by an error node and parsing
    // resumes at the next boundary, so one run reports every error in the file
    // and still returns a partial tree. Recovery always consumes at least one
    // token, so a repeating parser cannot loop on it.

    /// Whether the token is `{`.
    pub(crate) fn is_left_brace(t: &Token) -> bool {
        matches!(t, Token::Punct(Punct::LBrace))
    }

    /// The parser's input over a token vector: tokens by value, end of input at `len`.
    pub(crate) fn over<'a>(
        tokens: &'a [(Token, Span)],
        len: usize,
    ) -> impl In<'a> {
        use chumsky::input::Input;
        tokens.map(Span::new(len, len), |(t, s): &(Token, Span)| (t, s))
    }

    /// Whether the token opens a parenthesis, a bracket or a brace.
    pub(crate) fn is_opener(t: &Token) -> bool {
        matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace))
    }

    /// Whether the token closes a parenthesis, a bracket or a brace.
    pub(crate) fn is_closer(t: &Token) -> bool {
        matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace))
    }

    /// Whether a token ends an operand, so the token after it is infix or
    /// postfix; otherwise the next operator token is a prefix form.
    pub(crate) fn ends_operand(t: &Token) -> bool {
        match t {
            Token::Ident(_) | Token::RawIdent(_) | Token::Int(_) | Token::Float { .. } | Token::Str { .. } => true,
            Token::Kw(Kw::SelfLower | Kw::SelfUpper | Kw::True | Kw::False) => true,
            Token::Punct(p) => matches!(p, Punct::RParen | Punct::RBracket | Punct::RBrace | Punct::Question),
            _ => false,
        }
    }
}
