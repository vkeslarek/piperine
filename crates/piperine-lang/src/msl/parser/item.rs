//! Rust items (`fn`, `struct`, `enum`, `impl`, `trait`, `const`, `use`,
//! `mod`), attributes and doc comments.

use super::*;

/// Doc comments and attributes in front of a declaration.
pub(crate) fn meta<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Meta> {
    #[derive(Clone)]
    enum One {
        Doc(Doc),
        Attr(Attr),
        /// An inner attribute `#![..]`: removed, reported, dropped.
        Inner,
    }
    let doc = select! { Token::DocOuter(text) => (text, false), Token::DocOuterBlock(text) => (text, true) }
        .map_with(|(text, block), e| One::Doc(Doc { text, block, span: Loc(e.span()) }));

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

    let inner = punct(Punct::Pound)
        .ignore_then(punct(Punct::Not))
        .ignore_then(skip_group())
        .validate(|_, e, emitter| {
            emitter.emit(removed("inner attributes", e.span()));
            One::Inner
        });
    choice((doc, inner, attr))
        .repeated()
        .collect::<Vec<_>>()
        .map(|all| {
            let mut m = Meta::default();
            for one in all {
                match one {
                    One::Doc(d) => m.docs.push(d),
                    One::Attr(a) => m.attrs.push(a),
                    One::Inner => {}
                }
            }
            m
        })
        .boxed()
}

/// Docs and attributes where Rust takes an outer attribute but the language
/// does not (a parameter, a generic parameter, a struct-literal field): one
/// `E0900` (B.3.6), the comments and attributes dropped.
pub(crate) fn rejected_meta<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ()> {
    meta(h)
        .validate(|m, e, emitter| {
            if !m.docs.is_empty() || !m.attrs.is_empty() {
                emitter.emit(removed("attributes and doc comments on a parameter", e.span()));
            }
        })
        .boxed()
}

/// `where ..` on a struct, enum, trait or impl: every `where` there is the
/// removed trait-bound form (B.3.6); reported at `where`, the clause skipped.
fn removed_where<'a, I: In<'a>>() -> P<'a, I, ()> {
    kw(Kw::Where)
        .map_with(|_, e| e.span())
        .then_ignore(skip_head_rest())
        .validate(|at, _, emitter| emitter.emit(removed("trait-bound `where` clauses", at)))
        .or_not()
        .ignored()
        .boxed()
}

pub(crate) fn dir<'a, I: In<'a>>() -> impl Parser<'a, I, Dir, Extra<'a>> + Clone {
    choice((kw(Kw::In).to(Dir::In), kw(Kw::Out).to(Dir::Out)))
}

/// `fn name<G>(params) -> T where cond { body }` or `... ;`, with the
/// optional `const` and `extern "C"` in front.
pub(crate) fn fn_decl<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, FnDecl> {
    // E0143: a signature has no default value and no variadic parameter.
    let signature_error = |span: Span, what: &str| {
        Rich::custom(
            span,
            Custom {
                code: "E0143",
                message: format!("{what} in a function signature"),
                help: Some("a function takes a fixed list of `name: type` parameters; model optional values with `Option<T>`".into()),
            },
        )
    };
    let default_value = punct(Punct::Eq).ignore_then(h.expr.clone()).map_with(|_, e| e.span());
    // `&self` and `&mut self` receivers are removed: reported, kept as an error parameter.
    let ref_receiver = punct(Punct::And)
        .ignore_then(kw(Kw::Mut).or_not())
        .ignore_then(kw(Kw::SelfLower))
        .validate(|_, e, emitter| {
            emitter.emit(removed("`&self` and `&mut self` receivers", e.span()));
            FnParam::Error(Loc(e.span()))
        });
    // `mut x: T` and `(a, b): T`: a parameter is a plain name.
    let pattern_param = any()
        .filter(|t: &Token| {
            matches!(
                t,
                Token::Kw(Kw::Mut | Kw::Ref) | Token::Punct(Punct::LParen | Punct::LBracket | Punct::And | Punct::Underscore)
            )
        })
        .rewind()
        .ignore_then(h.pat1.clone())
        .then_ignore(punct(Punct::Colon))
        .then_ignore(h.ty.clone())
        .validate(|_, e, emitter| {
            emitter.emit(removed("`mut` and pattern parameters", e.span()));
            FnParam::Error(Loc(e.span()))
        });
    let param = rejected_meta(h).ignore_then(first_of(vec![
        kw(Kw::SelfLower).map_with(|_, e| FnParam::SelfValue(Loc(e.span()))).boxed(),
        ref_receiver.boxed(),
        ident()
            .then_ignore(punct(Punct::Colon))
            .then(h.ty.clone())
            .then(default_value.or_not())
            .validate(move |((name, ty), default), e, emitter| {
                if let Some(span) = default {
                    emitter.emit(signature_error(span, "a default value"));
                }
                FnParam::Named { name, ty, span: Loc(e.span()) }
            })
            .boxed(),
        punct(Punct::DotDotDot)
            .validate(move |_, e, emitter| {
                emitter.emit(signature_error(e.span(), "a variadic parameter"));
                FnParam::Error(Loc(e.span()))
            })
            .boxed(),
        pattern_param.boxed(),
    ]));
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
        .then(where_clause(h))
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

/// `where cond` after a head; a trait-bound clause (`where T: Tr`) is removed
/// (B.3.6): reported at `where`, the bounds skipped, `T` kept as the condition.
pub(crate) fn where_clause<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Option<Expr>> {
    kw(Kw::Where)
        .map_with(|_, e| e.span())
        .then(h.expr_ns.clone())
        .then(punct(Punct::Colon).ignore_then(skip_to_end()).or_not())
        // `where a, b`: one condition only. The comma is the fault; the rest
        // of the head is skipped up to its `{` or `;`, so the item survives.
        .then(
            punct(Punct::Comma)
                .map_with(|_, e| e.span())
                .then_ignore(skip_head_rest())
                .or_not(),
        )
        .validate(|(((at, cond), bounds), comma), _, emitter| {
            if bounds.is_some() {
                emitter.emit(removed("trait-bound `where` clauses", at));
            }
            if let Some(span) = comma {
                emitter.emit(Rich::custom(
                    span,
                    Custom {
                        code: "E0900",
                        message: "a `where` clause takes one condition".into(),
                        help: Some("join the conditions with `&&`".into()),
                    },
                ));
            }
            cond
        })
        .or_not()
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
        .then(visibility().or_not())
        .then(choice((
            fn_decl(h).map(ImplItemKind::Fn),
            const_item(h).map(ImplItemKind::Const),
        )))
        .map_with(|((meta, public), kind), e| {
            Some(ImplItem { meta, public: public.is_some(), kind, span: Loc(e.span()) })
        });
    // `type X;` and `type X = T;` are removed (B.3.6): reported, dropped.
    let associated_type = meta(h)
        .then(visibility().or_not())
        .ignore_then(kw(Kw::Type).map_with(|_, e| e.span()))
        .then_ignore(skip_to_end())
        .then_ignore(punct(Punct::Semi).or_not())
        .validate(|at, _, emitter| {
            emitter.emit(removed("associated types", at));
            None
        });
    first_of(vec![item.boxed(), associated_type.boxed()])
        .repeated()
        .collect::<Vec<_>>()
        .map(|items| items.into_iter().flatten().collect())
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace))
        .boxed()
}

/// `{ fields }`, `( fields );` or `;`.
fn fields<'a, I: In<'a>>(h: &Handles<'a, I>, unit_semi: bool) -> P<'a, I, Fields> {
    let named_field = meta(h)
        .then(visibility().or_not())
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
        .then(visibility().or_not())
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
        .then_ignore(removed_where())
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
        .then_ignore(removed_where())
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
        .then_ignore(removed_where())
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
        .then_ignore(removed_where())
        .then(impl_items(h))
        .map(|(((name, generics), supertraits), items)| {
            ItemKind::Trait(TraitDecl { name, generics, supertraits: supertraits.unwrap_or_default(), items })
        });

    // A leading `::` is removed (B.3.6): reported at the `::`, the tree kept.
    let use_decl = kw(Kw::Use)
        .ignore_then(punct(Punct::ColonColon).map_with(|_, e| e.span()).or_not())
        .then(use_tree())
        .then_ignore(punct(Punct::Semi))
        .validate(|(leading, tree), _, emitter| {
            if let Some(at) = leading {
                emitter.emit(removed("a leading `::`", at));
            }
            ItemKind::Use(tree)
        });

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

    // `type`, `static`, `union` and `extern` blocks are removed (B.3.6): each is
    // reported once at its keyword, its rest skipped, and the item kept as an error.
    let skipped = |first: P<'a, I, Span>, row: &'static str| {
        first
            .then_ignore(skip_to_end())
            .then_ignore(punct(Punct::Semi).or_not())
            .validate(move |at, _, emitter| {
                emitter.emit(removed(row, at));
                ItemKind::Error
            })
            .boxed()
    };
    let type_alias = skipped(kw(Kw::Type).map_with(|_, e| e.span()).boxed(), "type aliases");
    let static_item = skipped(kw(Kw::Static).map_with(|_, e| e.span()).boxed(), "`static` items");
    let union_item = select! { Token::Ident(s) if s == "union" => () }
        .map_with(|_, e| e.span())
        .then_ignore(ident())
        .then_ignore(ty::generics(h).or_not())
        .then_ignore(skip_group())
        .validate(|at, _, emitter| {
            emitter.emit(removed("`union` items", at));
            ItemKind::Error
        })
        .boxed();
    let extern_crate = skipped(
        kw(Kw::Extern).map_with(|_, e| e.span()).then_ignore(kw(Kw::Crate)).boxed(),
        "`extern crate` items",
    );
    let extern_block = kw(Kw::Extern)
        .map_with(|_, e| e.span())
        .then_ignore(string_lit().or_not())
        .then_ignore(skip_group())
        .validate(|at, _, emitter| {
            emitter.emit(removed("`extern` blocks", at));
            ItemKind::Error
        })
        .boxed();

    // An `emit!` statement at the file's top: `E0601`, as in a module body.
    let stray_emit = select! { Token::Ident(s) if s == "emit" => () }
        .then(punct(Punct::Not))
        .rewind()
        .ignore_then(expr::stmt(h))
        .validate(|statement, _, emitter| {
            emitter.emit(msl::misplaced_emit(statement.span.0));
            ItemKind::Error
        })
        .boxed();
    let kind = first_of(vec![
        stray_emit,
        fn_decl(h).map(ItemKind::Fn).boxed(),
        const_item(h).map(|c| ItemKind::Const(c)).boxed(),
        struct_decl.boxed(),
        enum_decl.boxed(),
        impl_block.boxed(),
        trait_decl.boxed(),
        use_decl.boxed(),
        mod_decl.boxed(),
        msl::nature_decl(h),
        msl::module_decl(h),
        msl::behavior_block(h),
        macro_rules.boxed(),
        type_alias,
        static_item,
        union_item,
        extern_crate,
        extern_block,
    ]);
    let item = meta(h)
        .then(visibility().or_not())
        .then(kind)
        .validate(|((meta, public), kind), e, emitter| {
            // A behavior block has no visibility: the block is kept, `pub` reported.
            if let (Some(at), ItemKind::Behavior(_)) = (public, &kind) {
                emitter.emit(Rich::custom(
                    at,
                    Custom {
                        code: "E0900",
                        message: "`pub` before a behavior block".into(),
                        help: Some("a behavior block has no visibility: remove `pub`".into()),
                    },
                ));
            }
            Item { meta, public: public.is_some(), kind, span: Loc(e.span()) }
        })
        .labelled("item")
        .boxed();
    recover(item, true, |span| Item { meta: Meta::default(), public: false, kind: ItemKind::Error, span: Loc(span) })
}
