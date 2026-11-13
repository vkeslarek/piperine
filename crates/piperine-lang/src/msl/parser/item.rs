//! Rust items (`fn`, `struct`, `enum`, `impl`, `trait`, `const`, `use`,
//! `mod`), attributes and doc comments.

use super::*;

/// Doc comments and attributes in front of a declaration.
pub(crate) fn meta<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Meta> {
    #[derive(Clone)]
    enum One {
        Doc(Doc),
        Attr(Attr),
    }
    let doc = select! { Token::DocOuter(text) => text }
        .map_with(|text, e| One::Doc(Doc { text, span: Loc(e.span()) }));

    let arg = choice((
        lit().map(AttrArg::Lit),
        ident().then_ignore(punct(Punct::Eq)).then(lit()).map(|(n, l)| AttrArg::NameValue(n, l)),
        ty::path(h, false).map(AttrArg::Path),
    ));
    let input = choice((
        punct(Punct::Eq).ignore_then(h.expr.clone()).map(AttrInput::Eq),
        arg.separated_by(punct(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(punct(Punct::LParen), punct(Punct::RParen))
            .map(AttrInput::List),
        empty().to(AttrInput::None),
    ));
    let attr = punct(Punct::Pound)
        .ignore_then(
            ty::path(h, false)
                .then(input)
                .delimited_by(punct(Punct::LBracket), punct(Punct::RBracket)),
        )
        .map_with(|(path, input), e| One::Attr(Attr { path, input, span: Loc(e.span()) }));

    choice((doc, attr))
        .repeated()
        .collect::<Vec<_>>()
        .map(|all| {
            let mut m = Meta::default();
            for one in all {
                match one {
                    One::Doc(d) => m.docs.push(d),
                    One::Attr(a) => m.attrs.push(a),
                }
            }
            m
        })
        .boxed()
}

fn dir<'a, I: In<'a>>() -> impl Parser<'a, I, Dir, Extra<'a>> + Clone {
    choice((kw(Kw::In).to(Dir::In), kw(Kw::Out).to(Dir::Out)))
}

/// `fn name<G>(params) -> T where cond { body }` or `... ;`, with the
/// optional `const` and `extern "C"` in front.
pub(crate) fn fn_decl<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, FnDecl> {
    let param = choice((
        kw(Kw::SelfLower).map_with(|_, e| FnParam::SelfValue(Loc(e.span()))),
        ident()
            .then_ignore(punct(Punct::Colon))
            .then(h.ty.clone())
            .map_with(|(name, ty), e| FnParam::Named { name, ty, span: Loc(e.span()) }),
    ));
    kw(Kw::Const)
        .or_not()
        .then(kw(Kw::Extern).ignore_then(string_lit()).or_not())
        .then_ignore(kw(Kw::Fn))
        .then(ident())
        .then(ty::generics(h).or_not())
        .then(
            param
                .separated_by(punct(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LParen), punct(Punct::RParen)),
        )
        .then(punct(Punct::Arrow).ignore_then(h.ty.clone()).or_not())
        .then(kw(Kw::Where).ignore_then(h.expr_ns.clone()).or_not())
        .then(choice((h.block.clone().map(Some), punct(Punct::Semi).to(None))))
        .map(|(((((((is_const, abi), name), generics), params), ret), where_), body)| FnDecl {
            is_const: is_const.is_some(),
            abi,
            name,
            generics,
            params,
            ret,
            where_,
            body,
        })
        .boxed()
}

/// `const NAME: ty = value;` (the value is optional in a trait).
fn const_item<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ConstItem> {
    kw(Kw::Const)
        .ignore_then(ident())
        .then_ignore(punct(Punct::Colon))
        .then(h.ty.clone())
        .then(punct(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .then_ignore(punct(Punct::Semi))
        .map(|((name, ty), value)| ConstItem { name, ty, value })
        .boxed()
}

fn impl_items<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Vec<ImplItem>> {
    let item = meta(h)
        .then(kw(Kw::Pub).or_not())
        .then(choice((
            fn_decl(h).map(ImplItemKind::Fn),
            const_item(h).map(ImplItemKind::Const),
        )))
        .map_with(|((meta, public), kind), e| ImplItem {
            meta,
            public: public.is_some(),
            kind,
            span: Loc(e.span()),
        });
    item.repeated()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace))
        .boxed()
}

/// `{ fields }`, `( fields );` or `;`.
fn fields<'a, I: In<'a>>(h: &Handles<'a, I>, unit_semi: bool) -> P<'a, I, Fields> {
    let named_field = meta(h)
        .then(kw(Kw::Pub).or_not())
        .then(dir().or_not())
        .then(ident())
        .then_ignore(punct(Punct::Colon))
        .then(h.ty.clone())
        .map_with(|((((meta, public), dir), name), ty), e| Field {
            meta,
            public: public.is_some(),
            dir,
            name: Some(name),
            ty,
            span: Loc(e.span()),
        });
    let named = named_field
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace))
        .map(Fields::Named);
    let tuple_field = meta(h)
        .then(kw(Kw::Pub).or_not())
        .then(h.ty.clone())
        .map_with(|((meta, public), ty), e| Field {
            meta,
            public: public.is_some(),
            dir: None,
            name: None,
            ty,
            span: Loc(e.span()),
        });
    let tuple = tuple_field
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen))
        .map(Fields::Tuple);
    if unit_semi {
        choice((named, tuple.then_ignore(punct(Punct::Semi)), punct(Punct::Semi).to(Fields::Unit))).boxed()
    } else {
        choice((named, tuple, empty().to(Fields::Unit))).boxed()
    }
}

/// `a::b::{c, d as e, *}`.
fn use_tree<'a, I: In<'a>>() -> P<'a, I, UseTree> {
    recursive(|tree| {
        let prefix = path_ident().then_ignore(punct(Punct::ColonColon)).repeated().collect::<Vec<_>>();
        let group = tree
            .separated_by(punct(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace));
        let alias = kw(Kw::As).ignore_then(ident()).or_not();
        let end = choice((
            punct(Punct::Star).to(None),
            group.map(|g| Some(Err(g))),
            path_ident().then(alias).map(|(last, alias)| Some(Ok((last, alias)))),
        ));
        prefix.then(end).map_with(|(mut path, end), e| {
            let kind = match end {
                None => UseKind::Glob,
                Some(Err(group)) => UseKind::Group(group),
                Some(Ok((last, alias))) => {
                    path.push(last);
                    UseKind::Simple { alias }
                }
            };
            UseTree { path, kind, span: Loc(e.span()) }
        })
    })
    .boxed()
}

/// The item parser: Rust items (natures, modules and behavior blocks join
/// it with their own tasks).
pub(crate) fn build<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Item> {
    let struct_decl = kw(Kw::Struct)
        .ignore_then(ident())
        .then(ty::generics(h).or_not())
        .then(fields(h, true))
        .map(|((name, generics), fields)| ItemKind::Struct(StructDecl { name, generics, fields }));

    let variant = meta(h)
        .then(ident())
        .then(fields(h, false))
        .then(punct(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .map_with(|(((meta, name), fields), discriminant), e| Variant {
            meta,
            name,
            fields,
            discriminant,
            span: Loc(e.span()),
        });
    let enum_decl = kw(Kw::Enum)
        .ignore_then(ident())
        .then(ty::generics(h).or_not())
        .then(
            variant
                .separated_by(punct(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace)),
        )
        .map(|((name, generics), variants)| ItemKind::Enum(EnumDecl { name, generics, variants }));

    let impl_block = kw(Kw::Impl)
        .ignore_then(ty::generics(h).or_not())
        .then(h.ty.clone())
        .then(kw(Kw::For).ignore_then(h.ty.clone()).or_not())
        .then(impl_items(h))
        .try_map(|(((generics, first), second), items), span| match second {
            None => Ok(ItemKind::Impl(ImplBlock { generics, trait_: None, self_ty: first, items })),
            Some(self_ty) => match first.kind {
                TypeKind::Path(trait_) => {
                    Ok(ItemKind::Impl(ImplBlock { generics, trait_: Some(trait_), self_ty, items }))
                }
                _ => Err(Rich::custom(
                    span,
                    Custom { code: "E0900", message: "the trait of an `impl` must be a path".into(), help: None },
                )),
            },
        });

    let trait_decl = kw(Kw::Trait)
        .ignore_then(ident())
        .then(ty::generics(h).or_not())
        .then(
            punct(Punct::Colon)
                .ignore_then(ty::path(h, false).separated_by(punct(Punct::Plus)).at_least(1).collect::<Vec<_>>())
                .or_not(),
        )
        .then(impl_items(h))
        .map(|(((name, generics), supertraits), items)| {
            ItemKind::Trait(TraitDecl { name, generics, supertraits: supertraits.unwrap_or_default(), items })
        });

    let use_decl = kw(Kw::Use).ignore_then(use_tree()).then_ignore(punct(Punct::Semi)).map(ItemKind::Use);

    let mod_decl = kw(Kw::Mod)
        .ignore_then(ident())
        .then(choice((
            punct(Punct::Semi).to(None),
            h.item
                .clone()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace))
                .map(Some),
        )))
        .map(|(name, items)| ItemKind::Mod(ModItem { name, items }));

    let macro_rules = select! { Token::Ident(s) if s == "macro_rules" => () }
        .ignore_then(punct(Punct::Not))
        .ignore_then(ident().or_not())
        .ignore_then(skip_group())
        .validate(|_, e, emitter| {
            emitter.emit(macro_error(e.span()));
            ItemKind::Error
        });

    let kind = first_of(vec![
        fn_decl(h).map(ItemKind::Fn).boxed(),
        const_item(h).map(|c| ItemKind::Const(c)).boxed(),
        struct_decl.boxed(),
        enum_decl.boxed(),
        impl_block.boxed(),
        trait_decl.boxed(),
        use_decl.boxed(),
        mod_decl.boxed(),
        msl::nature_decl(h),
        macro_rules.boxed(),
    ]);
    meta(h)
        .then(kw(Kw::Pub).or_not())
        .then(kind)
        .map_with(|((meta, public), kind), e| Item {
            meta,
            public: public.is_some(),
            kind,
            span: Loc(e.span()),
        })
        .labelled("item")
        .boxed()
}
