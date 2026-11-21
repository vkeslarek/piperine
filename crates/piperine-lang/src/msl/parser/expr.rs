//! Expressions, blocks and statements.
//!
//! Precedence follows Rust: assignment, range, `||`, `&&`, comparison, `|`,
//! `^`, `&`, shifts, `+ -`, `* / %`, `as`, unary, postfix. `+>` and `<-` are
//! not expression operators: each ends the left side of its statement
//! (Appendix A.5 rule 6), so they appear only in [`stmt`].
//!
//! Two expression parsers exist: the full one and one that refuses a struct
//! literal or instance param block at the top level (`if x { .. }`,
//! `match v { .. }`, `for i in r { .. }`, `on e { .. }`), as in Rust.

use chumsky::pratt::{infix, left, none, postfix, prefix};
use chumsky::input::MapExtra;

use super::*;

fn mk(kind: ExprKind, span: Span) -> Expr {
    Expr { kind, span: Loc(span) }
}

fn boxed(e: Expr) -> Box<Expr> {
    Box::new(e)
}

// ---- `>`-family operators ---------------------------------------------------------------------
//
// The lexer emits every `>` alone so `Vec<Vec<T>>` needs no splitting; the
// operators `>=`, `>>` and `>>=` are `>` tokens that touch, joined here.

fn gt<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
    just(Token::Punct(Punct::Gt)).map_with(|_, e| e.span())
}

fn eq_tok<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
    just(Token::Punct(Punct::Eq)).map_with(|_, e| e.span())
}

/// After `as T`, `<` and `<<` open generic arguments, never a comparison or
/// a shift (Rust rejects `a as u8 < b`).
fn next_not_angle<'a, I: In<'a>>() -> impl Parser<'a, I, (), Extra<'a>> + Clone {
    any().filter(|t: &Token| !matches!(t, Token::Punct(Punct::Lt | Punct::Shl))).rewind().ignored().or(end())
}

fn next_is_not<'a, I: In<'a>>(p: Punct) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
    any().filter(move |t: &Token| *t != Token::Punct(p)).rewind().ignored()
}

/// Fail unless the spans touch: the tokens were written as one operator.
fn touching(spans: &[Span], span: Span, op: &str) -> Result<(), Err<'static>> {
    if spans.windows(2).all(|w| w[0].end == w[1].start) {
        Ok(())
    } else {
        Err(Rich::custom(
            span,
            Custom {
                code: "E0900",
                message: format!("`{op}` must be written without spaces between its characters"),
                help: None,
            },
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GtOp {
    Gt,
    Ge,
    Shr,
    ShrEq,
}

fn gt_op<'a, I: In<'a>>(want: GtOp) -> P<'a, I, GtOp> {
    match want {
        GtOp::Gt => gt()
            .then_ignore(next_is_not(Punct::Gt))
            .then_ignore(next_is_not(Punct::Eq))
            .to(GtOp::Gt)
            .boxed(),
        GtOp::Ge => gt()
            .then(eq_tok())
            .try_map(|(a, b), sp| touching(&[a, b], sp, ">=").map(|_| GtOp::Ge))
            .boxed(),
        GtOp::Shr => gt()
            .then(gt())
            .try_map(|(a, b), sp| touching(&[a, b], sp, ">>"))
            .then_ignore(next_is_not(Punct::Eq))
            .to(GtOp::Shr)
            .boxed(),
        GtOp::ShrEq => gt()
            .then(gt())
            .then(eq_tok())
            .try_map(|((a, b), c), sp| touching(&[a, b, c], sp, ">>=").map(|_| GtOp::ShrEq))
            .boxed(),
    }
}

/// A postfix form: field, method call, call, index, `?`.
#[derive(Clone)]
enum Post {
    Field(Ident),
    /// `x.0.1`: the lexer reads `0.1` as one float, so it is split here.
    Field2(Ident, Ident),
    Method(Ident, Option<GenericArgs>, Vec<Arg>),
    Call(Vec<Arg>, Option<Vec<FieldInit>>),
    Index(Expr),
    Try,
}

fn apply_post<'a, I: In<'a>>(lhs: Expr, post: Post, e: &mut MapExtra<'a, '_, I, Extra<'a>>) -> Expr {
    let kind = match post {
        Post::Field(name) => ExprKind::Field { expr: boxed(lhs), name },
        Post::Field2(first, second) => {
            let inner = Expr {
                span: Loc(lhs.span.0.to(first.span.0)),
                kind: ExprKind::Field { expr: boxed(lhs), name: first },
            };
            ExprKind::Field { expr: boxed(inner), name: second }
        }
        Post::Method(method, generics, args) => {
            ExprKind::MethodCall { recv: boxed(lhs), method, generics, args }
        }
        Post::Call(args, params) => ExprKind::Call { callee: boxed(lhs), args, params },
        Post::Index(i) => ExprKind::Index { expr: boxed(lhs), index: boxed(i) },
        Post::Try => ExprKind::Try(boxed(lhs)),
    };
    mk(kind, e.span())
}

fn binary<'a, I: In<'a>>(lhs: Expr, op: BinOp, rhs: Expr, e: &mut MapExtra<'a, '_, I, Extra<'a>>) -> Expr {
    mk(ExprKind::Binary { op, lhs: boxed(lhs), rhs: boxed(rhs) }, e.span())
}

// ---- builders ---------------------------------------------------------------------------------

/// The expression parser; `allow_struct` admits `Path { .. }` and an
/// instance's `{ .. }` param block at the top level. `me` is the handle of
/// the parser being defined (for closure bodies and assignment's right side).
pub(crate) fn build<'a, I: In<'a>>(
    h: &Handles<'a, I>,
    allow_struct: bool,
    me: Rec<'a, I, Expr>,
) -> P<'a, I, Expr> {
    let expr = h.expr.clone();

    // ---- atoms ----
    let paren_or_tuple = expr
        .clone()
        .separated_by(punct(Punct::Comma))
        .collect::<Vec<_>>()
        .then(punct(Punct::Comma).or_not())
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen))
        .map_with(|(mut items, trailing), e| {
            if items.len() == 1 && trailing.is_none() {
                mk(ExprKind::Paren(boxed(items.remove(0))), e.span())
            } else {
                mk(ExprKind::Tuple(items), e.span())
            }
        });

    // `[e; n]`, `[a, b]` or `[]`: the first element is parsed once.
    #[derive(Clone)]
    enum ArrayTail {
        Repeat(Expr),
        List(Vec<Expr>),
    }
    let array_tail = choice((
        punct(Punct::Semi).ignore_then(expr.clone()).map(ArrayTail::Repeat),
        punct(Punct::Comma)
            .ignore_then(
                expr.clone()
                    .separated_by(punct(Punct::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>(),
            )
            .map(ArrayTail::List),
        empty().to(Vec::new()).map(ArrayTail::List),
    ));
    let array = expr
        .clone()
        .then(array_tail)
        .or_not()
        .delimited_by(punct(Punct::LBracket), punct(Punct::RBracket))
        .map_with(|first, e| match first {
            None => mk(ExprKind::Array(Vec::new()), e.span()),
            Some((elem, ArrayTail::Repeat(len))) => {
                mk(ExprKind::ArrayRepeat { elem: boxed(elem), len: boxed(len) }, e.span())
            }
            Some((first, ArrayTail::List(mut rest))) => {
                rest.insert(0, first);
                mk(ExprKind::Array(rest), e.span())
            }
        });

    // `name: value`, or the shorthand `name` for `name: name`.
    let field_init = super::item::rejected_meta(h)
        .ignore_then(ident())
        .then(punct(Punct::Colon).ignore_then(expr.clone()).or_not())
        .map_with(|(name, value), e| {
            let shorthand = value.is_none();
            let value = value.unwrap_or_else(|| Expr {
                span: name.span,
                kind: ExprKind::Path(Path {
                    segments: vec![PathSeg { ident: name.clone(), args: None }],
                    span: name.span,
                }),
            });
            FieldInit { name, value, shorthand, span: Loc(e.span()) }
        });
    let field_inits = field_init
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace));

    // `emit!("stream", name = value, ..)`: the arguments are parsed loosely so
    // a wrong shape is the spec's E0602, not a generic syntax error.
    #[derive(Clone)]
    enum EmitArg {
        Named(FieldInit),
        /// `name: value`: a field spelled like a struct field.
        Colon(Span),
        Positional(Expr),
    }
    let emit_arg = first_of(vec![
        ident()
            .then_ignore(punct(Punct::Eq))
            .then(expr.clone())
            .map_with(|(name, value), e| FieldInit { name, value, shorthand: false, span: Loc(e.span()) })
            .map(EmitArg::Named)
            .boxed(),
        ident()
            .then_ignore(punct(Punct::Colon))
            .then_ignore(expr.clone())
            .map_with(|_, e| EmitArg::Colon(e.span()))
            .boxed(),
        expr.clone().map(EmitArg::Positional).boxed(),
    ]);
    let emit = select! { Token::Ident(s) if s == "emit" => () }
        .ignore_then(punct(Punct::Not))
        .ignore_then(
            emit_arg
                .separated_by(punct(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LParen), punct(Punct::RParen)),
        )
        .validate(|args, e, emitter| {
            let bad = |span: Span, what: &str| {
                Rich::custom(
                    span,
                    Custom {
                        code: "E0602",
                        message: format!("{what} in `emit!`"),
                        help: Some("write `emit!(\"stream\", name = value, ..)`: a string literal, then `name = value` fields".into()),
                    },
                )
            };
            let mut args = args.into_iter();
            let stream = match args.next() {
                Some(EmitArg::Positional(Expr { kind: ExprKind::Lit(l @ Lit { kind: LitKind::Str { .. }, .. }), .. })) => l,
                Some(EmitArg::Positional(x)) => {
                    emitter.emit(bad(x.span.0, "the first argument is not a string literal"));
                    return mk(ExprKind::Error, e.span());
                }
                Some(EmitArg::Named(f)) => {
                    emitter.emit(bad(f.span.0, "the first argument is not a string literal"));
                    return mk(ExprKind::Error, e.span());
                }
                Some(EmitArg::Colon(span)) => {
                    emitter.emit(bad(span, "the first argument is not a string literal"));
                    return mk(ExprKind::Error, e.span());
                }
                None => {
                    emitter.emit(bad(e.span(), "the stream name is missing"));
                    return mk(ExprKind::Error, e.span());
                }
            };
            let mut fields = Vec::new();
            let mut ok = true;
            for arg in args {
                match arg {
                    EmitArg::Named(f) => fields.push(f),
                    EmitArg::Colon(span) => {
                        emitter.emit(bad(span, "a field is not `name = value`"));
                        ok = false;
                    }
                    EmitArg::Positional(x) => {
                        emitter.emit(bad(x.span.0, "a field is not `name = value`"));
                        ok = false;
                    }
                }
            }
            if ok { mk(ExprKind::Emit { stream, fields }, e.span()) } else { mk(ExprKind::Error, e.span()) }
        });
    let include_str = select! { Token::Ident(s) if s == "include_str" => () }
        .ignore_then(punct(Punct::Not))
        .ignore_then(
            expr.clone()
                .separated_by(punct(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(punct(Punct::LParen), punct(Punct::RParen)),
        )
        .validate(|args, e, emitter| {
            let bad = |span: Span| {
                Rich::custom(
                    span,
                    Custom {
                        code: "E0611",
                        message: "the argument of `include_str!` is not a string literal".into(),
                        help: Some("write `include_str!(\"path/to/file\")`".into()),
                    },
                )
            };
            match args.as_slice() {
                [Expr { kind: ExprKind::Lit(l @ Lit { kind: LitKind::Str { .. }, .. }), .. }] => {
                    mk(ExprKind::IncludeStr(l.clone()), e.span())
                }
                [x] => {
                    emitter.emit(bad(x.span.0));
                    mk(ExprKind::Error, e.span())
                }
                _ => {
                    emitter.emit(bad(e.span()));
                    mk(ExprKind::Error, e.span())
                }
            }
        });

    let path_atom = {
        let path = ty::path(h, true);
        if allow_struct {
            path.then(field_inits.clone().or_not())
                .map_with(|(path, fields), e| match fields {
                    Some(fields) => mk(ExprKind::Struct { path, fields }, e.span()),
                    None => mk(ExprKind::Path(path), e.span()),
                })
                .boxed()
        } else {
            path.map_with(|path, e| mk(ExprKind::Path(path), e.span())).boxed()
        }
    };

    // Any other `name!(..)` is a macro: E0901, then the group is skipped.
    let macro_call = ty::path(h, true)
        .filter(|p: &Path| {
            !(p.segments.len() == 1 && matches!(p.segments[0].ident.name.as_str(), "emit" | "include_str"))
        })
        .then_ignore(punct(Punct::Not))
        .then_ignore(skip_group())
        .validate(|_, e, emitter| {
            emitter.emit(macro_error(e.span()));
            mk(ExprKind::Error, e.span())
        });

    // `<T as Tr>::f`: a removed form (B.3.6), reported once.
    let qualified = ty::qualified_path(h).validate(|_, e, emitter| {
        emitter.emit(removed("qualified paths", e.span()));
        mk(ExprKind::Error, e.span())
    });

    // A malformed token already reported by the lexer.
    let error_atom = just(Token::Error).map_with(|_, e| mk(ExprKind::Error, e.span()));

    let break_ = kw(Kw::Break)
        .ignore_then(me.clone().or_not())
        .map_with(|v, e| mk(ExprKind::Break(v.map(boxed)), e.span()));
    let continue_ = kw(Kw::Continue).map_with(|_, e| mk(ExprKind::Continue, e.span()));
    let return_ = kw(Kw::Return)
        .ignore_then(me.clone().or_not())
        .map_with(|v, e| mk(ExprKind::Return(v.map(boxed)), e.span()));

    // `move |a, b: T| -> R { .. }` / `|x| x + 1` / `|| 0`.
    let closure_param = pat::binding(h)
        .then(punct(Punct::Colon).ignore_then(h.ty.clone()).or_not())
        .map_with(|(pat, ty), e| ClosureParam { pat, ty, span: Loc(e.span()) });
    let closure_params = choice((
        punct(Punct::OrOr).to(Vec::new()),
        closure_param
            .separated_by(punct(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(punct(Punct::Or), punct(Punct::Or)),
    ));
    let closure = kw(Kw::Move)
        .map_with(|_, e| e.span())
        .validate(|at, _, emitter| emitter.emit(ReservedMarker::report(Kw::Move, at)))
        .or_not()
        .then(closure_params)
        .then(first_of(vec![
            punct(Punct::Arrow)
                .ignore_then(h.ty.clone())
                .then(h.block.clone().map_with(|b: Block, e| {
                    mk(ExprKind::Block(b), e.span())
                }))
                .map(|(ty, body)| (Some(ty), body))
                .boxed(),
            me.clone().map(|body| (None, body)).boxed(),
        ]))
        .map_with(|((mv, params), (ret, body)), e| {
            mk(
                ExprKind::Closure { is_move: mv.is_some(), params, ret, body: boxed(body) },
                e.span(),
            )
        });

    // A slice of boxed alternatives, not a tuple: a tuple `choice` keeps every
    // alternative's temporaries in one stack frame (60 KiB in a debug build).
    let atom = first_of(vec![
        lit_expr().boxed(),
        emit.boxed(),
        include_str.boxed(),
        paren_or_tuple.boxed(),
        array.boxed(),
        block_like(h),
        break_.boxed(),
        continue_.boxed(),
        return_.boxed(),
        closure.boxed(),
        error_atom.boxed(),
        macro_call.boxed(),
        qualified.boxed(),
        path_atom,
    ])
    .labelled("expression")
    .boxed();

    // ---- postfix ----
    let arg = ident()
        .then_ignore(punct(Punct::Colon))
        .or_not()
        .then(expr.clone())
        .map_with(|(name, value), e| Arg { name, value, span: Loc(e.span()) });
    let args = arg
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LParen), punct(Punct::RParen));
    let tuple_index = select! { Token::Int(s) => s }
        .map_with(|name, e| Ident { name, raw: false, span: Loc(e.span()) });
    // `0.1` after a dot: two tuple indexes lexed as one float.
    let nested_tuple_index = select! { Token::Float { raw, .. } => raw }.try_map(|raw: String, span: Span| {
        match raw.split_once('.') {
            Some((a, b))
                if !a.is_empty()
                    && !b.is_empty()
                    && a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit()) =>
            {
                let mid = span.start + a.len();
                Ok((
                    Ident { name: a.to_string(), raw: false, span: Loc(Span::new(span.start, mid)) },
                    Ident { name: b.to_string(), raw: false, span: Loc(Span::new(mid + 1, span.end)) },
                ))
            }
            _ => Err(Rich::custom(
                span,
                Custom { code: "E0900", message: format!("`{raw}` is not a tuple index"), help: None },
            )),
        }
    });
    let method_or_field = punct(Punct::Dot).ignore_then(first_of(vec![
        ident()
            .then(ty::generic_args(h, true).or_not())
            .then(args.clone())
            .map(|((name, g), a)| Post::Method(name, g, a))
            .boxed(),
        ident().or(tuple_index).map(Post::Field).boxed(),
        nested_tuple_index.map(|(a, b)| Post::Field2(a, b)).boxed(),
    ]));
    let call = args.clone().then(if allow_struct { field_inits.clone().or_not().boxed() } else {
        empty().to(None).boxed()
    }).map(|(a, p)| Post::Call(a, p));
    let index = expr
        .clone()
        .delimited_by(punct(Punct::LBracket), punct(Punct::RBracket))
        .map(Post::Index);
    let postfix_op = first_of(vec![
        method_or_field.boxed(),
        call.boxed(),
        index.boxed(),
        punct(Punct::Question).to(Post::Try).boxed(),
    ]);
    let op = |p: Punct, b: BinOp| punct(p).to(b);

    // ---- operators: one Pratt parser over the atom ----
    //
    // One precedence-climbing loop replaces a parser per precedence level, so
    // a nesting level costs one frame chain rather than a dozen (T16).
    let unary_op = choice((
        punct(Punct::Minus).to(UnOp::Neg),
        punct(Punct::Not).to(UnOp::Not),
        punct(Punct::Star).to(UnOp::Deref),
        punct(Punct::And).to(UnOp::Ref),
        // `&&x` is removed (B.3.6): reported, read as one borrow.
        punct(Punct::AndAnd).validate(|_, e, emitter| {
            emitter.emit(removed("double borrows", e.span()));
            UnOp::Ref
        }),
    ));
    let mul_op = choice((op(Punct::Star, BinOp::Mul), op(Punct::Slash, BinOp::Div), op(Punct::Percent, BinOp::Rem)));
    let add_op = choice((op(Punct::Plus, BinOp::Add), op(Punct::Minus, BinOp::Sub)));
    let shift_op = choice((op(Punct::Shl, BinOp::Shl), gt_op(GtOp::Shr).to(BinOp::Shr)));
    let cmp_op = choice((
        op(Punct::EqEq, BinOp::Eq),
        op(Punct::Ne, BinOp::Ne),
        op(Punct::Le, BinOp::Le),
        op(Punct::Lt, BinOp::Lt),
        gt_op(GtOp::Ge).to(BinOp::Ge),
        gt_op(GtOp::Gt).to(BinOp::Gt),
    ));
    let or = atom
        .pratt((
            postfix(15, postfix_op, apply_post::<I>),
            prefix(14, unary_op, |op, rhs: Expr, e| mk(ExprKind::Unary { op, expr: boxed(rhs) }, e.span())),
            postfix(13, kw(Kw::As).ignore_then(h.ty.clone()).then_ignore(next_not_angle()), |lhs: Expr, ty, e| {
                mk(ExprKind::Cast { expr: boxed(lhs), ty }, e.span())
            }),
            infix(left(12), mul_op, binary::<I>),
            infix(left(11), add_op, binary::<I>),
            infix(left(10), shift_op, binary::<I>),
            infix(left(9), op(Punct::And, BinOp::BitAnd), binary::<I>),
            infix(left(8), op(Punct::Caret, BinOp::BitXor), binary::<I>),
            infix(left(7), op(Punct::Or, BinOp::BitOr), binary::<I>),
            infix(none(6), cmp_op, binary::<I>),
            infix(left(5), op(Punct::AndAnd, BinOp::And), binary::<I>),
            infix(left(4), op(Punct::OrOr, BinOp::Or), binary::<I>),
        ))
        .boxed();

    // ---- range, assignment ----
    let range_op = choice((punct(Punct::DotDotEq).to(true), punct(Punct::DotDot).to(false)));
    let prefix_range = range_op
        .clone()
        .then(or.clone().or_not())
        .map_with(|(inclusive, hi), e| {
            mk(ExprKind::Range { lo: None, hi: hi.map(boxed), inclusive }, e.span())
        })
        .boxed();
    let infix_range = or
        .clone()
        .then(range_op.then(or.clone().or_not()).or_not())
        .map_with(|(lo, tail), e| match tail {
            None => lo,
            Some((inclusive, hi)) => {
                mk(ExprKind::Range { lo: Some(boxed(lo)), hi: hi.map(boxed), inclusive }, e.span())
            }
        })
        .boxed();
    let range = first_of(vec![prefix_range, infix_range]);

    let assign_op = choice((
        punct(Punct::Eq).to(None),
        punct(Punct::PlusEq).to(Some(BinOp::Add)),
        punct(Punct::MinusEq).to(Some(BinOp::Sub)),
        punct(Punct::StarEq).to(Some(BinOp::Mul)),
        punct(Punct::SlashEq).to(Some(BinOp::Div)),
        punct(Punct::PercentEq).to(Some(BinOp::Rem)),
        punct(Punct::CaretEq).to(Some(BinOp::BitXor)),
        punct(Punct::AndEq).to(Some(BinOp::BitAnd)),
        punct(Punct::OrEq).to(Some(BinOp::BitOr)),
        punct(Punct::ShlEq).to(Some(BinOp::Shl)),
        gt_op(GtOp::ShrEq).to(Some(BinOp::Shr)),
    ));
    range
        .then(assign_op.then(me).or_not())
        .map_with(|(lhs, tail), e| match tail {
            None => lhs,
            Some((op, rhs)) => mk(ExprKind::Assign { op, lhs: boxed(lhs), rhs: boxed(rhs) }, e.span()),
        })
        .boxed()
}

/// `if`, `match`, `while`, `loop`, `for` and a plain block: expressions that
/// end at their closing brace, so a statement needs no `;` after them.
pub(crate) fn block_like<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Expr> {
    let block = h.block.clone();
    let block_expr = block.clone().map_with(|b: Block, e| mk(ExprKind::Block(b), e.span()));

    // `let pat = expr` or an expression, with no struct literal at the top.
    let cond = first_of(vec![
        kw(Kw::Let)
            .ignore_then(h.pat.clone())
            .then_ignore(punct(Punct::Eq))
            .then(h.expr_ns.clone())
            .map_with(|(pat, e), sp| mk(ExprKind::Let { pat, expr: boxed(e) }, sp.span()))
            .boxed(),
        h.expr_ns.clone().boxed(),
    ]);

    let if_expr = recursive(|if_expr| {
        kw(Kw::If)
            .ignore_then(cond.clone())
            .then(block.clone())
            .then(kw(Kw::Else).ignore_then(first_of(vec![if_expr.boxed(), block_expr.clone().boxed()])).or_not())
            .map_with(|((cond, then), else_), e| {
                mk(ExprKind::If { cond: boxed(cond), then, else_: else_.map(boxed) }, e.span())
            })
    });

    let arm = item::meta(h)
        .then(h.pat.clone())
        .then(kw(Kw::If).ignore_then(h.expr.clone()).or_not())
        .then_ignore(punct(Punct::FatArrow))
        .then(choice((
            // A block-like body needs no comma.
            block_expr
                .clone()
                .then_ignore(punct(Punct::Comma).or_not()),
            h.expr
                .clone()
                .then_ignore(choice((punct(Punct::Comma), punct(Punct::RBrace).rewind()))),
        )))
        .map_with(|(((meta, pat), guard), body), e| Arm {
            meta,
            pat,
            guard,
            body,
            span: Loc(e.span()),
        });
    let match_expr = kw(Kw::Match)
        .ignore_then(h.expr_ns.clone())
        .then(arm.repeated().collect::<Vec<_>>().delimited_by(punct(Punct::LBrace), punct(Punct::RBrace)))
        .map_with(|(scrutinee, arms), e| {
            mk(ExprKind::Match { scrutinee: boxed(scrutinee), arms }, e.span())
        });

    let while_expr = kw(Kw::While)
        .ignore_then(cond)
        .then(block.clone())
        .map_with(|(cond, body), e| mk(ExprKind::While { cond: boxed(cond), body }, e.span()));
    let loop_expr = kw(Kw::Loop)
        .ignore_then(block.clone())
        .map_with(|body, e| mk(ExprKind::Loop { body }, e.span()));
    let for_expr = kw(Kw::For)
        .ignore_then(pat::binding(h))
        .then_ignore(kw(Kw::In))
        .then(h.expr_ns.clone())
        .then(block)
        .map_with(|((pat, iter), body), e| {
            mk(ExprKind::For { pat, iter: boxed(iter), body }, e.span())
        });

    first_of(vec![if_expr.boxed(), match_expr.boxed(), while_expr.boxed(), loop_expr.boxed(), for_expr.boxed(), block_expr.boxed()])
}

/// A block: `{ stmt* }`.
pub(crate) fn block<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Block> {
    stmt(h)
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace))
        .map_with(|stmts, e| Block { stmts, span: Loc(e.span()) })
        .boxed()
}

/// One statement: `let`, `;`, a block-like expression, an expression
/// statement, a `contribution` (`e +> e;`) or a `commit` (`place <- e
/// [after e];`). The last statement of a block may omit its `;`.
pub(crate) fn stmt<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Stmt> {
    let mk = |kind: StmtKind, span: Span| Stmt { meta: Meta::default(), kind, span: Loc(span) };
    let semi = punct(Punct::Semi);

    let empty = semi.clone().map_with(move |_, e| mk(StmtKind::Empty, e.span()));

    let let_stmt = kw(Kw::Let)
        .ignore_then(pat::binding(h))
        .then(punct(Punct::Colon).ignore_then(h.ty.clone()).or_not())
        .then(
            punct(Punct::Eq)
                .ignore_then(h.expr.clone())
                // `else` is matched without listing it among the expected tokens: it is
                // rare enough that an error after `let x = 1` names `;` first.
                .then(select! { Token::Kw(Kw::Else) => () }.ignore_then(h.block.clone()).or_not())
                .or_not(),
        )
        .then_ignore(semi.clone())
        .map_with(move |((pat, ty), tail), e| {
            let (init, else_) = match tail {
                Some((init, else_)) => (Some(init), else_),
                None => (None, None),
            };
            mk(StmtKind::Let { pat, ty, init, else_ }, e.span())
        });

    // Which of the two statement forms applies is decided by the first token,
    // so a statement is never parsed twice (that would cost 2^depth when an
    // error sits deep inside nested blocks).
    let starts_block_like = |t: &Token| {
        matches!(
            t,
            Token::Kw(Kw::If | Kw::Match | Kw::While | Kw::Loop | Kw::For)
                | Token::Punct(Punct::LBrace)
        )
    };
    let block_like_stmt = any()
        .filter(starts_block_like)
        .rewind()
        .ignore_then(block_like(h))
        .then(semi.clone().or_not())
        .map_with(move |(expr, semi), e| mk(StmtKind::Expr { expr, semi: semi.is_some() }, e.span()));

    #[derive(Clone)]
    enum Tail {
        Contribution(Expr),
        Commit(Expr, Option<Expr>),
        Semi,
        Last,
    }
    let tail = first_of(vec![
        punct(Punct::PlusGt)
            .ignore_then(h.expr.clone())
            .then_ignore(semi.clone())
            .map(Tail::Contribution)
            .boxed(),
        punct(Punct::LtMinus)
            .ignore_then(h.expr.clone())
            .then(kw(Kw::After).ignore_then(h.expr.clone()).or_not())
            .then_ignore(semi.clone())
            .map(|(v, after)| Tail::Commit(v, after))
            .boxed(),
        semi.to(Tail::Semi).boxed(),
        punct(Punct::RBrace).rewind().to(Tail::Last).boxed(),
    ]);
    let simple = any()
        .filter(move |t: &Token| !starts_block_like(t))
        .rewind()
        .ignore_then(h.expr.clone())
        .then(tail).map_with(move |(lhs, tail), e| {
        let kind = match tail {
            Tail::Contribution(rhs) => StmtKind::Contribution { lhs, rhs },
            Tail::Commit(value, after) => StmtKind::Commit { place: lhs, value, after },
            Tail::Semi => StmtKind::Expr { expr: lhs, semi: true },
            Tail::Last => StmtKind::Expr { expr: lhs, semi: false },
        };
        mk(kind, e.span())
    });

    // `signal` and `on` are module and `digital` forms: parsed here so a
    // context check can name them where they are not allowed.
    let signal_stmt = kw(Kw::Signal)
        .ignore_then(ident())
        .then_ignore(punct(Punct::Colon))
        .then(ty::sigtype(h))
        .then(punct(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .then_ignore(punct(Punct::Semi))
        .map_with(move |((name, ty), init), e| mk(StmtKind::Signal { name, ty, init }, e.span()));
    let reaction_stmt = kw(Kw::On)
        .ignore_then(h.expr_ns.clone())
        .then(h.block.clone())
        .map_with(move |(event, body), e| mk(StmtKind::Reaction(Reaction { event, body }), e.span()));

    // A Rust item inside a block starts with an item keyword, after any
    // doc comments and attributes (which are then the item's own).
    let item_keyword = any().filter(|t: &Token| {
        matches!(
            t,
            Token::Kw(Kw::Const | Kw::Fn | Kw::Use | Kw::Struct | Kw::Enum | Kw::Impl | Kw::Trait | Kw::Mod | Kw::Module | Kw::Pub | Kw::Extern | Kw::Type | Kw::Static)
        )
    });
    let item_stmt = item::meta(h)
        .ignore_then(item_keyword)
        .rewind()
        .ignore_then(h.item.clone())
        .map_with(move |item, e| mk(StmtKind::Item(Box::new(item)), e.span()));

    // Every other form takes outer docs and attributes (Rust's `stmt`).
    let with_meta = item::meta(h)
        .then(first_of(vec![
            empty.boxed(),
            let_stmt.boxed(),
            signal_stmt.boxed(),
            reaction_stmt.boxed(),
            block_like_stmt.boxed(),
            simple.boxed(),
        ]))
        .map(|(meta, mut s): (Meta, Stmt)| {
            s.meta = meta;
            s
        });
    let stmt = first_of(vec![item_stmt.boxed(), with_meta.boxed()])
    .labelled("statement")
    .boxed();
    recover(stmt, false, |span| Stmt {
        meta: Meta::default(),
        kind: StmtKind::Expr { expr: Expr { kind: ExprKind::Error, span: Loc(span) }, semi: true },
        span: Loc(span),
    })
}
