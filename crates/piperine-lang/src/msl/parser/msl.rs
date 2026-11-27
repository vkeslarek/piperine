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
    let attr = TokenInput::identifier()
        .then_ignore(TokenInput::punctuation(Punct::Colon))
        .then(h.expr.clone())
        .map_with(|(name, value), e| FieldInit { name, value, shorthand: false, span: Loc(e.span()) });
    let attr_block = attr
        .separated_by(TokenInput::punctuation(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace));
    let quantity = choice((
        word("potential").to(Quantity::Potential),
        word("flow").to(Quantity::Flow),
    ))
    .then_ignore(TokenInput::punctuation(Punct::Colon))
    .then(TokenInput::identifier())
    .then(attr_block.or_not())
    .map(|((which, name), attrs)| NatureFieldKind::Quantity {
        which,
        name,
        attrs: attrs.unwrap_or_default(),
    });
    let value = word("value")
        .ignore_then(TokenInput::punctuation(Punct::Colon))
        .ignore_then(h.ty.clone())
        .map(NatureFieldKind::Value);
    let field = item::meta(h)
        .then(TokenInput::first_of(vec![quantity.boxed(), value.boxed()]))
        .map_with(|(meta, kind), e| NatureField { meta, kind, span: Loc(e.span()) })
        .labelled("nature field");
    TokenInput::keyword(Kw::Nature)
        .ignore_then(TokenInput::identifier())
        .then(ty::generics(h).or_not())
        .then(
            field
                .separated_by(TokenInput::punctuation(Punct::Comma))
                .at_least(1)
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace)),
        )
        .map(|((name, generics), fields)| ItemKind::Nature(NatureDecl { name, generics, fields }))
        .boxed()
}

// ---- module items -----------------------------------------------------------------------------

/// `signal name: sigtype = init;`
fn signal_decl<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ModuleItemKind> {
    TokenInput::keyword(Kw::Signal)
        .ignore_then(TokenInput::identifier())
        .then_ignore(TokenInput::punctuation(Punct::Colon))
        .then(ty::sigtype(h))
        .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .then_ignore(TokenInput::punctuation(Punct::Semi))
        .map(|((name, ty), init)| ModuleItemKind::Signal { name, ty, init })
        .boxed()
}

/// A `place`: a path followed by `[expr]` and `.name` steps. It names a
/// carrier, an element of one or a field of one.
fn place<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Expr> {
    #[derive(Clone)]
    enum Step {
        Index(Expr),
        Field(Ident),
    }
    let step = TokenInput::first_of(vec![
        h.expr
            .clone()
            .delimited_by(TokenInput::punctuation(Punct::LBracket), TokenInput::punctuation(Punct::RBracket))
            .map(Step::Index)
            .boxed(),
        TokenInput::punctuation(Punct::Dot).ignore_then(TokenInput::identifier()).map(Step::Field).boxed(),
    ]);
    // A parenthesized place is parsed so that its one diagnostic names it.
    let paren = h
        .expr
        .clone()
        .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
        .validate(|inner, e, emitter| {
            let span = e.span();
            emitter.emit(Rich::custom(span, ParseDiagnostics::not_a_place()));
            Expr { span: Loc(span), kind: ExprKind::Paren(Box::new(inner)) }
        });
    let head = TokenInput::first_of(vec![
        ty::path(h, true).map_with(|path, e| Expr { span: Loc(e.span()), kind: ExprKind::Path(path) }).boxed(),
        paren.boxed(),
    ]);
    head
        .foldl_with(step.repeated(), |lhs, step, e| {
            let kind = match step {
                Step::Index(i) => ExprKind::Index { expr: Box::new(lhs), index: Box::new(i) },
                Step::Field(name) => ExprKind::Field { expr: Box::new(lhs), name },
            };
            Expr { kind, span: Loc(e.span()) }
        })
        .boxed()
}

/// `gen<X>`: generation `for` and `if` over params, repeating the production
/// `leaf`. The result parses `X` itself: a leaf, or a generation wrapped by
/// `wrap` into an `X`.
fn gen_of<'a, I: In<'a>, X: Clone + 'a>(
    h: &Handles<'a, I>,
    leaf: P<'a, I, X>,
    late: Option<P<'a, I, X>>,
    wrap: fn(Gen<X>, Meta) -> X,
) -> P<'a, I, X> {
    // A generation takes outer docs and attributes like any item.
    let meta = item::meta(h);
    // `late` is tried after a generation: a bare statement that `E0434`
    // names, inside a generation's list too (`else { y = 2; }`).
    let with_late = |mut alts: Vec<P<'a, I, X>>| {
        alts.extend(late.clone());
        TokenInput::first_of(alts)
    };
    let generation = recursive(|generation: Recursive<chumsky::recursive::Direct<'a, 'a, I, Gen<X>, Extra<'a>>>| {
        let item = with_late(vec![
            leaf.clone(),
            meta.clone().then(generation.clone()).map(move |(m, g)| wrap(g, m)).boxed(),
        ]);
        let list = item
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace))
            .boxed();
        let for_ = TokenInput::keyword(Kw::For)
            .ignore_then(pat::binding(h))
            .then_ignore(TokenInput::keyword(Kw::In))
            .then(h.expr_ns.clone())
            .then(list.clone())
            .map_with(|((pat, iter), body), e| Gen {
                kind: GenKind::For { pat, iter, body },
                span: Loc(e.span()),
            });
        let else_ = TokenInput::keyword(Kw::Else).ignore_then(TokenInput::first_of(vec![
            list.clone().map(GenElse::Block).boxed(),
            generation.clone().map(GenElse::If).boxed(),
        ]));
        let if_ = TokenInput::keyword(Kw::If)
            .ignore_then(h.expr_ns.clone())
            .then(list)
            .then(else_.or_not())
            .map_with(|((cond, then), else_), e| Gen {
                kind: GenKind::If { cond, then, else_: else_.map(Box::new) },
                span: Loc(e.span()),
            });
        TokenInput::first_of(vec![for_.boxed(), if_.boxed()])
    });
    with_late(vec![leaf, meta.then(generation).map(move |(m, g)| wrap(g, m)).boxed()])
}

/// Looks ahead, without consuming, for a statement that holds `+>` or `<-` at
/// its own level before its `;`. It reads tokens only, so a failure is
/// positioned at the item's start and never outweighs another alternative's.
fn statement_with_arrow<'a, I: In<'a>>() -> impl Parser<'a, I, (), Extra<'a>> + Clone {
    custom(|inp| {
        let marker = inp.save();
        let start = inp.cursor();
        let (mut depth, mut found) = (0usize, false);
        while let Some(t) = inp.next() {
            match t {
                Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace) => depth += 1,
                Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace) => match depth.checked_sub(1) {
                    Some(d) => depth = d,
                    None => break,
                },
                Token::Punct(Punct::PlusGt | Punct::LtMinus) if depth == 0 => {
                    found = true;
                    break;
                }
                Token::Punct(Punct::Semi) if depth == 0 => break,
                _ => {}
            }
        }
        let span = inp.span_since(&start);
        inp.rewind(marker);
        if found {
            Ok(())
        } else {
            // Expects nothing, so it adds nothing to the message of the alternatives beside it.
            Err(<Err<'a> as chumsky::label::LabelError<'a, I, chumsky::DefaultExpected<'a, Token>>>::expected_found(
                std::iter::empty(),
                None,
                span,
            ))
        }
    })
}

/// One item of a module body, with error recovery.
pub(crate) fn module_item<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ModuleItem> {
    recover_item(h, None)
}

fn recover_item<'a, I: In<'a>>(h: &Handles<'a, I>, unless: Option<P<'a, I, ()>>) -> P<'a, I, ModuleItem> {
    // `name: type = default where cond;`
    let param = TokenInput::identifier()
        .then_ignore(TokenInput::punctuation(Punct::Colon))
        .then(h.ty.clone())
        .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .then(TokenInput::keyword(Kw::Where).ignore_then(h.expr.clone()).or_not())
        .then_ignore(TokenInput::punctuation(Punct::Semi))
        .map(|(((name, ty), default), where_)| ModuleItemKind::Param { name, ty, default, where_ })
        .boxed();
    // `let pat = expr;`: a body `let` takes no type annotation.
    let annotation = TokenInput::punctuation(Punct::Colon).ignore_then(h.ty.clone()).map_with(|_, e| e.span()).or_not();
    let let_item = TokenInput::keyword(Kw::Let)
        .ignore_then(pat::binding(h))
        .then(annotation)
        .then_ignore(TokenInput::punctuation(Punct::Eq))
        .then(h.expr.clone())
        .then_ignore(TokenInput::punctuation(Punct::Semi))
        .validate(|((pat, annotation), value), _, emitter| {
            if let Some(span) = annotation {
                emitter.emit(Rich::custom(
                    span,
                    Custom {
                        code: "E0900",
                        message: "a body `let` takes no type annotation".into(),
                        help: Some("its type is that of its expression: write `let y = 1.0;`".into()),
                    },
                ));
            }
            ModuleItemKind::Let { pat, value }
        })
        .boxed();
    // `place = expr [after expr];`
    let def = place(h)
        .then_ignore(TokenInput::punctuation(Punct::Eq))
        .then(h.expr.clone())
        .then(TokenInput::keyword(Kw::After).ignore_then(h.expr.clone()).or_not())
        .then_ignore(TokenInput::punctuation(Punct::Semi))
        .map(|((place, value), after)| ModuleItemKind::Def { place, value, after })
        .boxed();
    // E0303: a module declaration is not a module item.
    let nested = any()
        .filter(|t: &Token| matches!(t, Token::Kw(Kw::Module | Kw::Extern)))
        .rewind()
        .ignore_then(h.item.clone())
        .validate(|item, _, emitter| {
            emitter.emit(Rich::custom(
                item.span.0,
                Custom {
                    code: "E0303",
                    message: "a module declaration inside a module body".into(),
                    help: Some("modules are declared at file scope; instantiate the inner one instead".into()),
                },
            ));
            ModuleItemKind::Error
        })
        .boxed();
    let declared = TokenInput::first_of(vec![signal_decl(h), param, let_item, nested]);
    let leaf = item::meta(h)
        .then(TokenInput::visibility().or_not())
        .then(declared)
        .map_with(|((meta, public), kind), e| ModuleItem {
            meta,
            public: public.is_some(),
            kind,
            span: Loc(e.span()),
        })
        .boxed();
    let def = item::meta(h)
        .then(def)
        .map_with(|(meta, kind), e| ModuleItem { meta, public: false, kind, span: Loc(e.span()) })
        .boxed();
    let wrap = |g: Gen<ModuleItem>, meta: Meta| ModuleItem {
        span: g.span,
        meta,
        public: false,
        kind: ModuleItemKind::Gen(g),
    };
    // A commit, a contribution or a reaction written where only items go:
    // parsed as the statement it is, so one diagnostic names it (E0404).
    let starts_behavior = TokenInput::first_of(vec![
        any().filter(|t: &Token| matches!(t, Token::Kw(Kw::On))).rewind().ignored().boxed(),
        statement_with_arrow().boxed(),
        select! { Token::Ident(s) if s == "bound_step" => () }.then(TokenInput::punctuation(Punct::LParen)).rewind().ignored().boxed(),
        select! { Token::Ident(s) if s == "emit" => () }.then(TokenInput::punctuation(Punct::Not)).rewind().ignored().boxed(),
    ]);
    let stray = starts_behavior
        .ignore_then(expr::stmt(h))
        .validate(|s, e, emitter| {
            if matches!(&s.kind, StmtKind::Expr { expr, .. } if matches!(expr.kind, ExprKind::Emit { .. })) {
                // an `emit!` statement: the output runs when behavior runs (§19 rule 10)
                emitter.emit(misplaced_emit(s.span.0));
                return ModuleItem { meta: Meta::default(), public: false, kind: ModuleItemKind::Error, span: Loc(e.span()) };
            }
            let what = match &s.kind {
                StmtKind::Commit { .. } => "a commit (`<-`)",
                StmtKind::Contribution { .. } => "a contribution (`+>`)",
                StmtKind::Reaction(_) => "`on`",
                _ => "`bound_step`",
            };
            let d = check::misplaced(
                check::Ctx::Module,
                what,
                s.span.0,
                "",
                "a commit belongs in a reaction (`on event { .. }`) of a `digital` block, a contribution or a `bound_step` call in an `analog` block; a module body holds declarations and definitions",
            );
            emitter.emit(Rich::custom(
                s.span.0,
                Custom { code: "E0404", message: d.message.clone(), help: d.help.clone() },
            ));
            ModuleItem { meta: Meta::default(), public: false, kind: ModuleItemKind::Error, span: Loc(e.span()) }
        })
        .boxed();
    let item = gen_of(h, TokenInput::first_of(vec![leaf, def]), Some(stray), wrap).labelled("module item").boxed();
    Recovery::recover_unless(
        item,
        true,
        |span| ModuleItem { meta: Meta::default(), public: false, kind: ModuleItemKind::Error, span: Loc(span) },
        unless,
    )
}

/// The `E0601` of an `emit!` statement written where only items go (a module body or the file's top):
/// the output runs when behavior runs (§19 rule 10).
pub(crate) fn misplaced_emit<'a>(span: Span) -> Err<'a> {
    Rich::custom(
        span,
        Custom {
            code: "E0601",
            message: "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)".into(),
            help: Some("output runs when behavior runs: put it in a reaction, `on init() { emit!(..); }`".into()),
        },
    )
}

// ---- modules ----------------------------------------------------------------------------------

/// `[extern "abi"] module Name<G>(ports) [-> T] [where cond] { items [result] }`
pub(crate) fn module_decl<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ItemKind> {
    let port = item::meta(h)
        .then(item::dir().or_not())
        .then(TokenInput::identifier())
        .then_ignore(TokenInput::punctuation(Punct::Colon))
        .then(h.ty.clone())
        .then(TokenInput::punctuation(Punct::Eq).ignore_then(h.expr.clone()).or_not())
        .map_with(|((((meta, dir), name), ty), default), e| Port {
            meta,
            dir,
            name,
            ty,
            default,
            span: Loc(e.span()),
        });
    // A port that fails to parse is skipped to the next `,` or the closing
    // `)` and dropped: the module keeps its other ports and its body.
    let bad_port = any()
        .filter(|t: &Token| !matches!(t, Token::Punct(Punct::Comma | Punct::RParen | Punct::RBracket | Punct::RBrace)))
        .ignore_then(
            choice((
                any().filter(|t: &Token| matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace))).rewind().ignore_then(Recovery::skip_group()),
                any().filter(|t: &Token| !matches!(t, Token::Punct(Punct::Comma | Punct::RParen | Punct::RBracket | Punct::RBrace))).ignored(),
            ))
            .repeated(),
        )
        .to(None);
    let ports = port
        .map(Some)
        .recover_with(via_parser(bad_port))
        .separated_by(TokenInput::punctuation(Punct::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .map(|ports| ports.into_iter().flatten().collect::<Vec<Port>>())
        .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen));
    // The body's last expression is the result, not an item to skip.
    let result_ahead = h
        .expr
        .clone()
        .then(just(Token::Punct(Punct::RBrace)))
        .rewind()
        .ignored()
        .boxed();
    let body = recover_item(h, Some(result_ahead))
        .repeated()
        .collect::<Vec<_>>()
        .then(h.expr.clone().or_not())
        .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace));
    TokenInput::keyword(Kw::Extern)
        .ignore_then(TokenInput::string_literal())
        .or_not()
        .then_ignore(TokenInput::keyword(Kw::Module))
        .then(TokenInput::identifier())
        .then(ty::generics(h).or_not())
        .then(ports)
        .then(TokenInput::punctuation(Punct::Arrow).ignore_then(h.ty.clone()).or_not())
        .then(item::where_clause(h))
        .then(body)
        .map(|((((((abi, name), generics), ports), ret), where_), (body, result))| {
            ItemKind::Module(ModuleDecl { abi, name, generics, ports, ret, where_, body, result })
        })
        .validate(|kind, _, emitter| {
            // E0302: a result expression needs the `-> T` head (§8 rule 3).
            if let ItemKind::Module(m) = &kind {
                if let (None, Some(result)) = (&m.ret, &m.result) {
                    emitter.emit(Rich::custom(
                        result.span.0,
                        Custom {
                            code: "E0302",
                            message: "a module without `-> T` cannot end with an expression".into(),
                            help: Some("declare the result type: `module M(..) -> T { .. }`".into()),
                        },
                    ));
                }
            }
            kind
        })
        .boxed()
}

// ---- behavior blocks --------------------------------------------------------------------------

/// `on event { stmts and commits }`; the event is an expression that does
/// not take a struct literal, so the `{` opens the body.
fn reaction<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, Reaction> {
    TokenInput::keyword(Kw::On)
        .ignore_then(h.expr_ns.clone())
        .then(h.block.clone())
        .map(|(event, body)| Reaction { event, body })
        .boxed()
}

/// One item of a `digital` block: a reaction or a generation over reactions.
/// A bare statement is parsed so that `E0434` can name it.
fn digital_item<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, DigitalItem> {
    let reaction = item::meta(h)
        .then(reaction(h))
        .map_with(|(meta, r), e| DigitalItem { meta, kind: DigitalItemKind::Reaction(r), span: Loc(e.span()) })
        .boxed();
    let wrap = |g: Gen<DigitalItem>, meta: Meta| DigitalItem { span: g.span, meta, kind: DigitalItemKind::Gen(g) };
    let stray = expr::stmt(h)
        .validate(|_, e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                Custom {
                    code: "E0434",
                    message: "a statement outside a reaction in a `digital` block".into(),
                    help: Some("a `digital` block holds reactions: `on event { .. }`".into()),
                },
            ));
            DigitalItem { meta: Meta::default(), kind: DigitalItemKind::Error, span: Loc(e.span()) }
        })
        .boxed();
    let item = gen_of(h, reaction, Some(stray), wrap).labelled("reaction").boxed();
    Recovery::recover(item, true, |span| DigitalItem { meta: Meta::default(), kind: DigitalItemKind::Error, span: Loc(span) })
}

/// `analog [<names>] for Path { stmts }` or `digital [<names>] for Path { reactions }`.
pub(crate) fn behavior_block<'a, I: In<'a>>(h: &Handles<'a, I>) -> P<'a, I, ItemKind> {
    let names = TokenInput::identifier()
        .separated_by(TokenInput::punctuation(Punct::Comma))
        .at_least(1)
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(TokenInput::punctuation(Punct::Lt), TokenInput::punctuation(Punct::Gt));
    // `analog` and `digital` are keywords only in front of `for` or `<`.
    let head = |name: &'static str| {
        word(name)
            .ignore_then(names.clone().or_not())
            .then_ignore(TokenInput::keyword(Kw::For))
            .then(ty::path(h, false))
    };
    let analog = head("analog")
        .then(
            expr::stmt(h)
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace)),
        )
        .map(|((names, target), stmts)| {
            ItemKind::Behavior(BehaviorBlock {
                kind: BehaviorKind::Analog,
                names: names.unwrap_or_default(),
                target,
                body: BehaviorBody::Analog(stmts),
            })
        })
        .boxed();
    let digital = head("digital")
        .then(
            digital_item(h)
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace)),
        )
        .map(|((names, target), items)| {
            ItemKind::Behavior(BehaviorBlock {
                kind: BehaviorKind::Digital,
                names: names.unwrap_or_default(),
                target,
                body: BehaviorBody::Digital(items),
            })
        })
        .boxed();
    TokenInput::first_of(vec![analog, digital])
}
