//! Error recovery: a construct that fails to parse is replaced by an error node and parsing
//! resumes at the next boundary, so one run reports every error and still returns a partial tree.
//! Recovery always consumes at least one token, so a repeating parser cannot loop on it.

use super::*;

/// The recovery of a failed item or statement, and the skips it is made of.
pub(crate) struct Recovery;

impl Recovery {
    /// Tokens up to, not including, the next `;`, `{` or `}`: the rest of a form
    /// whose own diagnostic says all there is to say.
    pub(crate) fn skip_to_end<'a, I: In<'a>>() -> impl Parser<'a, I, (), Extra<'a>> + Clone {
        any()
            .filter(|t: &Token| !matches!(t, Token::Punct(Punct::Semi | Punct::LBrace | Punct::RBrace)))
            .repeated()
            .ignored()
    }

    /// The rest of an item head up to its `{` or `;` (not consumed); a bracket,
    /// parenthesis or brace group inside it is skipped whole.
    pub(crate) fn skip_head_rest<'a, I: In<'a>>() -> P<'a, I, ()> {
        choice((
            any().filter(|t: &Token| matches!(t, Token::Punct(Punct::LParen | Punct::LBracket))).rewind().ignore_then(Recovery::skip_group()),
            any().filter(|t: &Token| !TokenInput::is_closer(t) && !matches!(t, Token::Punct(Punct::Semi | Punct::LBrace))).ignored(),
        ))
        .repeated()
        .ignored()
        .boxed()
    }

    /// A balanced `( .. )`, `[ .. ]` or `{ .. }` group, contents skipped. An
    /// opener closed by another kind of delimiter (or never) is no group.
    pub(crate) fn skip_group<'a, I: In<'a>>() -> P<'a, I, ()> {
        recursive(|nested| {
            let inner = choice((
                nested,
                any().filter(|t: &Token| !TokenInput::is_opener(t) && !TokenInput::is_closer(t)).ignored(),
            ))
            .repeated()
            .boxed();
            let group = |open: Punct, close: Punct| {
                just(Token::Punct(open)).ignore_then(inner.clone()).then_ignore(just(Token::Punct(close))).ignored()
            };
            choice((
                group(Punct::LParen, Punct::RParen),
                group(Punct::LBracket, Punct::RBracket),
                group(Punct::LBrace, Punct::RBrace),
            ))
        })
        .boxed()
    }

    /// A token that starts a file-level item.
    pub(crate) fn starts_item(t: &Token) -> bool {
        matches!(
            t,
            Token::Kw(
                Kw::Fn | Kw::Struct | Kw::Enum | Kw::Impl | Kw::Trait | Kw::Const | Kw::Use | Kw::Mod
                    | Kw::Module | Kw::Nature | Kw::Pub | Kw::Extern
            ) | Token::Punct(Punct::Pound)
                | Token::DocOuter(_)
                | Token::DocOuterBlock(_)
        )
    }

    /// Skip the rest of a failed item: stop after its `{ .. }` body or `;`, or
    /// before the next item keyword or a closing delimiter that is not ours.
    pub(crate) fn skip_rest_of_item<'a, I: In<'a>>() -> P<'a, I, ()> {
        let behavior_start = select! { Token::Ident(s) if s == "analog" || s == "digital" => () }
            .then(any().filter(|t: &Token| matches!(t, Token::Kw(Kw::For) | Token::Punct(Punct::Lt))))
            .rewind()
            .ignored();
        // `Name<..>`: a generic list is skipped to its `>`, item keywords inside
        // it (`const N: u32`) are no place to resume.
        let angle_list = select! { Token::Ident(_) => () }
            .then(just(Token::Punct(Punct::Lt)))
            .ignore_then(
                any()
                    .filter(|t: &Token| !TokenInput::is_closer(t) && !matches!(t, Token::Punct(Punct::Gt | Punct::Semi | Punct::LBrace)))
                    .repeated(),
            )
            .then_ignore(just(Token::Punct(Punct::Gt)).or_not())
            .ignored();
        let more = choice((
            angle_list,
            // A bracket or parenthesis group is skipped whole.
            any()
                .filter(|t: &Token| matches!(t, Token::Punct(Punct::LParen | Punct::LBracket)))
                .rewind()
                .ignore_then(Recovery::skip_group()),
            any()
                .filter(|t: &Token| {
                    !TokenInput::is_closer(t) && !Recovery::starts_item(t) && !matches!(t, Token::Punct(Punct::Semi | Punct::LBrace))
                })
                .ignored(),
        ))
        .and_is(behavior_start.not());
        let done = choice((
            any().filter(TokenInput::is_left_brace).rewind().ignore_then(Recovery::skip_group()),
            just(Token::Punct(Punct::Semi)).ignored(),
        ));
        more.repeated().ignore_then(done.or_not()).ignored().boxed()
    }

    /// Skip the rest of a failed statement: through its `;`, or up to the `}`
    /// that closes the block.
    pub(crate) fn skip_rest_of_statement<'a, I: In<'a>>() -> P<'a, I, ()> {
        // Only the `}` that ends the block stops the skip; a `)` or `]` that no
        // group of the statement closes is skipped with it.
        let more = choice((
            any().filter(TokenInput::is_opener).rewind().ignore_then(Recovery::skip_group()),
            any()
                .filter(|t: &Token| !matches!(t, Token::Punct(Punct::Semi | Punct::RBrace)))
                .ignored(),
        ));
        more.repeated().ignore_then(just(Token::Punct(Punct::Semi)).ignored().or_not()).ignored().boxed()
    }

    /// `p`, or on failure an error node covering what was skipped. `item_like`
    /// picks the boundary: a whole item (after its body) or a statement (after
    /// its `;`).
    pub(crate) fn recover<'a, I: In<'a>, O: Clone + 'a>(
        p: P<'a, I, O>,
        item_like: bool,
        error: fn(Span) -> O,
    ) -> P<'a, I, O> {
        Recovery::recover_unless(p, item_like, error, None)
    }

    /// [`recover`] that stands aside where `unless` matches: a module body ends
    /// with an expression that is no item, and must not be skipped as one.
    pub(crate) fn recover_unless<'a, I: In<'a>, O: Clone + 'a>(
        p: P<'a, I, O>,
        item_like: bool,
        error: fn(Span) -> O,
        unless: Option<P<'a, I, ()>>,
    ) -> P<'a, I, O> {
        let skip = if item_like { Recovery::skip_rest_of_item() } else { Recovery::skip_rest_of_statement() };
        // The first token is taken unconditionally (never a closing delimiter:
        // the enclosing group must see it), so recovery makes progress.
        let first = any().filter(|t: &Token| !TokenInput::is_closer(t));
        let skipped = first.ignore_then(skip).map_with(move |_, e| error(e.span()));
        match unless {
            None => p.recover_with(via_parser(skipped)).boxed(),
            Some(unless) => p.recover_with(via_parser(skipped.and_is(unless.not()))).boxed(),
        }
    }
}
