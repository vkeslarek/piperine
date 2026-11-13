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

    let field_init = ident()
        .then_ignore(punct(Punct::Colon))
        .then(expr.clone())
        .map_with(|(name, value), e| FieldInit { name, value, span: Loc(e.span()) });
    let field_inits = field_init
        .separated_by(punct(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(punct(Punct::LBrace), punct(Punct::RBrace));

    let emit = select! { Token::Ident(s) if s == "emit" => () }
        .ignore_then(punct(Punct::Not))
        .ignore_then(
            lit_string()
                .then(
                    punct(Punct::Comma)
                        .ignore_then(
                            ident()
                                .then_ignore(punct(Punct::Eq))
                                .then(expr.clone())
                                .map_with(|(name, value), e| FieldInit {
                                    name,
                                    value,
                                    span: Loc(e.span()),
                                }),
                        )
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(punct(Punct::Comma).or_not())
                .delimited_by(punct(Punct::LParen), punct(Punct::RParen)),
        )
        .map_with(|(stream, fields), e| mk(ExprKind::Emit { stream, fields }, e.span()));
    let include_str = select! { Token::Ident(s) if s == "include_str" => () }
        .ignore_then(punct(Punct::Not))
        .ignore_then(lit_string().delimited_by(punct(Punct::LParen), punct(Punct::RParen)))
        .map_with(|s, e| mk(ExprKind::IncludeStr(s), e.span()));

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
    let closure_param = h
        .pat1
        .clone()
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
        .or_not()
        .then(closure_params)
        .then(
            punct(Punct::Arrow)
                .ignore_then(h.ty.clone())
                .then(h.block.clone().map_with(|b: Block, e| {
                    mk(ExprKind::Block(b), e.span())
                }))
                .map(|(ty, body)| (Some(ty), body))
                .or(me.clone().map(|body| (None, body))),
        )
        .map_with(|((mv, params), (ret, body)), e| {
            mk(
                ExprKind::Closure { is_move: mv.is_some(), params, ret, body: boxed(body) },
                e.span(),
            )
        });

    let atom = choice((
        lit_expr(),
        emit,
        include_str,
        paren_or_tuple,
        array,
        block_like(h),
        break_,
        continue_,
        return_,
        closure,
        error_atom,
        path_atom,
    ))
    .labelled("expression")
    .boxed();

    // ---- postfix ----
    #[derive(Clone)]
    enum Post {
        Field(Ident),
        Method(Ident, Option<GenericArgs>, Vec<Arg>),
        Call(Vec<Arg>, Option<Vec<FieldInit>>),
        Index(Expr),
        Try,
    }
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
        .map_with(|name, e| Ident { name, span: Loc(e.span()) });
    let method_or_field = punct(Punct::Dot).ignore_then(choice((
        ident()
            .then(ty::generic_args(h, true).or_not())
            .then(args.clone())
            .map(|((name, g), a)| Post::Method(name, g, a)),
        ident().or(tuple_index).map(Post::Field),
    )));
    let call = args.clone().then(if allow_struct { field_inits.clone().or_not().boxed() } else {
        empty().to(None).boxed()
    }).map(|(a, p)| Post::Call(a, p));
    let index = expr
        .clone()
        .delimited_by(punct(Punct::LBracket), punct(Punct::RBracket))
        .map(Post::Index);
    let postfix = choice((method_or_field, call, index, punct(Punct::Question).to(Post::Try)));

    let postfixed = atom
        .foldl_with(postfix.repeated(), |lhs, post, e| {
            let span = e.span();
            let kind = match post {
                Post::Field(name) => ExprKind::Field { expr: boxed(lhs), name },
                Post::Method(method, generics, args) => {
                    ExprKind::MethodCall { recv: boxed(lhs), method, generics, args }
                }
                Post::Call(args, params) => ExprKind::Call { callee: boxed(lhs), args, params },
                Post::Index(i) => ExprKind::Index { expr: boxed(lhs), index: boxed(i) },
                Post::Try => ExprKind::Try(boxed(lhs)),
            };
            mk(kind, span)
        })
        .boxed();

    // ---- unary, cast ----
    let unary_op = choice((
        punct(Punct::Minus).to(UnOp::Neg),
        punct(Punct::Not).to(UnOp::Not),
        punct(Punct::Star).to(UnOp::Deref),
        punct(Punct::And).to(UnOp::Ref),
    ))
    .map_with(|op, e| (op, e.span()));
    let unary = unary_op
        .repeated()
        .foldr(postfixed, |(op, sp), rhs| {
            let span = sp.to(rhs.span.0);
            mk(ExprKind::Unary { op, expr: boxed(rhs) }, span)
        })
        .boxed();

    let cast = unary
        .foldl_with(kw(Kw::As).ignore_then(h.ty.clone()).repeated(), |lhs, ty, e| {
            mk(ExprKind::Cast { expr: boxed(lhs), ty }, e.span())
        })
        .boxed();

    // ---- binary levels ----
    let level = |operand: P<'a, I, Expr>, op: P<'a, I, BinOp>| -> P<'a, I, Expr> {
        operand
            .clone()
            .foldl_with(op.then(operand).repeated(), |lhs, (op, rhs), e| {
                mk(ExprKind::Binary { op, lhs: boxed(lhs), rhs: boxed(rhs) }, e.span())
            })
            .boxed()
    };
    let op = |p: Punct, b: BinOp| punct(p).to(b);

    let mul = level(
        cast,
        choice((op(Punct::Star, BinOp::Mul), op(Punct::Slash, BinOp::Div), op(Punct::Percent, BinOp::Rem)))
            .boxed(),
    );
    let add = level(mul, choice((op(Punct::Plus, BinOp::Add), op(Punct::Minus, BinOp::Sub))).boxed());
    let shift = level(
        add,
        choice((op(Punct::Shl, BinOp::Shl), gt_op(GtOp::Shr).to(BinOp::Shr))).boxed(),
    );
    let bitand = level(shift, op(Punct::And, BinOp::BitAnd).boxed());
    let bitxor = level(bitand, op(Punct::Caret, BinOp::BitXor).boxed());
    let bitor = level(bitxor, op(Punct::Or, BinOp::BitOr).boxed());
    let cmp = level(
        bitor,
        choice((
            op(Punct::EqEq, BinOp::Eq),
            op(Punct::Ne, BinOp::Ne),
            op(Punct::Le, BinOp::Le),
            op(Punct::Lt, BinOp::Lt),
            gt_op(GtOp::Ge).to(BinOp::Ge),
            gt_op(GtOp::Gt).to(BinOp::Gt),
        ))
        .boxed(),
    );
    let and = level(cmp, op(Punct::AndAnd, BinOp::And).boxed());
    let or = level(and, op(Punct::OrOr, BinOp::Or).boxed());

    // ---- range, assignment ----
    let range_op = choice((punct(Punct::DotDotEq).to(true), punct(Punct::DotDot).to(false)));
    let range = range_op
        .clone()
        .then(or.clone().or_not())
        .map_with(|(inclusive, hi), e| {
            mk(ExprKind::Range { lo: None, hi: hi.map(boxed), inclusive }, e.span())
        })
        .or(or
            .clone()
            .then(range_op.then(or.clone().or_not()).or_not())
            .map_with(|(lo, tail), e| match tail {
                None => lo,
                Some((inclusive, hi)) => {
                    mk(ExprKind::Range { lo: Some(boxed(lo)), hi: hi.map(boxed), inclusive }, e.span())
                }
            }))
        .boxed();

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

/// A string literal token as a [`Lit`].
fn lit_string<'a, I: In<'a>>() -> impl Parser<'a, I, Lit, Extra<'a>> + Clone {
    select! { Token::Str(s) => LitKind::Str(s) }
        .map_with(|kind, e| Lit { kind, span: Loc(e.span()) })
        .labelled("string literal")
}

/// `if`, `match`, `while`, `loop`, `for` and a plain block: expressions that
/// end at their closing brace, so a statement needs no `;` after them.
pub(crate) fn block_like<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Expr> {
    let block = h.block.clone();
    let block_expr = block.clone().map_with(|b: Block, e| mk(ExprKind::Block(b), e.span()));

    // `let pat = expr` or an expression, with no struct literal at the top.
    let cond = choice((
        kw(Kw::Let)
            .ignore_then(h.pat.clone())
            .then_ignore(punct(Punct::Eq))
            .then(h.expr_ns.clone())
            .map_with(|(pat, e), sp| mk(ExprKind::Let { pat, expr: boxed(e) }, sp.span())),
        h.expr_ns.clone(),
    ));

    let if_expr = recursive(|if_expr| {
        kw(Kw::If)
            .ignore_then(cond.clone())
            .then(block.clone())
            .then(kw(Kw::Else).ignore_then(if_expr.or(block_expr.clone())).or_not())
            .map_with(|((cond, then), else_), e| {
                mk(ExprKind::If { cond: boxed(cond), then, else_: else_.map(boxed) }, e.span())
            })
    });

    let arm = h
        .pat
        .clone()
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
        .map_with(|((pat, guard), body), e| Arm {
            meta: Meta::default(),
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
        .ignore_then(h.pat1.clone())
        .then_ignore(kw(Kw::In))
        .then(h.expr_ns.clone())
        .then(block)
        .map_with(|((pat, iter), body), e| {
            mk(ExprKind::For { pat, iter: boxed(iter), body }, e.span())
        });

    choice((if_expr, match_expr, while_expr, loop_expr, for_expr, block_expr)).boxed()
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
    let mk = |kind: StmtKind, span: Span| Stmt { kind, span: Loc(span) };
    let semi = punct(Punct::Semi);

    let empty = semi.clone().map_with(move |_, e| mk(StmtKind::Empty, e.span()));

    let let_stmt = kw(Kw::Let)
        .ignore_then(h.pat1.clone())
        .then(punct(Punct::Colon).ignore_then(h.ty.clone()).or_not())
        .then(punct(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .then_ignore(semi.clone())
        .map_with(move |((pat, ty), init), e| mk(StmtKind::Let { pat, ty, init }, e.span()));

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
    let tail = choice((
        punct(Punct::PlusGt)
            .ignore_then(h.expr.clone())
            .then_ignore(semi.clone())
            .map(Tail::Contribution),
        punct(Punct::LtMinus)
            .ignore_then(h.expr.clone())
            .then(kw(Kw::After).ignore_then(h.expr.clone()).or_not())
            .then_ignore(semi.clone())
            .map(|(v, after)| Tail::Commit(v, after)),
        semi.to(Tail::Semi),
        punct(Punct::RBrace).rewind().to(Tail::Last),
    ));
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

    choice((empty, let_stmt, block_like_stmt, simple)).labelled("statement").boxed()
}
