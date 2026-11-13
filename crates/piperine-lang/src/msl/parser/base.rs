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
    select! { Token::Ident(name) => name }
        .map_with(|name, e| Ident { name, span: Loc(e.span()) })
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
    .map_with(|name, e| Ident { name: name.to_string(), span: Loc(e.span()) });
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
