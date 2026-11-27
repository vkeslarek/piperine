//! Shared parser plumbing: error types, the recursive handles, token helpers.

use chumsky::extra;
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use chumsky::recursive::{Indirect, Recursive};

use crate::msl::ast::*;
use crate::msl::diag::Diagnostic;
use crate::msl::lexer::{Kw, Token};
use crate::msl::span::Span;

use super::{ExpressionGrammar, ItemGrammar, PatternGrammar, TypeGrammar};

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

impl std::fmt::Display for Custom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub(crate) type Parsed<O> = (Option<O>, Vec<Diagnostic>);
pub(crate) type Err<'a> = Rich<'a, Token, Span, Custom>;
pub(crate) type Extra<'a> = extra::Err<Err<'a>>;
pub(crate) type P<'a, I, O> = Boxed<'a, 'a, I, O, Extra<'a>>;
pub(crate) type Rec<'a, I, O> = Recursive<Indirect<'a, 'a, I, O, Extra<'a>>>;

/// An input of spanned MSL tokens.
///
/// Kept as a trait alias: stable Rust has no `trait X = ..`, and this bound is written on every
/// grammar signature (113 of them); the blanket impl is the alias, so there is one implementor-less
/// name for `ValueInput<'a, Token = Token, Span = Span>`.
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

impl<'a, I: In<'a>> Handles<'a, I> {
    /// The mutually recursive parsers, declared and then defined once each.
    pub(crate) fn declare() -> Handles<'a, I> {
        let mut h = Handles {
            ty: Recursive::declare(),
            pat: Recursive::declare(),
            pat1: Recursive::declare(),
            expr: Recursive::declare(),
            expr_ns: Recursive::declare(),
            block: Recursive::declare(),
            item: Recursive::declare(),
        };
        let p = TypeGrammar::definition(&h);
        h.ty.define(p);
        let (pat, pat1) = PatternGrammar::definition(&h);
        h.pat.define(pat);
        h.pat1.define(pat1);
        let p = ExpressionGrammar::expression(&h, true, h.expr.clone());
        h.expr.define(p);
        let p = ExpressionGrammar::expression(&h, false, h.expr_ns.clone());
        h.expr_ns.define(p);
        let p = ExpressionGrammar::block(&h);
        h.block.define(p);
        let p = ItemGrammar::definition(&h);
        h.item.define(p);
        h
    }
}
