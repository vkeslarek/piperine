//! Rust items (`fn`, `struct`, `enum`, `impl`, `trait`, `const`, `use`,
//! `mod`), attributes and doc comments.

use super::*;

/// The item grammar: attributes, doc comments, functions, structs, enums, impls, traits, constants and `use` trees.
pub struct ItemGrammar;

impl ItemGrammar {
    /// Doc comments and attributes in front of a declaration.
    pub(crate) fn meta<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Meta> {
        /// One entry in front of a declaration.
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
            TokenInput::literal().map(AttrArg::Lit),
            TokenInput::identifier().then_ignore(TokenInput::punctuation(Punct::Eq)).then(TokenInput::literal()).map(|(n, l)| AttrArg::NameValue(n, l)),
            TypeGrammar::path(h, false).map(AttrArg::Path),
        ));
        let input = choice((
            TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).map(AttrInput::Eq),
            arg.separated_by(TokenInput::punctuation(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
                .map(AttrInput::List),
            empty().to(AttrInput::None),
        ));
        let attr = TokenInput::punctuation(Punct::Pound)
            .ignore_then(
                TypeGrammar::path(h, false)
                    .then(input)
                    .delimited_by(TokenInput::punctuation(Punct::LBracket), TokenInput::punctuation(Punct::RBracket)),
            )
            .map_with(|(path, input), e| One::Attr(Attr { path, input, span: Loc(e.span()) }));

        let inner = TokenInput::punctuation(Punct::Pound)
            .ignore_then(TokenInput::punctuation(Punct::Not))
            .ignore_then(Recovery::skip_group())
            .validate(|_, e, emitter| {
                emitter.emit(ParseDiagnostics::removed("inner attributes", e.span()));
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
        ItemGrammar::meta(h)
            .validate(|m, e, emitter| {
                if !m.docs.is_empty() || !m.attrs.is_empty() {
                    emitter.emit(ParseDiagnostics::removed("attributes and doc comments on a parameter", e.span()));
                }
            })
            .boxed()
    }

    /// `where ..` on a struct, enum, trait or impl: every `where` there is the
    /// removed trait-bound form (B.3.6); reported at `where`, the clause skipped.
    fn removed_where<'a, I: In<'a>>() -> P<'a, I, ()> {
        TokenInput::keyword(Kw::Where)
            .map_with(|_, e| e.span())
            .then_ignore(Recovery::skip_head_rest())
            .validate(|at, _, emitter| emitter.emit(ParseDiagnostics::removed("trait-bound `where` clauses", at)))
            .or_not()
            .ignored()
            .boxed()
    }

    pub(crate) fn direction<'a, I: In<'a>>() -> impl Parser<'a, I, Dir, Extra<'a>> + Clone {
        choice((TokenInput::keyword(Kw::In).to(Dir::In), TokenInput::keyword(Kw::Out).to(Dir::Out)))
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
        let default_value = TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).map_with(|_, e| e.span());
        // `&self` and `&mut self` receivers are removed (B.3.6): reported, read as `self`.
        let ref_receiver = TokenInput::reference_head().then_ignore(TokenInput::keyword(Kw::SelfLower)).validate(|_, e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                Custom {
                    code: "E0101",
                    message: "references and borrows are not part of the language".into(),
                    help: Some("write `self`: a method takes `self` and returns what it changed".into()),
                },
            ));
            FnParam::SelfValue(Loc(e.span()))
        });
        // `mut x: T`: the marker is a reserved word (`E0903`), the parameter is read as a plain one.
        let marked_param = choice((TokenInput::keyword(Kw::Mut).to(Kw::Mut), TokenInput::keyword(Kw::Ref).to(Kw::Ref)))
            .map_with(|word, e| (word, e.span()))
            .then(TokenInput::identifier())
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then(h.ty.clone())
            .validate(|(((word, at), name), ty), e, emitter| {
                emitter.emit(ReservedMarker::report(word, at));
                FnParam::Named { name, ty, span: Loc(e.span()) }
            });
        // `(a, b): T`: a parameter is a plain name.
        let pattern_param = any()
            .filter(|t: &Token| {
                matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::And | Punct::Underscore))
            })
            .rewind()
            .ignore_then(h.pat1.clone())
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then_ignore(h.ty.clone())
            .validate(|_, e, emitter| {
                emitter.emit(ParseDiagnostics::removed("pattern parameters", e.span()));
                FnParam::Error(Loc(e.span()))
            });
        let param = ItemGrammar::rejected_meta(h).ignore_then(TokenInput::first_of(vec![
            TokenInput::keyword(Kw::SelfLower).map_with(|_, e| FnParam::SelfValue(Loc(e.span()))).boxed(),
            ref_receiver.boxed(),
            TokenInput::identifier()
                .then_ignore(TokenInput::punctuation(Punct::Colon))
                .then(h.ty.clone())
                .then(default_value.or_not())
                .validate(move |((name, ty), default), e, emitter| {
                    if let Some(span) = default {
                        emitter.emit(signature_error(span, "a default value"));
                    }
                    FnParam::Named { name, ty, span: Loc(e.span()) }
                })
                .boxed(),
            marked_param.boxed(),
            TokenInput::punctuation(Punct::DotDotDot)
                .validate(move |_, e, emitter| {
                    emitter.emit(signature_error(e.span(), "a variadic parameter"));
                    FnParam::Error(Loc(e.span()))
                })
                .boxed(),
            pattern_param.boxed(),
        ]));
        TokenInput::keyword(Kw::Const)
            .or_not()
            .then(TokenInput::keyword(Kw::Extern).ignore_then(TokenInput::string_literal()).or_not())
            .then_ignore(TokenInput::keyword(Kw::Fn))
            .then(TokenInput::identifier())
            .then(GenericsGrammar::parameters(h).or_not())
            .then(
                param
                    .separated_by(TokenInput::punctuation(Punct::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen)),
            )
            .then(TokenInput::punctuation(Punct::Arrow).ignore_then(h.ty.clone()).or_not())
            .then(ItemGrammar::where_clause(h))
            .then(choice((h.block.clone().map(Some), TokenInput::punctuation(Punct::Semi).to(None))))
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
        TokenInput::keyword(Kw::Where)
            .map_with(|_, e| e.span())
            .then(h.expr_ns.clone())
            .then(TokenInput::punctuation(Punct::Colon).ignore_then(Recovery::skip_to_end()).or_not())
            // `where a, b`: one condition only. The comma is the fault; the rest
            // of the head is skipped up to its `{` or `;`, so the item survives.
            .then(
                TokenInput::punctuation(Punct::Comma)
                    .map_with(|_, e| e.span())
                    .then_ignore(Recovery::skip_head_rest())
                    .or_not(),
            )
            .validate(|(((at, cond), bounds), comma), _, emitter| {
                if bounds.is_some() {
                    emitter.emit(ParseDiagnostics::removed("trait-bound `where` clauses", at));
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
        TokenInput::keyword(Kw::Const)
            .ignore_then(TokenInput::identifier())
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then(h.ty.clone())
            .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).or_not())
            .then_ignore(TokenInput::punctuation(Punct::Semi))
            .map(|((name, ty), value)| ConstItem { name, ty, value })
            .boxed()
    }

    fn impl_items<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Vec<ImplItem>> {
        let item = ItemGrammar::meta(h)
            .then(TokenInput::visibility().or_not())
            .then(choice((
                ItemGrammar::fn_decl(h).map(ImplItemKind::Fn),
                ItemGrammar::const_item(h).map(ImplItemKind::Const),
            )))
            .map_with(|((meta, public), kind), e| {
                Some(ImplItem { meta, public: public.is_some(), kind, span: Loc(e.span()) })
            });
        // `type X;` and `type X = T;` are removed (B.3.6): reported, dropped.
        let associated_type = ItemGrammar::meta(h)
            .then(TokenInput::visibility().or_not())
            .ignore_then(TokenInput::keyword(Kw::Type).map_with(|_, e| e.span()))
            .then_ignore(Recovery::skip_to_end())
            .then_ignore(TokenInput::punctuation(Punct::Semi).or_not())
            .validate(|at, _, emitter| {
                emitter.emit(ParseDiagnostics::removed("associated types", at));
                None
            });
        TokenInput::first_of(vec![item.boxed(), associated_type.boxed()])
            .repeated()
            .collect::<Vec<_>>()
            .map(|items| items.into_iter().flatten().collect())
            .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace))
            .boxed()
    }

    /// `{ fields }`, `( fields );` or `;`.
    fn fields<'a, I: In<'a>>(h: &Handles<'a, I>, unit_semi: bool) -> P<'a, I, Fields> {
        let named_field = ItemGrammar::meta(h)
            .then(TokenInput::visibility().or_not())
            .then(ItemGrammar::direction().or_not())
            .then(TokenInput::identifier())
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then(h.ty.clone())
            .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).or_not())
            .map_with(|(((((meta, public), dir), name), ty), init), e| Field {
                meta,
                public: public.is_some(),
                dir,
                name: Some(name),
                ty,
                init,
                span: Loc(e.span()),
            });
        let named = named_field
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace))
            .map(Fields::Named);
        let tuple_field = ItemGrammar::meta(h)
            .then(TokenInput::visibility().or_not())
            .then(h.ty.clone())
            .map_with(|((meta, public), ty), e| Field {
                meta,
                public: public.is_some(),
                dir: None,
                name: None,
                ty,
                init: None,
                span: Loc(e.span()),
            });
        let tuple = tuple_field
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
            .map(Fields::Tuple);
        if unit_semi {
            choice((named, tuple.then_ignore(TokenInput::punctuation(Punct::Semi)), TokenInput::punctuation(Punct::Semi).to(Fields::Unit))).boxed()
        } else {
            choice((named, tuple, empty().to(Fields::Unit))).boxed()
        }
    }

    /// `a::b::{c, d as e, *}`.
    fn use_tree<'a, I: In<'a>>() -> P<'a, I, UseTree> {
        recursive(|tree| {
            let prefix = TokenInput::path_identifier().then_ignore(TokenInput::punctuation(Punct::ColonColon)).repeated().collect::<Vec<_>>();
            let group = tree
                .separated_by(TokenInput::punctuation(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace));
            let alias = TokenInput::keyword(Kw::As).ignore_then(TokenInput::identifier()).or_not();
            let end = choice((
                TokenInput::punctuation(Punct::Star).to(None),
                group.map(|g| Some(Err(g))),
                TokenInput::path_identifier().then(alias).map(|(last, alias)| Some(Ok((last, alias)))),
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
    pub(crate) fn definition<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Item> {
        let struct_decl = TokenInput::keyword(Kw::Struct)
            .ignore_then(TokenInput::identifier())
            .then(GenericsGrammar::parameters(h).or_not())
            .then_ignore(ItemGrammar::removed_where())
            .then(ItemGrammar::fields(h, true))
            .map(|((name, generics), fields)| ItemKind::Struct(StructDecl { name, generics, fields }));

        let variant = ItemGrammar::meta(h)
            .then(TokenInput::identifier())
            .then(ItemGrammar::fields(h, false))
            .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).or_not())
            .map_with(|(((meta, name), fields), discriminant), e| Variant {
                meta,
                name,
                fields,
                discriminant,
                span: Loc(e.span()),
            });
        let enum_decl = TokenInput::keyword(Kw::Enum)
            .ignore_then(TokenInput::identifier())
            .then(GenericsGrammar::parameters(h).or_not())
            .then_ignore(ItemGrammar::removed_where())
            .then(
                variant
                    .separated_by(TokenInput::punctuation(Punct::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace)),
            )
            .map(|((name, generics), variants)| ItemKind::Enum(EnumDecl { name, generics, variants }));

        let impl_block = TokenInput::keyword(Kw::Impl)
            .ignore_then(GenericsGrammar::parameters(h).or_not())
            .then(h.ty.clone())
            .then(TokenInput::keyword(Kw::For).ignore_then(h.ty.clone()).or_not())
            .then_ignore(ItemGrammar::removed_where())
            .then(ItemGrammar::impl_items(h))
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

        let trait_decl = TokenInput::keyword(Kw::Trait)
            .ignore_then(TokenInput::identifier())
            .then(GenericsGrammar::parameters(h).or_not())
            .then(
                TokenInput::punctuation(Punct::Colon)
                    .ignore_then(TypeGrammar::path(h, false).separated_by(TokenInput::punctuation(Punct::Plus)).at_least(1).collect::<Vec<_>>())
                    .or_not(),
            )
            .then_ignore(ItemGrammar::removed_where())
            .then(ItemGrammar::impl_items(h))
            .map(|(((name, generics), supertraits), items)| {
                ItemKind::Trait(TraitDecl { name, generics, supertraits: supertraits.unwrap_or_default(), items })
            });

        // A leading `::` is removed (B.3.6): reported at the `::`, the tree kept.
        let use_decl = TokenInput::keyword(Kw::Use)
            .ignore_then(TokenInput::punctuation(Punct::ColonColon).map_with(|_, e| e.span()).or_not())
            .then(ItemGrammar::use_tree())
            .then_ignore(TokenInput::punctuation(Punct::Semi))
            .validate(|(leading, tree), _, emitter| {
                if let Some(at) = leading {
                    emitter.emit(ParseDiagnostics::removed("a leading `::`", at));
                }
                ItemKind::Use(tree)
            });

        let mod_decl = TokenInput::keyword(Kw::Mod)
            .ignore_then(TokenInput::identifier())
            .then(choice((
                TokenInput::punctuation(Punct::Semi).to(None),
                h.item
                    .clone()
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace))
                    .map(Some),
            )))
            .map(|(name, items)| ItemKind::Mod(ModItem { name, items }));

        let macro_rules = select! { Token::Ident(s) if s == "macro_rules" => () }
            .ignore_then(TokenInput::punctuation(Punct::Not))
            .ignore_then(TokenInput::identifier().or_not())
            .ignore_then(Recovery::skip_group())
            .validate(|_, e, emitter| {
                emitter.emit(ParseDiagnostics::macro_error(e.span()));
                ItemKind::Error
            });

        // `type`, `static`, `union` and `extern` blocks are removed (B.3.6): each is
        // reported once at its keyword, its rest skipped, and the item kept as an error.
        let skipped = |first: P<'a, I, Span>, row: &'static str| {
            first
                .then_ignore(Recovery::skip_to_end())
                .then_ignore(TokenInput::punctuation(Punct::Semi).or_not())
                .validate(move |at, _, emitter| {
                    emitter.emit(ParseDiagnostics::removed(row, at));
                    ItemKind::Error
                })
                .boxed()
        };
        let type_alias = skipped(TokenInput::keyword(Kw::Type).map_with(|_, e| e.span()).boxed(), "type aliases");
        let static_item = skipped(TokenInput::keyword(Kw::Static).map_with(|_, e| e.span()).boxed(), "`static` items");
        let union_item = select! { Token::Ident(s) if s == "union" => () }
            .map_with(|_, e| e.span())
            .then_ignore(TokenInput::identifier())
            .then_ignore(GenericsGrammar::parameters(h).or_not())
            .then_ignore(Recovery::skip_group())
            .validate(|at, _, emitter| {
                emitter.emit(ParseDiagnostics::removed("`union` items", at));
                ItemKind::Error
            })
            .boxed();
        let extern_crate = skipped(
            TokenInput::keyword(Kw::Extern).map_with(|_, e| e.span()).then_ignore(TokenInput::keyword(Kw::Crate)).boxed(),
            "`extern crate` items",
        );
        let extern_block = TokenInput::keyword(Kw::Extern)
            .map_with(|_, e| e.span())
            .then_ignore(TokenInput::string_literal().or_not())
            .then_ignore(Recovery::skip_group())
            .validate(|at, _, emitter| {
                emitter.emit(ParseDiagnostics::removed("`extern` blocks", at));
                ItemKind::Error
            })
            .boxed();

        // An `emit!` statement at the file's top: `E0601`, as in a module body.
        let stray_emit = select! { Token::Ident(s) if s == "emit" => () }
            .then(TokenInput::punctuation(Punct::Not))
            .rewind()
            .ignore_then(ExpressionGrammar::statement(h))
            .validate(|statement, _, emitter| {
                emitter.emit(msl::misplaced_emit(statement.span.0));
                ItemKind::Error
            })
            .boxed();
        let kind = TokenInput::first_of(vec![
            stray_emit,
            ItemGrammar::fn_decl(h).map(ItemKind::Fn).boxed(),
            ItemGrammar::const_item(h).map(|c| ItemKind::Const(c)).boxed(),
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
        let item = ItemGrammar::meta(h)
            .then(TokenInput::visibility().or_not())
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
        Recovery::recover(item, true, |span| Item { meta: Meta::default(), public: false, kind: ItemKind::Error, span: Loc(span) })
    }
}
