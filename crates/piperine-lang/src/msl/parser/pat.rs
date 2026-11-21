//! Patterns.

use super::*;

/// Both pattern parsers: the full one (alternatives allowed) and the one
/// without a top-level alternative.
pub(crate) fn build<'a, I: In<'a>>(
    h: &Handles<'a, I>,
) -> (P<'a, I, Pat>, P<'a, I, Pat>) {
    let pat = h.pat.clone();
    let pat1 = h.pat1.clone();
    let mk = |kind: PatKind, span: Span| Pat { kind, span: Loc(span) };

    let wild = punct(Punct::Underscore).map_with(move |_, e| mk(PatKind::Wild, e.span()));
    let rest = punct(Punct::DotDot).map_with(move |_, e| mk(PatKind::Rest, e.span()));
    // `&p`: removed (B.3.6); reported at the head, read as the pattern it borrows.
    let reference = reference_head().then(pat1.clone()).validate(|(head, inner), _, emitter| {
        emitter.emit(removed("references and borrows", head));
        inner
    });

    // `(p)` is p; `(p,)`, `(a, b)` and `()` are tuples.
    let tuple = pat
        .clone()
        .separated_by(punct(Punct::Comma))
        .collect::<Vec<_>>()
        .then(punct(Punct::Comma).or_not())
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen))
        .map_with(move |(mut items, trailing), e| {
            if items.len() == 1 && trailing.is_none() && !matches!(items[0].kind, PatKind::Rest) {
                items.remove(0)
            } else {
                mk(PatKind::Tuple(items), e.span())
            }
        });
    let slice = pat
        .clone()
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBracket), punct(Punct::RBracket))
        .map_with(move |items, e| mk(PatKind::Array(items), e.span()));

    // A range bound: an optionally negated literal, or a path.
    let neg_lit = punct(Punct::Minus).or_not().then(lit_expr()).map_with(|(neg, l), e| match neg {
        Some(()) => Expr {
            span: Loc(e.span()),
            kind: ExprKind::Unary { op: UnOp::Neg, expr: Box::new(l) },
        },
        None => l,
    });
    let path_bound = ty::path(h, true).map_with(|p: Path, e| Expr {
        span: Loc(e.span()),
        kind: ExprKind::Path(p),
    });
    let bound = neg_lit.clone().or(path_bound);
    let range_op = punct(Punct::DotDotEq).to(true).or(punct(Punct::DotDot).to(false));

    // `..=hi`
    let range_to = punct(Punct::DotDotEq).ignore_then(bound.clone()).map_with(move |hi, e| {
        mk(PatKind::Range { lo: None, hi: Some(Box::new(hi)), inclusive: true }, e.span())
    });
    // `lo`, `lo..`, `lo..hi`, `lo..=hi` starting from a literal.
    let lit_or_range = neg_lit
        .clone()
        .then(range_op.clone().then(bound.clone().or_not()).or_not())
        .map_with(move |(lo, tail), e| match tail {
            None => mk(PatKind::Lit(Box::new(lo)), e.span()),
            Some((inclusive, hi)) => mk(
                PatKind::Range { lo: Some(Box::new(lo)), hi: hi.map(Box::new), inclusive },
                e.span(),
            ),
        });

    // `ref x`, `mut x`, `ref mut x`: each marker is a reserved word (`E0903`), the name is read as plain.
    let binding_mods = kw(Kw::Ref)
        .map_with(|_, e| (Kw::Ref, e.span()))
        .or_not()
        .then(kw(Kw::Mut).map_with(|_, e| (Kw::Mut, e.span())).or_not())
        .try_map(|(r, m), span| {
            if r.is_none() && m.is_none() {
                Err(Rich::custom(span, Custom {
                    code: "E0900",
                    message: "expected a binding modifier".into(),
                    help: None,
                }))
            } else {
                Ok([r, m])
            }
        })
        .then(ident())
        .then(punct(Punct::At).ignore_then(pat1.clone()).or_not())
        .validate(move |((markers, name), sub), e, emitter| {
            for (word, span) in markers.into_iter().flatten() {
                emitter.emit(ReservedMarker::report(word, span));
            }
            mk(PatKind::Ident { name, sub: sub.map(Box::new) }, e.span())
        });

    // A path: a binding (single segment), a unit/const path, a tuple struct,
    // a struct, or the start of a range.
    let field_pat = ident()
        .then(punct(Punct::Colon).ignore_then(pat.clone()).or_not())
        .map_with(|(name, pat), e| FieldPat { name, pat, span: Loc(e.span()) });
    let struct_body = field_pat
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .then(punct(Punct::DotDot).ignore_then(punct(Punct::Comma).or_not()).or_not())
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace));
    let tuple_body = pat
        .clone()
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen));
    enum Tail {
        Tuple(Vec<Pat>),
        Struct(Vec<FieldPat>, bool),
        Range(bool, Option<Expr>),
        At(Pat),
    }
    let tail = choice((
        tuple_body.map(Tail::Tuple),
        struct_body.map(|(fields, rest)| Tail::Struct(fields, rest.is_some())),
        range_op.then(bound.or_not()).map(|(inc, hi)| Tail::Range(inc, hi)),
        punct(Punct::At).ignore_then(pat1.clone()).map(Tail::At),
    ))
    .or_not();
    let path_pat = ty::path(h, true).then(tail).validate(move |(path, tail), e, emitter| {
        // `self`, `Self`, `super` and `crate` are path keywords: never a name.
        if let (None | Some(Tail::At(_)), [seg]) = (&tail, path.segments.as_slice()) {
            if seg.args.is_none() && matches!(seg.ident.name.as_str(), "self" | "Self" | "super" | "crate") {
                emitter.emit(Rich::custom(
                    seg.ident.span.0,
                    Custom {
                        code: "E0903",
                        message: format!("`{}` is a reserved keyword and cannot be used as an identifier", seg.ident.name),
                        help: Some("choose another name for the binding".into()),
                    },
                ));
            }
        }
        (path, tail, e.span())
    })
    .map(move |(path, tail, span)| {
        match tail {
            Some(Tail::Tuple(pats)) => mk(PatKind::TupleStruct { path, pats }, span),
            Some(Tail::Struct(fields, rest)) => mk(PatKind::Struct { path, fields, rest }, span),
            Some(Tail::Range(inclusive, hi)) => {
                let lo = Expr { span: path.span, kind: ExprKind::Path(path) };
                mk(PatKind::Range { lo: Some(Box::new(lo)), hi: hi.map(Box::new), inclusive }, span)
            }
            Some(Tail::At(sub)) if path.segments.len() == 1 && path.segments[0].args.is_none() => {
                let name = path.segments[0].ident.clone();
                mk(PatKind::Ident { name, sub: Some(Box::new(sub)) }, span)
            }
            Some(Tail::At(_)) | None if path.segments.len() == 1 && path.segments[0].args.is_none() => {
                let name = path.segments[0].ident.clone();
                mk(PatKind::Ident { name, sub: None }, span)
            }
            _ => mk(PatKind::Path(path), span),
        }
    });

    // `..x`: a half-open range pattern is removed (B.3.6); reported, stands as `_`.
    let half_open = punct(Punct::DotDot).ignore_then(neg_lit.clone().map(|_| ()).or(ty::path(h, true).map(|_| ()))).validate(
        move |_, e, emitter| {
            emitter.emit(removed("half-open range patterns", e.span()));
            mk(PatKind::Wild, e.span())
        },
    );

    let no_alt = first_of(vec![
        wild.boxed(),
        half_open.boxed(),
        rest.boxed(),
        reference.boxed(),
        tuple.boxed(),
        slice.boxed(),
        range_to.boxed(),
        lit_or_range.boxed(),
        binding_mods.boxed(),
        path_pat.boxed(),
    ])
        .labelled("pattern")
        .boxed();

    let alts = punct(Punct::Or)
        .or_not()
        .ignore_then(pat1.clone().separated_by(punct(Punct::Or)).at_least(1).collect::<Vec<_>>())
        .map_with(move |mut v, e| {
            if v.len() == 1 { v.remove(0) } else { mk(PatKind::Or(v), e.span()) }
        })
        .boxed();

    (alts, no_alt)
}

/// Whether a pattern binds the literal `Unknown` where a name belongs.
fn binds_unknown(p: &Pat) -> Option<Span> {
    match &p.kind {
        PatKind::Lit(e) if matches!(e.kind, ExprKind::Lit(Lit { kind: LitKind::Unknown, .. })) => Some(p.span.0),
        PatKind::Tuple(v) | PatKind::Array(v) | PatKind::Or(v) | PatKind::TupleStruct { pats: v, .. } => {
            v.iter().find_map(binds_unknown)
        }
        PatKind::Ident { sub: Some(p), .. } => binds_unknown(p),
        PatKind::Struct { fields, .. } => fields.iter().find_map(|f| f.pat.as_ref().and_then(binds_unknown)),
        _ => None,
    }
}

/// A pattern in a binding position (`let`, `for`, a closure parameter):
/// like [`Handles::pat1`], but `Unknown` is a literal, not a name (`E0908`).
pub(crate) fn binding<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Pat> {
    h.pat1
        .clone()
        .validate(|p, _, emitter| {
            if let Some(span) = binds_unknown(&p) {
                emitter.emit(Rich::custom(
                    span,
                    Custom {
                        code: "E0908",
                        message: "`Unknown` is a literal, not a name".into(),
                        help: Some("choose another name for the binding".into()),
                    },
                ));
            }
            p
        })
        .boxed()
}
