//! Shared parser plumbing: error types, the recursive handles, token helpers.

use chumsky::extra;
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use chumsky::recursive::{Indirect, Recursive};

use crate::msl::ast::*;
use crate::msl::lexer::{Kw, Punct, Token};
use crate::msl::span::Span;

use super::{expr, item, pat, ty};

/// A custom parse error: a spec code, a message, an optional help line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Custom {
    pub code: &'static str,
    pub message: String,
    pub help: Option<String>,
}

impl std::fmt::Display for Custom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub(crate) type Err<'a> = Rich<'a, Token, Span, Custom>;
pub(crate) type Extra<'a> = extra::Err<Err<'a>>;
pub(crate) type P<'a, I, O> = Boxed<'a, 'a, I, O, Extra<'a>>;
pub(crate) type Rec<'a, I, O> = Recursive<Indirect<'a, 'a, I, O, Extra<'a>>>;

/// An input of spanned MSL tokens.
pub(crate) trait In<'a>: ValueInput<'a, Token = Token, Span = Span> {}
impl<'a, I> In<'a> for I where I: ValueInput<'a, Token = Token, Span = Span> {}

/// The mutually recursive parsers, declared up front and defined once.
#[derive(Clone)]
pub(crate) struct Handles<'a, I: In<'a>> {
    pub ty: Rec<'a, I, Type>,
    /// A pattern, alternatives (`a | b`) included.
    pub pat: Rec<'a, I, Pat>,
    /// A pattern with no top-level alternative: `let`, parameters, `for`.
    pub pat1: Rec<'a, I, Pat>,
    pub expr: Rec<'a, I, Expr>,
    /// An expression that refuses a struct literal at the top level.
    pub expr_ns: Rec<'a, I, Expr>,
    pub block: Rec<'a, I, Block>,
    pub item: Rec<'a, I, Item>,
}

pub(crate) fn handles<'a, I: In<'a>>() -> Handles<'a, I> {
    let mut h = Handles {
        ty: Recursive::declare(),
        pat: Recursive::declare(),
        pat1: Recursive::declare(),
        expr: Recursive::declare(),
        expr_ns: Recursive::declare(),
        block: Recursive::declare(),
        item: Recursive::declare(),
    };
    let p = ty::build(&h);
    h.ty.define(p);
    let (pat, pat1) = pat::build(&h);
    h.pat.define(pat);
    h.pat1.define(pat1);
    let p = expr::build(&h, true, h.expr.clone());
    h.expr.define(p);
    let p = expr::build(&h, false, h.expr_ns.clone());
    h.expr_ns.define(p);
    let p = expr::block(&h);
    h.block.define(p);
    let p = item::build(&h);
    h.item.define(p);
    h
}

// ---- token-level helpers ----------------------------------------------------------------------

pub(crate) fn punct<'a, I: In<'a>>(p: Punct) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
    just(Token::Punct(p)).ignored()
}

pub(crate) fn kw<'a, I: In<'a>>(k: Kw) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
    just(Token::Kw(k)).ignored()
}

pub(crate) fn ident<'a, I: In<'a>>() -> impl Parser<'a, I, Ident, Extra<'a>> + Clone {
    select! { Token::Ident(name) => (name, false), Token::RawIdent(name) => (name, true) }
        .map_with(|(name, raw), e| Ident { name, raw, span: Loc(e.span()) })
        .labelled("identifier")
}

/// An identifier or one of `self`, `Self`, `super`, `crate`: a path segment.
pub(crate) fn path_ident<'a, I: In<'a>>() -> impl Parser<'a, I, Ident, Extra<'a>> + Clone {
    let kw_seg = select! {
        Token::Kw(Kw::SelfLower) => "self",
        Token::Kw(Kw::SelfUpper) => "Self",
        Token::Kw(Kw::Super) => "super",
        Token::Kw(Kw::Crate) => "crate",
    }
    .map_with(|name, e| Ident { name: name.to_string(), raw: false, span: Loc(e.span()) });
    ident().or(kw_seg).labelled("identifier")
}

/// Any literal: integer, float, string, `true`/`false`, `Unknown`.
pub(crate) fn lit<'a, I: In<'a>>() -> impl Parser<'a, I, Lit, Extra<'a>> + Clone {
    select! {
        Token::Int(s) => LitKind::Int(s),
        Token::Float { value, raw } => LitKind::Float { value, raw },
        Token::Str(s) => LitKind::Str(s),
        Token::Kw(Kw::True) => LitKind::Bool(true),
        Token::Kw(Kw::False) => LitKind::Bool(false),
        Token::Unknown => LitKind::Unknown,
    }
    .map_with(|kind, e| Lit { kind, span: Loc(e.span()) })
    .labelled("literal")
}

pub(crate) fn lit_expr<'a, I: In<'a>>() -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
    lit().map(|l| Expr { span: l.span, kind: ExprKind::Lit(l) })
}


fn delim_open(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace))
}

fn delim_close(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace))
}

/// A balanced `( .. )`, `[ .. ]` or `{ .. }` group, contents skipped.
pub(crate) fn skip_group<'a, I: In<'a>>() -> P<'a, I, ()> {
    recursive(|nested| {
        let inner = choice((
            nested,
            any().filter(|t: &Token| !delim_open(t) && !delim_close(t)).ignored(),
        ))
        .repeated();
        any().filter(delim_open).ignore_then(inner).then_ignore(any().filter(delim_close)).ignored()
    })
    .boxed()
}

/// The `E0901` of a macro invocation or definition.
pub(crate) fn macro_error(span: Span) -> Err<'static> {
    Rich::custom(
        span,
        Custom {
            code: "E0901",
            message: "macros are not part of the language".into(),
            help: Some("the only `!` forms are the built-ins `emit!` and `include_str!`".into()),
        },
    )
}

/// A string literal token as a [`Lit`].
pub(crate) fn string_lit<'a, I: In<'a>>() -> impl Parser<'a, I, Lit, Extra<'a>> + Clone {
    select! { Token::Str(s) => LitKind::Str(s) }
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

fn is_lbrace(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::LBrace))
}

fn is_closer(t: &Token) -> bool {
    delim_close(t)
}

/// A token that starts a file-level item.
fn starts_item(t: &Token) -> bool {
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
fn skip_rest_of_item<'a, I: In<'a>>() -> P<'a, I, ()> {
    let behavior_start = select! { Token::Ident(s) if s == "analog" || s == "digital" => () }
        .then(any().filter(|t: &Token| matches!(t, Token::Kw(Kw::For) | Token::Punct(Punct::Lt))))
        .rewind()
        .ignored();
    let more = choice((
        // A bracket or parenthesis group is skipped whole.
        any()
            .filter(|t: &Token| matches!(t, Token::Punct(Punct::LParen | Punct::LBracket)))
            .rewind()
            .ignore_then(skip_group()),
        any()
            .filter(|t: &Token| {
                !is_closer(t) && !starts_item(t) && !matches!(t, Token::Punct(Punct::Semi | Punct::LBrace))
            })
            .ignored(),
    ))
    .and_is(behavior_start.not());
    let done = choice((
        any().filter(is_lbrace).rewind().ignore_then(skip_group()),
        just(Token::Punct(Punct::Semi)).ignored(),
    ));
    more.repeated().ignore_then(done.or_not()).ignored().boxed()
}

/// Skip the rest of a failed statement: through its `;`, or up to the `}`
/// that closes the block.
fn skip_rest_of_stmt<'a, I: In<'a>>() -> P<'a, I, ()> {
    let more = choice((
        any().filter(delim_open).rewind().ignore_then(skip_group()),
        any()
            .filter(|t: &Token| !is_closer(t) && !matches!(t, Token::Punct(Punct::Semi)))
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
    recover_unless(p, item_like, error, None)
}

/// [`recover`] that stands aside where `unless` matches: a module body ends
/// with an expression that is no item, and must not be skipped as one.
pub(crate) fn recover_unless<'a, I: In<'a>, O: Clone + 'a>(
    p: P<'a, I, O>,
    item_like: bool,
    error: fn(Span) -> O,
    unless: Option<P<'a, I, ()>>,
) -> P<'a, I, O> {
    let skip = if item_like { skip_rest_of_item() } else { skip_rest_of_stmt() };
    // The first token is taken unconditionally (never a closing delimiter:
    // the enclosing group must see it), so recovery makes progress.
    let first = any().filter(|t: &Token| !is_closer(t));
    let skipped = first.ignore_then(skip).map_with(move |_, e| error(e.span()));
    match unless {
        None => p.recover_with(via_parser(skipped)).boxed(),
        Some(unless) => p.recover_with(via_parser(skipped.and_is(unless.not()))).boxed(),
    }
}
