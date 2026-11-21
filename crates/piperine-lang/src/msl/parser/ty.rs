//! Types, paths and generics.

use super::*;

/// `Segment (:: Segment)*`. A segment's generic arguments need a turbofish
/// (`idt::<Heat>`) in an expression path and take `::` optionally in a type
/// path (`Bit<u8>`); `Fn(A) -> R` is the bound sugar.
pub(crate) fn path<'a, I: In<'a>>(h: &Handles<'a, I>, in_expr: bool) -> P<'a, I, Path> {
    path_with(h, in_expr, false)
}

/// A bound path: `Repr`, `Fn(Cell) -> R`.
fn bound<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Path> {
    path_with(h, false, true)
}

fn path_with<'a, I: In<'a>>(h: &Handles<'a, I>, in_expr: bool, sugar: bool) -> P<'a, I, Path> {
    let args = if sugar {
        generic_args(h, in_expr).or(fn_sugar(h)).or_not().boxed()
    } else {
        generic_args(h, in_expr).or_not().boxed()
    };
    let seg = path_ident().then(args).map(|(ident, args)| PathSeg { ident, args });
    // A leading `::` is removed (B.3.6): reported at the `::`, the path kept.
    let leading = just(Token::Punct(Punct::ColonColon)).map_with(|_, e| e.span()).or_not();
    leading
        .then(seg.separated_by(punct(Punct::ColonColon)).at_least(1).collect::<Vec<_>>())
        .validate(|(leading, segments), e, emitter| {
            if let Some(at) = leading {
                emitter.emit(removed("a leading `::`", at));
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
        lit_expr().map(GenericArg::Const),
        neg_lit_expr().map(GenericArg::Const),
        h.ty.clone().map(GenericArg::Type),
    ));
    let colons = punct(Punct::ColonColon);
    let open = if in_expr {
        colons.map(|_| true).boxed()
    } else {
        colons.or_not().map(|c| c.is_some()).boxed()
    };
    open.then(
        arg.separated_by(punct(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(punct(Punct::Lt), punct(Punct::Gt)),
    )
    .map_with(|(turbofish, args), e| GenericArgs::Angle { args, turbofish, span: Loc(e.span()) })
    .boxed()
}

/// `- literal`: a negative literal as a const argument (A.5 rule 4).
fn neg_lit_expr<'a, I: In<'a>>() -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
    punct(Punct::Minus).ignore_then(lit_expr()).map_with(|inner, e| Expr {
        span: Loc(e.span()),
        kind: ExprKind::Unary { op: UnOp::Neg, expr: Box::new(inner) },
    })
}

/// `(A, B) -> R` after the name `Fn`.
fn fn_sugar<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, GenericArgs> {
    let inputs = h
        .ty
        .clone()
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen));
    inputs
        .then(punct(Punct::Arrow).ignore_then(h.ty.clone()).or_not())
        .map_with(|(inputs, output), e| GenericArgs::Paren {
            inputs,
            output: output.map(Box::new),
            span: Loc(e.span()),
        })
        .boxed()
}

/// `<T: Bound + Bound = Default, const N: u32 = 4>` on a declaration.
pub(crate) fn generics<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Generics> {
    let bounds = bound(h).separated_by(punct(Punct::Plus)).at_least(1).collect::<Vec<_>>();
    let type_param = ident()
        .then(punct(Punct::Colon).ignore_then(bounds).or_not())
        .then(punct(Punct::Eq).ignore_then(h.ty.clone()).or_not())
        .map_with(|((name, bounds), default), e| GenericParam {
            name,
            kind: GenericParamKind::Type { bounds: bounds.unwrap_or_default(), default },
            span: Loc(e.span()),
        });
    let const_param = kw(Kw::Const)
        .ignore_then(ident())
        .then_ignore(punct(Punct::Colon))
        .then(h.ty.clone())
        .then(punct(Punct::Eq).ignore_then(lit_expr().or(h.block.clone().map_with(|b: Block, e| {
            Expr { span: Loc(e.span()), kind: ExprKind::Block(b) }
        }))).or_not())
        .map_with(|((name, ty), default), e| GenericParam {
            name,
            kind: GenericParamKind::Const { ty, default },
            span: Loc(e.span()),
        });
    super::item::rejected_meta(h)
        .ignore_then(choice((const_param, type_param)))
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::Lt), punct(Punct::Gt))
        .map_with(|params, e| Generics { params, span: Loc(e.span()) })
        .boxed()
}

/// `<T as Tr>::X`: a qualified path, a removed form (B.3.6); the caller reports it.
pub(crate) fn qualified_path<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ()> {
    punct(Punct::Lt)
        .ignore_then(h.ty.clone())
        .ignore_then(kw(Kw::As))
        .ignore_then(bound(h))
        .ignore_then(punct(Punct::Gt))
        .ignore_then(punct(Punct::ColonColon).ignore_then(path_ident()).repeated().at_least(1))
        .ignored()
        .boxed()
}

/// A `sigtype` (B.3.3): `flip`, a path and an array of them. Only a `signal`
/// declaration takes one; a tuple, a reference or a function pointer there is
/// reported (`E0900`) and the declaration is kept.
pub(crate) fn sigtype<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Type> {
    fn carrier(t: &Type) -> bool {
        match &t.kind {
            TypeKind::Path(_) => true,
            TypeKind::Flip(inner) | TypeKind::Array { elem: inner, .. } => carrier(inner),
            _ => false,
        }
    }
    h.ty
        .clone()
        .validate(|t, _, emitter| {
            if !carrier(&t) {
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
pub(crate) fn build<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Type> {
    let ty = h.ty.clone();
    let mk = |kind: TypeKind, span: Span| Type { kind, span: Loc(span) };

    let flip = kw(Kw::Flip)
        .ignore_then(ty.clone())
        .map_with(move |t, e| mk(TypeKind::Flip(Box::new(t)), e.span()));

    // `&T`, `&&T`, `&mut T`: removed (B.3.6); reported at the head, read as the type it borrows.
    let reference = reference_head().then(ty.clone()).validate(|(head, inner), _, emitter| {
        emitter.emit(removed("references and borrows", head));
        inner
    });

    // `[T; N]`: a type that is a list has its length (`[T]` is not a type).
    let array = ty
        .clone()
        .then_ignore(punct(Punct::Semi))
        .then(h.expr.clone())
        .delimited_by(punct(Punct::LBracket), punct(Punct::RBracket))
        .map_with(move |(elem, len), e| mk(TypeKind::Array { elem: Box::new(elem), len: Box::new(len) }, e.span()));

    // `fn(A, B) -> R`, taken only where a type is expected.
    // A parameter name is removed (B.3.6): reported, the type kept.
    let named_param = ident()
        .then_ignore(punct(Punct::Colon))
        .then(ty.clone())
        .validate(|(_, t), e, emitter| {
            emitter.emit(removed("named function-pointer parameters", e.span()));
            t
        });
    let fn_ptr = kw(Kw::Fn)
        .ignore_then(
            named_param
                .or(ty.clone())
                .separated_by(punct(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LParen), punct(Punct::RParen)),
        )
        .then(punct(Punct::Arrow).ignore_then(ty.clone()).or_not())
        .map_with(move |(params, ret), e| mk(TypeKind::FnPtr { params, ret: ret.map(Box::new) }, e.span()));

    // `(T)` is T; `(T,)`, `(A, B)` and `()` are tuples.
    let tuple_or_paren = ty
        .clone()
        .separated_by(punct(Punct::Comma))
        .collect::<Vec<_>>()
        .then(punct(Punct::Comma).or_not())
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen))
        .map_with(move |(mut items, trailing), e| {
            if items.len() == 1 && trailing.is_none() {
                items.remove(0)
            } else {
                mk(TypeKind::Tuple(items), e.span())
            }
        });

    let infer = punct(Punct::Underscore).map_with(move |_, e| mk(TypeKind::Infer, e.span()));

    // Removed forms (B.3.6): each is reported once and stands as an inferred type.
    let impl_trait = kw(Kw::Impl)
        .ignore_then(bound(h).separated_by(punct(Punct::Plus)).at_least(1).collect::<Vec<_>>())
        .validate(move |_, e, emitter| {
            emitter.emit(removed("`impl Trait` types", e.span()));
            mk(TypeKind::Infer, e.span())
        });
    let never = punct(Punct::Not).validate(move |_, e, emitter| {
        emitter.emit(removed("the never type", e.span()));
        mk(TypeKind::Infer, e.span())
    });
    let raw_pointer = punct(Punct::Star)
        .ignore_then(kw(Kw::Const).or(kw(Kw::Mut)))
        .ignore_then(ty.clone())
        .validate(move |_, e, emitter| {
            emitter.emit(removed("raw-pointer types", e.span()));
            mk(TypeKind::Infer, e.span())
        });
    let qualified = qualified_path(h).validate(move |_, e, emitter| {
        emitter.emit(removed("qualified paths", e.span()));
        mk(TypeKind::Infer, e.span())
    });
    let path_ty = path(h, false).map_with(move |p, e| mk(TypeKind::Path(p), e.span()));

    first_of(vec![
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
