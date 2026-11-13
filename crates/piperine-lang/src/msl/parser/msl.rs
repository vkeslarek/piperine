//! The language's own productions (Appendix B.3): natures, signals, modules,
//! behavior blocks. Each is a function over the shared [`Handles`], used by
//! [`super::item::build`] for the item level.
//!
//! `potential`, `flow`, `value`, `resolve`, `analog` and `digital` are
//! contextual keywords: plain identifiers that mean something only at the
//! position a production names, so [`word`] matches them by text.

use super::*;

/// A contextual keyword: the identifier `word`.
pub(crate) fn word<'a, I: In<'a>>(word: &'static str) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
    select! { Token::Ident(s) if s == word => () }
}

// ---- natures ----------------------------------------------------------------------------------

/// `nature Name<G> { potential: V { access: v }, flow: I, value: T, resolve: f }`
pub(crate) fn nature_decl<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ItemKind> {
    let attr = ident()
        .then_ignore(punct(Punct::Colon))
        .then(h.expr.clone())
        .map_with(|(name, value), e| FieldInit { name, value, span: Loc(e.span()) });
    let attr_block = attr
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace));
    let quantity = choice((
        word("potential").to(Quantity::Potential),
        word("flow").to(Quantity::Flow),
    ))
    .then_ignore(punct(Punct::Colon))
    .then(ident())
    .then(attr_block.or_not())
    .map(|((which, name), attrs)| NatureFieldKind::Quantity {
        which,
        name,
        attrs: attrs.unwrap_or_default(),
    });
    let value = word("value")
        .ignore_then(punct(Punct::Colon))
        .ignore_then(h.ty.clone())
        .map(NatureFieldKind::Value);
    let resolve = word("resolve")
        .ignore_then(punct(Punct::Colon))
        .ignore_then(h.expr.clone())
        .map(NatureFieldKind::Resolve);
    let field = first_of(vec![quantity.boxed(), value.boxed(), resolve.boxed()])
        .map_with(|kind, e| NatureField { kind, span: Loc(e.span()) })
        .labelled("nature field");
    kw(Kw::Nature)
        .ignore_then(ident())
        .then(ty::generics(h).or_not())
        .then(
            field
                .separated_by(punct(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace)),
        )
        .map(|((name, generics), fields)| ItemKind::Nature(NatureDecl { name, generics, fields }))
        .boxed()
}

// ---- module items -----------------------------------------------------------------------------

/// `signal name: sigtype = init;`
fn signal_decl<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ModuleItemKind> {
    kw(Kw::Signal)
        .ignore_then(ident())
        .then_ignore(punct(Punct::Colon))
        .then(h.ty.clone())
        .then(punct(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .then_ignore(punct(Punct::Semi))
        .map(|((name, ty), init)| ModuleItemKind::Signal { name, ty, init })
        .boxed()
}

/// One item of a module body.
pub(crate) fn module_item<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ModuleItem> {
    let declared = first_of(vec![signal_decl(h)]);
    item::meta(h)
        .then(kw(Kw::Pub).or_not())
        .then(declared)
        .map_with(|((meta, public), kind), e| ModuleItem {
            meta,
            public: public.is_some(),
            kind,
            span: Loc(e.span()),
        })
        .labelled("module item")
        .boxed()
}
