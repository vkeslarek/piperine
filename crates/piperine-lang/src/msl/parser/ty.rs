//! Types, paths and generics.

use super::*;

/// The grammar of the generic parameters of a declaration: `<T: Bound = Default, const N: u32 = 4>`.
pub struct GenericsGrammar;

impl GenericsGrammar {
    /// Parse `src` as generic parameters (`<T: Repr = bool, const N: u32>`).
    pub fn parse(src: &str) -> Parsed<Generics> {
        let (tokens, diags) = SourceParser::lex_limited(src);
        let input = TokenInput::over(&tokens, src.len());
        let h = Handles::declare();
        ParseDiagnostics::finish(crate::stack::Stack::deep(|| GenericsGrammar::parameters(&h).then_ignore(end()).parse(input).into_output_errors()), diags, &tokens)
    }

    /// `<T: Bound + Bound = Default, const N: u32 = 4>` on a declaration.
    pub(crate) fn parameters<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Generics> {
        let bounds = TypeGrammar::bound(h).separated_by(TokenInput::punctuation(Punct::Plus)).at_least(1).collect::<Vec<_>>();
        let type_param = TokenInput::identifier()
            .then(TokenInput::punctuation(Punct::Colon).ignore_then(bounds).or_not())
            .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.ty.clone()).or_not())
            .map_with(|((name, bounds), default), e| GenericParam {
                name,
                kind: GenericParamKind::Type { bounds: bounds.unwrap_or_default(), default },
                span: Loc(e.span()),
            });
        let const_param = TokenInput::keyword(Kw::Const)
            .ignore_then(TokenInput::identifier())
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then(h.ty.clone())
            .then(TokenInput::punctuation(Punct::Eq).ignore_then(TokenInput::literal_expression().or(h.block.clone().map_with(|b: Block, e| {
                Expr { span: Loc(e.span()), kind: ExprKind::Block(b) }
            }))).or_not())
            .map_with(|((name, ty), default), e| GenericParam {
                name,
                kind: GenericParamKind::Const { ty, default },
                span: Loc(e.span()),
            });
        ItemGrammar::rejected_meta(h)
            .ignore_then(choice((const_param, type_param)))
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::Lt), TokenInput::punctuation(Punct::Gt))
            .map_with(|params, e| Generics { params, span: Loc(e.span()) })
            .boxed()
    }
}

/// The type grammar: types, paths and generic arguments. Each production is a method that returns its parser.
pub struct TypeGrammar;

impl TypeGrammar {
    /// Parse `src` as one type. Lexical diagnostics come first-class in the result.
    pub fn parse(src: &str) -> Parsed<Type> {
        let (tokens, diags) = SourceParser::lex_limited(src);
        let input = TokenInput::over(&tokens, src.len());
        let h = Handles::declare();
        ParseDiagnostics::finish(crate::stack::Stack::deep(|| TypeGrammar::definition(&h).then_ignore(end()).parse(input).into_output_errors()), diags, &tokens)
    }

    /// `Segment (:: Segment)*`. A segment's generic arguments need a turbofish
    /// (`idt::<Heat>`) in an expression path and take `::` optionally in a type
    /// path (`Bit<u8>`); `Fn(A) -> R` is the bound sugar.
    pub(crate) fn path<'a, I: In<'a>>(h: &Handles<'a, I>, in_expr: bool) -> P<'a, I, Path> {
        TypeGrammar::path_with_sugar(h, in_expr, false)
    }

    /// A bound path: `Repr`, `Fn(Cell) -> R`.
    fn bound<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Path> {
        TypeGrammar::path_with_sugar(h, false, true)
    }

    /// A path; `in_expr` asks for the turbofish before generic arguments and `sugar` admits `Fn(A) -> R`.
    fn path_with_sugar<'a, I: In<'a>>(h: &Handles<'a, I>, in_expr: bool, sugar: bool) -> P<'a, I, Path> {
        let args = if sugar {
            TypeGrammar::generic_args(h, in_expr).or(TypeGrammar::fn_sugar(h)).or_not().boxed()
        } else {
            TypeGrammar::generic_args(h, in_expr).or_not().boxed()
        };
        let seg = TokenInput::path_identifier().then(args).map(|(ident, args)| PathSeg { ident, args });
        // A leading `::` is removed (B.3.6): reported at the `::`, the path kept.
        let leading = just(Token::Punct(Punct::ColonColon)).map_with(|_, e| e.span()).or_not();
        leading
            .then(seg.separated_by(TokenInput::punctuation(Punct::ColonColon)).at_least(1).collect::<Vec<_>>())
            .validate(|(leading, segments), e, emitter| {
                if let Some(at) = leading {
                    emitter.emit(ParseDiagnostics::removed("a leading `::`", at));
                }
                Path { segments, span: Loc(e.span()) }
            })
            .labelled("path")
            .boxed()
    }

    /// `<A, B, 4, { N + 1 }>`, with a required `::` in front when `in_expr`.
    pub(crate) fn generic_args<'a, I: In<'a>>(
        h: &Handles<'a, I>,
        in_expr: bool,
    ) -> P<'a, I, GenericArgs> {
        let arg = choice((
            h.block.clone().map_with(|b: Block, e| {
                GenericArg::Const(Expr { span: Loc(e.span()), kind: ExprKind::Block(b) })
            }),
            TokenInput::literal_expression().map(GenericArg::Const),
            TypeGrammar::negative_literal().map(GenericArg::Const),
            h.ty.clone().map(GenericArg::Type),
        ));
        let colons = TokenInput::punctuation(Punct::ColonColon);
        let open = if in_expr {
            colons.map(|_| true).boxed()
        } else {
            colons.or_not().map(|c| c.is_some()).boxed()
        };
        open.then(
            arg.separated_by(TokenInput::punctuation(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::Lt), TokenInput::punctuation(Punct::Gt)),
        )
        .map_with(|(turbofish, args), e| GenericArgs::Angle { args, turbofish, span: Loc(e.span()) })
        .boxed()
    }

    /// `- literal`: a negative literal as a const argument (A.5 rule 4).
    fn negative_literal<'a, I: In<'a>>() -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
        TokenInput::punctuation(Punct::Minus).ignore_then(TokenInput::literal_expression()).map_with(|inner, e| Expr {
            span: Loc(e.span()),
            kind: ExprKind::Unary { op: UnOp::Neg, expr: Box::new(inner) },
        })
    }

    /// `(A, B) -> R` after the name `Fn`.
    fn fn_sugar<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, GenericArgs> {
        let inputs = h
            .ty
            .clone()
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen));
        inputs
            .then(TokenInput::punctuation(Punct::Arrow).ignore_then(h.ty.clone()).or_not())
            .map_with(|(inputs, output), e| GenericArgs::Paren {
                inputs,
                output: output.map(Box::new),
                span: Loc(e.span()),
            })
            .boxed()
    }

    /// `<T as Tr>::X`: a qualified path, a removed form (B.3.6); the caller reports it.
    pub(crate) fn qualified_path<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ()> {
        TokenInput::punctuation(Punct::Lt)
            .ignore_then(h.ty.clone())
            .ignore_then(TokenInput::keyword(Kw::As))
            .ignore_then(TypeGrammar::bound(h))
            .ignore_then(TokenInput::punctuation(Punct::Gt))
            .ignore_then(TokenInput::punctuation(Punct::ColonColon).ignore_then(TokenInput::path_identifier()).repeated().at_least(1))
            .ignored()
            .boxed()
    }

    /// Whether `t` is a carrier type: a path, or `flip` or an array of one.
    fn is_carrier(t: &Type) -> bool {
        match &t.kind {
            TypeKind::Path(_) => true,
            TypeKind::Flip(inner) | TypeKind::Array { elem: inner, .. } => TypeGrammar::is_carrier(inner),
            _ => false,
        }
    }

    /// A `sigtype` (B.3.3): `flip`, a path and an array of them. Only a `signal`
    /// declaration takes one; a tuple, a reference or a function pointer there is
    /// reported (`E0900`) and the declaration is kept.
    pub(crate) fn sigtype<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Type> {
        h.ty
            .clone()
            .validate(|t, _, emitter| {
                if !TypeGrammar::is_carrier(&t) {
                    emitter.emit(Rich::custom(
                        t.span.0,
                        Custom {
                            code: "E0900",
                            message: "a signal's type is a nature, a bundle or an array of them".into(),
                            help: Some("write a nature or bundle: `Bit`, `flip Stream<T>` or `[Bit; 4]`".into()),
                        },
                    ));
                }
                t
            })
            .boxed()
    }

    /// The type parser (also `sigtype` of B.3: `flip` and arrays are types).
    pub(crate) fn definition<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Type> {
        let ty = h.ty.clone();
        let mk = |kind: TypeKind, span: Span| Type { kind, span: Loc(span) };

        let flip = TokenInput::keyword(Kw::Flip)
            .ignore_then(ty.clone())
            .map_with(move |t, e| mk(TypeKind::Flip(Box::new(t)), e.span()));

        // `&T`, `&&T`, `&mut T`: removed (B.3.6); reported at the head, read as the type it borrows.
        let reference = TokenInput::reference_head().then(ty.clone()).validate(|(head, inner), _, emitter| {
            emitter.emit(ParseDiagnostics::removed("references and borrows", head));
            inner
        });

        // `[T; N]`. `[T]` is no type (B.3.7): it is reported at the type and stands as an inferred one, so the
        // form around it (`&[T]`) is still reported for itself.
        let array = ty
            .clone()
            .then(TokenInput::punctuation(Punct::Semi).ignore_then(h.expr.clone()).or_not())
            .delimited_by(TokenInput::punctuation(Punct::LBracket), TokenInput::punctuation(Punct::RBracket))
            .validate(move |(elem, len), e, emitter| match len {
                Some(len) => mk(TypeKind::Array { elem: Box::new(elem), len: Box::new(len) }, e.span()),
                None => {
                    emitter.emit(Rich::custom(
                        e.span(),
                        Custom {
                            code: "E0900",
                            message: "slice types are not part of the language".into(),
                            help: Some("write an array type with its length, `[T; N]`: a function takes a table as `[T; N]` under `const N: usize`".into()),
                        },
                    ));
                    mk(TypeKind::Infer, e.span())
                }
            });

        // `fn(A, B) -> R`, taken only where a type is expected.
        // A parameter name is removed (B.3.6): reported, the type kept.
        let named_param = TokenInput::identifier()
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then(ty.clone())
            .validate(|(_, t), e, emitter| {
                emitter.emit(ParseDiagnostics::removed("named function-pointer parameters", e.span()));
                t
            });
        let fn_ptr = TokenInput::keyword(Kw::Fn)
            .ignore_then(
                named_param
                    .or(ty.clone())
                    .separated_by(TokenInput::punctuation(Punct::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen)),
            )
            .then(TokenInput::punctuation(Punct::Arrow).ignore_then(ty.clone()).or_not())
            .map_with(move |(params, ret), e| mk(TypeKind::FnPtr { params, ret: ret.map(Box::new) }, e.span()));

        // `(T)` is T; `(T,)`, `(A, B)` and `()` are tuples.
        let tuple_or_paren = ty
            .clone()
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .collect::<Vec<_>>()
            .then(TokenInput::punctuation(Punct::Comma).or_not())
            .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
            .map_with(move |(mut items, trailing), e| {
                if items.len() == 1 && trailing.is_none() {
                    items.remove(0)
                } else {
                    mk(TypeKind::Tuple(items), e.span())
                }
            });

        let infer = TokenInput::punctuation(Punct::Underscore).map_with(move |_, e| mk(TypeKind::Infer, e.span()));

        // Removed forms (B.3.6): each is reported once and stands as an inferred type.
        let impl_trait = TokenInput::keyword(Kw::Impl)
            .ignore_then(TypeGrammar::bound(h).separated_by(TokenInput::punctuation(Punct::Plus)).at_least(1).collect::<Vec<_>>())
            .validate(move |_, e, emitter| {
                emitter.emit(ParseDiagnostics::removed("`impl Trait` types", e.span()));
                mk(TypeKind::Infer, e.span())
            });
        let never = TokenInput::punctuation(Punct::Not).validate(move |_, e, emitter| {
            emitter.emit(ParseDiagnostics::removed("the never type", e.span()));
            mk(TypeKind::Infer, e.span())
        });
        let raw_pointer = TokenInput::punctuation(Punct::Star)
            .ignore_then(TokenInput::keyword(Kw::Const).or(TokenInput::keyword(Kw::Mut)))
            .ignore_then(ty.clone())
            .validate(move |_, e, emitter| {
                emitter.emit(ParseDiagnostics::removed("raw-pointer types", e.span()));
                mk(TypeKind::Infer, e.span())
            });
        let qualified = TypeGrammar::qualified_path(h).validate(move |_, e, emitter| {
            emitter.emit(ParseDiagnostics::removed("qualified paths", e.span()));
            mk(TypeKind::Infer, e.span())
        });
        let path_ty = TypeGrammar::path(h, false).map_with(move |p, e| mk(TypeKind::Path(p), e.span()));

        TokenInput::first_of(vec![
            flip.boxed(),
            fn_ptr.boxed(),
            reference.boxed(),
            array.boxed(),
            tuple_or_paren.boxed(),
            infer.boxed(),
            impl_trait.boxed(),
            never.boxed(),
            raw_pointer.boxed(),
            qualified.boxed(),
            path_ty.boxed(),
        ])
            .labelled("type")
            .boxed()
    }
}
