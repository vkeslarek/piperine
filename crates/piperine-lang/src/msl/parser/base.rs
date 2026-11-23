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

/// One row of B.3.6: a form Rust has and the language removes.
pub(crate) struct RemovedForm {
    /// The start of the row's first cell.
    pub row: &'static str,
    pub code: &'static str,
    pub message: &'static str,
    pub help: &'static str,
}

/// The rows of B.3.6, in the order of the spec's table.
pub(crate) const REMOVED_FORMS: &[RemovedForm] = &[
    RemovedForm { row: "type aliases", code: "E0903", message: "type aliases are not part of the language", help: "write the type out" },
    RemovedForm { row: "associated types", code: "E0903", message: "associated types are not part of the language", help: "name the type where it is used" },
    RemovedForm { row: "`static` items", code: "E0903", message: "`static` items are not part of the language", help: "write a `const` item; state lives in signals" },
    RemovedForm { row: "trait-bound `where` clauses", code: "E0900", message: "trait-bound `where` clauses are not part of the language", help: "write the bound in the generic list: `<T: Tr>`" },
    RemovedForm { row: "`impl Trait` types", code: "E0900", message: "`impl Trait` types are not part of the language", help: "write a named generic: `<T: Tr>`" },
    RemovedForm { row: "qualified paths", code: "E0900", message: "qualified paths are not part of the language", help: "write a path through the type: `T::X`" },
    RemovedForm { row: "restricted visibility", code: "E0900", message: "restricted visibility is not part of the language", help: "write `pub` or no visibility" },
    RemovedForm { row: "loop labels", code: "E0905", message: "loop labels are not part of the language", help: "restructure the loop" },
    RemovedForm { row: "references and borrows", code: "E0101", message: "references and borrows are not part of the language", help: "pass the value itself: every value is copied where it is used" },
    RemovedForm { row: "pattern parameters", code: "E0900", message: "pattern parameters are not part of the language", help: "write a plain name, then `let`" },
    RemovedForm { row: "half-open range patterns", code: "E0900", message: "half-open range patterns are not part of the language", help: "write `..=x` or a guard" },
    RemovedForm { row: "a leading `::`", code: "E0900", message: "a leading `::` is not part of the language", help: "write a path from a name in scope" },
    RemovedForm { row: "inner attributes", code: "E0900", message: "inner attributes are not part of the language", help: "write an outer attribute on the item" },
    RemovedForm { row: "the never type", code: "E0900", message: "the never type is not part of the language", help: "write a unit return type" },
    RemovedForm { row: "raw-pointer types", code: "E0900", message: "raw-pointer types are not part of the language", help: "write a value or an array" },
    RemovedForm { row: "named function-pointer parameters", code: "E0900", message: "named function-pointer parameters are not part of the language", help: "write the types only: `fn(f64) -> f64`" },
    RemovedForm { row: "attributes and doc comments on a parameter", code: "E0900", message: "attributes and doc comments on parameters, generic parameters and struct-literal fields are not part of the language", help: "put the attribute or comment on the item" },
    RemovedForm { row: "`union` items", code: "E0900", message: "`union` items are not part of the language", help: "write a `struct` or an `enum`" },
    RemovedForm { row: "`extern` blocks", code: "E0900", message: "`extern` blocks are not part of the language", help: "write an `extern \"C\" fn` item" },
    RemovedForm { row: "`extern crate` items", code: "E0900", message: "`extern crate` is not part of the language", help: "name the dependency under `[dependencies]` in Piperine.toml" },
];

/// The diagnostic of a removed form, by the start of its B.3.6 row.
pub(crate) fn removed(row: &str, span: Span) -> Err<'static> {
    let f = REMOVED_FORMS.iter().find(|f| f.row == row).unwrap_or_else(|| panic!("B.3.6 has no row `{row}`"));
    Rich::custom(span, Custom { code: f.code, message: f.message.into(), help: Some(f.help.into()) })
}

/// The head of a reference form: `&`, `&&` or `&mut`, as one span. The form is removed (B.3.6); the
/// parser reports it where it reads it and goes on with what follows.
pub(crate) fn reference_head<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
    choice((punct(Punct::And), punct(Punct::AndAnd))).then_ignore(kw(Kw::Mut).or_not()).map_with(|_, e| e.span())
}

/// One of the three markers of B.3.6 (`mut`, `ref`, `move`): a reserved word (A.1) with the plain
/// form to write instead.
pub(crate) struct ReservedMarker {
    /// The reserved word.
    pub word: Kw,
    /// What to write in its place.
    pub help: &'static str,
}

/// The markers, one row each.
pub(crate) const RESERVED_MARKERS: &[ReservedMarker] = &[
    ReservedMarker { word: Kw::Mut, help: "write the plain form `let x`: every local may be assigned" },
    ReservedMarker { word: Kw::Ref, help: "write the plain name: a pattern binds a copy" },
    ReservedMarker { word: Kw::Move, help: "write `|x| e`: a closure captures by value" },
];

impl ReservedMarker {
    /// The marker for a keyword, when it is one.
    pub(crate) fn of(word: Kw) -> Option<&'static ReservedMarker> {
        RESERVED_MARKERS.iter().find(|m| m.word == word)
    }

    /// `E0903` at the word `word` found at `span`, with the help of its row.
    pub(crate) fn report(word: Kw, span: Span) -> Err<'static> {
        let help = Self::of(word).map_or("choose another name", |m| m.help);
        Rich::custom(span, Custom { code: "E0903", message: format!("{} is a reserved keyword", Token::Kw(word)), help: Some(help.into()) })
    }
}

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
        any().filter(|t: &Token| matches!(t, Token::Punct(Punct::LParen | Punct::LBracket))).rewind().ignore_then(skip_group()),
        any().filter(|t: &Token| !is_closer(t) && !matches!(t, Token::Punct(Punct::Semi | Punct::LBrace))).ignored(),
    ))
    .repeated()
    .ignored()
    .boxed()
}

/// `pub`, or `pub(crate)`, `pub(super)`, `pub(self)`, `pub(in path)`: the span
/// of `pub`. The restricted forms are removed (B.3.6) and reported once.
pub(crate) fn visibility<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
    let restriction = any()
        .filter(|t: &Token| matches!(t, Token::Kw(Kw::Crate | Kw::Super | Kw::SelfLower | Kw::In)))
        .then(any().filter(|t: &Token| !matches!(t, Token::Punct(Punct::RParen))).repeated())
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen))
        .map_with(|_, e| e.span());
    kw(Kw::Pub).map_with(|_, e| e.span()).then(restriction.or_not()).validate(|(at, restricted): (Span, Option<Span>), _, emitter| {
        if let Some(group) = restricted {
            emitter.emit(removed("restricted visibility", Span::new(at.start, group.end)));
        }
        at
    })
}

/// Whether `e` is a `place` (B.3.3): a carrier path followed by `[expr]` and
/// `.name` steps. A parenthesized place is not one. The one decision for the
/// definition and the commit alike.
pub(crate) fn is_place(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(_) => true,
        ExprKind::Index { expr, .. } => is_place(expr),
        // `.IDENT` only: a tuple index (`q.0`) names no carrier member.
        ExprKind::Field { expr, name } => !name.name.starts_with(|c: char| c.is_ascii_digit()) && is_place(expr),
        _ => false,
    }
}

/// The diagnostic of a left side that is not a `place`.
pub(crate) fn not_a_place() -> Custom {
    Custom {
        code: "E0900",
        message: "expected a place: a carrier, `x[i]` or `x.f`".into(),
        help: Some("a place takes no parentheses and no operators: write `q`, `q[i]` or `q.f`".into()),
    }
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

/// Any literal: integer, float, string, `true`/`false`.
pub(crate) fn lit<'a, I: In<'a>>() -> impl Parser<'a, I, Lit, Extra<'a>> + Clone {
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

pub(crate) fn lit_expr<'a, I: In<'a>>() -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
    lit().map(|l| Expr { span: l.span, kind: ExprKind::Lit(l) })
}


fn delim_open(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace))
}

fn delim_close(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace))
}

/// A balanced `( .. )`, `[ .. ]` or `{ .. }` group, contents skipped. An
/// opener closed by another kind of delimiter (or never) is no group.
pub(crate) fn skip_group<'a, I: In<'a>>() -> P<'a, I, ()> {
    recursive(|nested| {
        let inner = choice((
            nested,
            any().filter(|t: &Token| !delim_open(t) && !delim_close(t)).ignored(),
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
    // `Name<..>`: a generic list is skipped to its `>`, item keywords inside
    // it (`const N: u32`) are no place to resume.
    let angle_list = select! { Token::Ident(_) => () }
        .then(just(Token::Punct(Punct::Lt)))
        .ignore_then(
            any()
                .filter(|t: &Token| !is_closer(t) && !matches!(t, Token::Punct(Punct::Gt | Punct::Semi | Punct::LBrace)))
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
    // Only the `}` that ends the block stops the skip; a `)` or `]` that no
    // group of the statement closes is skipped with it.
    let more = choice((
        any().filter(delim_open).rewind().ignore_then(skip_group()),
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
