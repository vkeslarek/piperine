//! Expressions, blocks and statements.
//!
//! Precedence follows Rust: assignment, range, `||`, `&&`, comparison, `|`,
//! `^`, `&`, shifts, `+ -`, `* / %`, `as`, unary, postfix. `+>` and `<-` are
//! not expression operators: each ends the left side of its statement
//! (Appendix A.5 rule 6), so they appear only in [`ExpressionGrammar::statement`].
//!
//! Two expression parsers exist: the full one and one that refuses a struct
//! literal or instance param block at the top level (`if x { .. }`,
//! `match v { .. }`, `for i in r { .. }`, `on e { .. }`), as in Rust.

use chumsky::input::{Emitter, MapExtra};
use chumsky::pratt::{infix, left, none, postfix, prefix};

use super::*;

/// The expression grammar: expressions, blocks and statements. Each production is a method that
/// returns its parser.
pub struct ExpressionGrammar;

/// One of the operators the lexer spells as `>` tokens that touch: the lexer emits every `>` alone
/// so `Vec<Vec<T>>` needs no splitting, and `>=`, `>>` and `>>=` are joined here.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GreaterThanFamily {
    /// `>`.
    Greater,
    /// `>=`.
    GreaterOrEqual,
    /// `>>`.
    ShiftRight,
    /// `>>=`.
    ShiftRightAssign,
}

/// A prefix form: an operator that builds a node, or a removed reference form (B.3.6) that stands
/// as its operand once it is reported.
#[derive(Clone)]
enum Prefix {
    /// `-x` or `!x`.
    Operator(UnOp),
    /// `&x`, `&&x`, `&mut x` or `*x`.
    Reference,
}

/// A postfix form: field, method call, call, index, `?`.
#[derive(Clone)]
enum Postfix {
    /// `x.name` or `x.0`.
    Field(Ident),
    /// `x.0.1`: the lexer reads `0.1` as one float, so it is split here.
    Field2(Ident, Ident),
    /// `x.name::<T>(args)`.
    Method(Ident, Option<GenericArgs>, Vec<Arg>),
    /// `f(args)`, with an instance's `{ .. }` param block where the context admits one.
    Call(Vec<Arg>, Option<Vec<FieldInit>>),
    /// `x[i]`.
    Index(Expr),
    /// `x?`.
    Try,
}

/// What follows the first element of an array literal: `[e; n]` or `[a, b]`.
#[derive(Clone)]
enum ArrayTail {
    /// `; n`: the repeat count.
    Repeat(Expr),
    /// `, b, c`: the other elements, none for `[a]`.
    List(Vec<Expr>),
}

/// One argument of `emit!`, parsed loosely so a wrong shape is the spec's E0602, not a generic
/// syntax error.
#[derive(Clone)]
enum EmitArgument {
    /// `name = value`.
    Named(FieldInit),
    /// `name: value`: a field spelled like a struct field.
    Colon(Span),
    /// A bare expression.
    Positional(Expr),
}

/// What ends the left side of a simple statement.
#[derive(Clone)]
enum StatementTail {
    /// `+> rhs;`.
    Contribution(Expr),
    /// `<- value [after delay];`.
    Commit(Expr, Option<Expr>),
    /// `;`.
    Semi,
    /// The block's closing `}`: the last statement, with no `;`.
    Last,
}

impl ExpressionGrammar {
    /// Parse `src` as one expression.
    pub fn parse(src: &str) -> Parsed<Expr> {
        let (tokens, diags) = SourceParser::lex_limited(src);
        let input = TokenInput::over(&tokens, src.len());
        let h = Handles::declare();
        ParseDiagnostics::finish(crate::stack::Stack::deep(|| h.expr.clone().then_ignore(end()).parse(input).into_output_errors()), diags, &tokens)
    }

    /// An expression node of `kind` over `span`.
    fn node(kind: ExprKind, span: Span) -> Expr {
        Expr { kind, span: Loc(span) }
    }

    // ---- `>`-family operators -----------------------------------------------------------------

    /// One `>` token, as its span.
    fn greater_than<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
        just(Token::Punct(Punct::Gt)).map_with(|_, e| e.span())
    }

    /// One `=` token, as its span.
    fn equals<'a, I: In<'a>>() -> impl Parser<'a, I, Span, Extra<'a>> + Clone {
        just(Token::Punct(Punct::Eq)).map_with(|_, e| e.span())
    }

    /// After `as T`, `<` and `<<` open generic arguments, never a comparison or
    /// a shift (Rust rejects `a as u8 < b`).
    fn not_before_angle<'a, I: In<'a>>() -> impl Parser<'a, I, (), Extra<'a>> + Clone {
        any().filter(|t: &Token| !matches!(t, Token::Punct(Punct::Lt | Punct::Shl))).rewind().ignored().or(end())
    }

    /// Succeeds, consuming nothing, when the next token is not `punct`.
    fn not_before<'a, I: In<'a>>(punct: Punct) -> impl Parser<'a, I, (), Extra<'a>> + Clone {
        any().filter(move |t: &Token| *t != Token::Punct(punct)).rewind().ignored()
    }

    /// Fail unless the spans touch: the tokens were written as one operator.
    fn touching(spans: &[Span], span: Span, operator: &str) -> Result<(), Err<'static>> {
        if spans.windows(2).all(|w| w[0].end == w[1].start) {
            Ok(())
        } else {
            Err(Rich::custom(
                span,
                Custom {
                    code: "E0900",
                    message: format!("`{operator}` must be written without spaces between its characters"),
                    help: None,
                },
            ))
        }
    }

    /// The operator `want` of the `>` family, its tokens joined.
    fn greater_than_operator<'a, I: In<'a>>(want: GreaterThanFamily) -> P<'a, I, GreaterThanFamily> {
        match want {
            GreaterThanFamily::Greater => Self::greater_than()
                .then_ignore(Self::not_before(Punct::Gt))
                .then_ignore(Self::not_before(Punct::Eq))
                .to(GreaterThanFamily::Greater)
                .boxed(),
            GreaterThanFamily::GreaterOrEqual => Self::greater_than()
                .then(Self::equals())
                .try_map(|(a, b), sp| Self::touching(&[a, b], sp, ">=").map(|_| GreaterThanFamily::GreaterOrEqual))
                .boxed(),
            GreaterThanFamily::ShiftRight => Self::greater_than()
                .then(Self::greater_than())
                .try_map(|(a, b), sp| Self::touching(&[a, b], sp, ">>"))
                .then_ignore(Self::not_before(Punct::Eq))
                .to(GreaterThanFamily::ShiftRight)
                .boxed(),
            GreaterThanFamily::ShiftRightAssign => Self::greater_than()
                .then(Self::greater_than())
                .then(Self::equals())
                .try_map(|((a, b), c), sp| Self::touching(&[a, b, c], sp, ">>=").map(|_| GreaterThanFamily::ShiftRightAssign))
                .boxed(),
        }
    }

    // ---- node builders ------------------------------------------------------------------------

    /// The node of a postfix form applied to `lhs`.
    fn apply_postfix<'a, I: In<'a>>(lhs: Expr, form: Postfix, e: &mut MapExtra<'a, '_, I, Extra<'a>>) -> Expr {
        let kind = match form {
            Postfix::Field(name) => ExprKind::Field { expr: Box::new(lhs), name },
            Postfix::Field2(first, second) => {
                let inner = Expr {
                    span: Loc(lhs.span.0.to(first.span.0)),
                    kind: ExprKind::Field { expr: Box::new(lhs), name: first },
                };
                ExprKind::Field { expr: Box::new(inner), name: second }
            }
            Postfix::Method(method, generics, args) => {
                ExprKind::MethodCall { recv: Box::new(lhs), method, generics, args }
            }
            Postfix::Call(args, params) => ExprKind::Call { callee: Box::new(lhs), args, params },
            Postfix::Index(i) => ExprKind::Index { expr: Box::new(lhs), index: Box::new(i) },
            Postfix::Try => ExprKind::Try(Box::new(lhs)),
        };
        Self::node(kind, e.span())
    }

    /// The node of a binary operation.
    fn binary<'a, I: In<'a>>(lhs: Expr, op: BinOp, rhs: Expr, e: &mut MapExtra<'a, '_, I, Extra<'a>>) -> Expr {
        Self::node(ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, e.span())
    }

    // ---- the expression, level by level -------------------------------------------------------

    /// The expression parser; `allow_struct` admits `Path { .. }` and an
    /// instance's `{ .. }` param block at the top level. `me` is the handle of
    /// the parser being defined (for closure bodies and assignment's right side).
    pub(crate) fn expression<'a, I: In<'a>>(
        handles: &Handles<'a, I>,
        allow_struct: bool,
        me: Rec<'a, I, Expr>,
    ) -> P<'a, I, Expr> {
        let atom = Self::atom(handles, allow_struct, me.clone());
        let operand = Self::operators(handles, atom, allow_struct);
        Self::assignment(Self::range(operand), me)
    }

    /// `lhs = rhs` and the compound assignments, right-associative through `me`; a range alone
    /// otherwise.
    fn assignment<'a, I: In<'a>>(range: P<'a, I, Expr>, me: Rec<'a, I, Expr>) -> P<'a, I, Expr> {
        range
            .then(Self::assignment_operator().then(me).or_not())
            .map_with(|(lhs, tail), e| match tail {
                None => lhs,
                Some((op, rhs)) => Self::node(ExprKind::Assign { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, e.span()),
            })
            .boxed()
    }

    /// `=` (no operator) or a compound assignment's operator.
    fn assignment_operator<'a, I: In<'a>>() -> impl Parser<'a, I, Option<BinOp>, Extra<'a>> + Clone {
        choice((
            TokenInput::punctuation(Punct::Eq).to(None),
            TokenInput::punctuation(Punct::PlusEq).to(Some(BinOp::Add)),
            TokenInput::punctuation(Punct::MinusEq).to(Some(BinOp::Sub)),
            TokenInput::punctuation(Punct::StarEq).to(Some(BinOp::Mul)),
            TokenInput::punctuation(Punct::SlashEq).to(Some(BinOp::Div)),
            TokenInput::punctuation(Punct::PercentEq).to(Some(BinOp::Rem)),
            TokenInput::punctuation(Punct::CaretEq).to(Some(BinOp::BitXor)),
            TokenInput::punctuation(Punct::AndEq).to(Some(BinOp::BitAnd)),
            TokenInput::punctuation(Punct::OrEq).to(Some(BinOp::BitOr)),
            TokenInput::punctuation(Punct::ShlEq).to(Some(BinOp::Shl)),
            Self::greater_than_operator(GreaterThanFamily::ShiftRightAssign).to(Some(BinOp::Shr)),
        ))
    }

    /// `..`, `a..`, `..b`, `a..b` and the `..=` forms over two operands; an operand alone otherwise.
    fn range<'a, I: In<'a>>(operand: P<'a, I, Expr>) -> P<'a, I, Expr> {
        let prefix_range = Self::range_operator()
            .then(operand.clone().or_not())
            .map_with(|(inclusive, hi), e| {
                Self::node(ExprKind::Range { lo: None, hi: hi.map(Box::new), inclusive }, e.span())
            })
            .boxed();
        let infix_range = operand
            .clone()
            .then(Self::range_operator().then(operand.or_not()).or_not())
            .map_with(|(lo, tail), e| match tail {
                None => lo,
                Some((inclusive, hi)) => {
                    Self::node(ExprKind::Range { lo: Some(Box::new(lo)), hi: hi.map(Box::new), inclusive }, e.span())
                }
            })
            .boxed();
        TokenInput::first_of(vec![prefix_range, infix_range])
    }

    /// `..=` (true) or `..` (false).
    fn range_operator<'a, I: In<'a>>() -> impl Parser<'a, I, bool, Extra<'a>> + Clone {
        choice((TokenInput::punctuation(Punct::DotDotEq).to(true), TokenInput::punctuation(Punct::DotDot).to(false)))
    }

    /// Every level from `||` down to postfix: one Pratt parser over the atom.
    ///
    /// One precedence-climbing loop replaces a parser per precedence level, so
    /// a nesting level costs one frame chain rather than a dozen (T16).
    fn operators<'a, I: In<'a>>(handles: &Handles<'a, I>, atom: P<'a, I, Expr>, allow_struct: bool) -> P<'a, I, Expr> {
        let cast = TokenInput::keyword(Kw::As).ignore_then(handles.ty.clone()).then_ignore(Self::not_before_angle());
        atom.pratt((
            postfix(15, Self::postfix_operator(handles, allow_struct), Self::apply_postfix::<I>),
            prefix(14, Self::unary_operator(), |op, rhs: Expr, e| match op {
                Prefix::Operator(op) => Self::node(ExprKind::Unary { op, expr: Box::new(rhs) }, e.span()),
                Prefix::Reference => rhs,
            }),
            postfix(13, cast, |lhs: Expr, ty, e| Self::node(ExprKind::Cast { expr: Box::new(lhs), ty }, e.span())),
            infix(left(12), Self::product_operator(), Self::binary::<I>),
            infix(left(11), Self::sum_operator(), Self::binary::<I>),
            infix(left(10), Self::shift_operator(), Self::binary::<I>),
            infix(left(9), Self::binary_operator(Punct::And, BinOp::BitAnd), Self::binary::<I>),
            infix(left(8), Self::binary_operator(Punct::Caret, BinOp::BitXor), Self::binary::<I>),
            infix(left(7), Self::binary_operator(Punct::Or, BinOp::BitOr), Self::binary::<I>),
            infix(none(6), Self::comparison_operator(), Self::binary::<I>),
            infix(left(5), Self::binary_operator(Punct::AndAnd, BinOp::And), Self::binary::<I>),
            infix(left(4), Self::binary_operator(Punct::OrOr, BinOp::Or), Self::binary::<I>),
        ))
        .boxed()
    }

    /// The token `punct` read as the binary operator `op`.
    fn binary_operator<'a, I: In<'a>>(punct: Punct, op: BinOp) -> impl Parser<'a, I, BinOp, Extra<'a>> + Clone {
        TokenInput::punctuation(punct).to(op)
    }

    /// `-`, `!`, or a removed reference form (B.3.6), reported at the operator and read as the operand.
    fn unary_operator<'a, I: In<'a>>() -> impl Parser<'a, I, Prefix, Extra<'a>> + Clone {
        choice((
            TokenInput::punctuation(Punct::Minus).to(Prefix::Operator(UnOp::Neg)),
            TokenInput::punctuation(Punct::Not).to(Prefix::Operator(UnOp::Not)),
            choice((TokenInput::reference_head(), TokenInput::punctuation(Punct::Star).map_with(|_, e| e.span()))).validate(|head, _, emitter| {
                emitter.emit(ParseDiagnostics::removed("references and borrows", head));
                Prefix::Reference
            }),
        ))
    }

    /// `*`, `/`, `%`.
    fn product_operator<'a, I: In<'a>>() -> impl Parser<'a, I, BinOp, Extra<'a>> + Clone {
        choice((
            Self::binary_operator(Punct::Star, BinOp::Mul),
            Self::binary_operator(Punct::Slash, BinOp::Div),
            Self::binary_operator(Punct::Percent, BinOp::Rem),
        ))
    }

    /// `+`, `-`.
    fn sum_operator<'a, I: In<'a>>() -> impl Parser<'a, I, BinOp, Extra<'a>> + Clone {
        choice((Self::binary_operator(Punct::Plus, BinOp::Add), Self::binary_operator(Punct::Minus, BinOp::Sub)))
    }

    /// `<<`, `>>`.
    fn shift_operator<'a, I: In<'a>>() -> impl Parser<'a, I, BinOp, Extra<'a>> + Clone {
        choice((
            Self::binary_operator(Punct::Shl, BinOp::Shl),
            Self::greater_than_operator(GreaterThanFamily::ShiftRight).to(BinOp::Shr),
        ))
    }

    /// `==`, `!=`, `<=`, `<`, `>=`, `>`.
    fn comparison_operator<'a, I: In<'a>>() -> impl Parser<'a, I, BinOp, Extra<'a>> + Clone {
        choice((
            Self::binary_operator(Punct::EqEq, BinOp::Eq),
            Self::binary_operator(Punct::Ne, BinOp::Ne),
            Self::binary_operator(Punct::Le, BinOp::Le),
            Self::binary_operator(Punct::Lt, BinOp::Lt),
            Self::greater_than_operator(GreaterThanFamily::GreaterOrEqual).to(BinOp::Ge),
            Self::greater_than_operator(GreaterThanFamily::Greater).to(BinOp::Gt),
        ))
    }

    // ---- postfix ------------------------------------------------------------------------------

    /// A postfix form: `.field`, `.method(..)`, a call, an index or `?`.
    fn postfix_operator<'a, I: In<'a>>(handles: &Handles<'a, I>, allow_struct: bool) -> P<'a, I, Postfix> {
        let index = handles
            .expr
            .clone()
            .delimited_by(TokenInput::punctuation(Punct::LBracket), TokenInput::punctuation(Punct::RBracket))
            .map(Postfix::Index);
        TokenInput::first_of(vec![
            Self::method_or_field(handles).boxed(),
            Self::call(handles, allow_struct).boxed(),
            index.boxed(),
            TokenInput::punctuation(Punct::Question).to(Postfix::Try).boxed(),
        ])
    }

    /// `(a, name: b)`: a call's arguments, each optionally named.
    fn arguments<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Vec<Arg>, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::identifier()
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .or_not()
            .then(handles.expr.clone())
            .map_with(|(name, value), e| Arg { name, value, span: Loc(e.span()) })
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
    }

    /// `(args)`, followed by an instance's `{ .. }` param block where `allow_struct` admits one.
    fn call<'a, I: In<'a>>(handles: &Handles<'a, I>, allow_struct: bool) -> impl Parser<'a, I, Postfix, Extra<'a>> + Clone + use<'a, I> {
        let params = if allow_struct { Self::field_values(handles).or_not().boxed() } else { empty().to(None).boxed() };
        Self::arguments(handles).then(params).map(|(a, p)| Postfix::Call(a, p))
    }

    /// `.name`, `.name(args)`, `.0` or `.0.1`.
    fn method_or_field<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Postfix, Extra<'a>> + Clone + use<'a, I> {
        let tuple_index = select! { Token::Int(s) => s }
            .map_with(|name, e| Ident { name, raw: false, span: Loc(e.span()) });
        TokenInput::punctuation(Punct::Dot).ignore_then(TokenInput::first_of(vec![
            TokenInput::identifier()
                .then(TypeGrammar::generic_args(handles, true).or_not())
                .then(Self::arguments(handles))
                .map(|((name, g), a)| Postfix::Method(name, g, a))
                .boxed(),
            TokenInput::identifier().or(tuple_index).map(Postfix::Field).boxed(),
            Self::nested_tuple_index().map(|(a, b)| Postfix::Field2(a, b)).boxed(),
        ]))
    }

    /// `0.1` after a dot: two tuple indexes lexed as one float.
    fn nested_tuple_index<'a, I: In<'a>>() -> impl Parser<'a, I, (Ident, Ident), Extra<'a>> + Clone {
        select! { Token::Float { raw, .. } => raw }.try_map(|raw: String, span: Span| {
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
        })
    }

    // ---- atoms --------------------------------------------------------------------------------

    /// The operand of the operators: a literal, a macro form, a group, an array, a block-like
    /// expression, a jump, a closure or a path.
    fn atom<'a, I: In<'a>>(handles: &Handles<'a, I>, allow_struct: bool, me: Rec<'a, I, Expr>) -> P<'a, I, Expr> {
        // A malformed token already reported by the lexer.
        let error_atom = just(Token::Error).map_with(|_, e| Self::node(ExprKind::Error, e.span()));
        let continue_expression = TokenInput::keyword(Kw::Continue).map_with(|_, e| Self::node(ExprKind::Continue, e.span()));
        // A slice of boxed alternatives, not a tuple: a tuple `choice` keeps every
        // alternative's temporaries in one stack frame (60 KiB in a debug build).
        TokenInput::first_of(vec![
            TokenInput::literal_expression().boxed(),
            Self::emit(handles).boxed(),
            Self::include_string(handles).boxed(),
            Self::parenthesized(handles).boxed(),
            Self::array(handles).boxed(),
            Self::block_like(handles),
            Self::break_expression(me.clone()).boxed(),
            continue_expression.boxed(),
            Self::return_expression(me.clone()).boxed(),
            Self::closure(handles, me).boxed(),
            error_atom.boxed(),
            Self::macro_call(handles).boxed(),
            Self::qualified_path(handles).boxed(),
            Self::path_expression(handles, allow_struct),
        ])
        .labelled("expression")
        .boxed()
    }

    /// `(e)` or a tuple `(a, b)`, `(a,)`, `()`.
    fn parenthesized<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        handles
            .expr
            .clone()
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .collect::<Vec<_>>()
            .then(TokenInput::punctuation(Punct::Comma).or_not())
            .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen))
            .map_with(|(mut items, trailing), e| {
                if items.len() == 1 && trailing.is_none() {
                    Self::node(ExprKind::Paren(Box::new(items.remove(0))), e.span())
                } else {
                    Self::node(ExprKind::Tuple(items), e.span())
                }
            })
    }

    /// `[e; n]`, `[a, b]` or `[]`: the first element is parsed once.
    fn array<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        handles
            .expr
            .clone()
            .then(Self::array_tail(handles))
            .or_not()
            .delimited_by(TokenInput::punctuation(Punct::LBracket), TokenInput::punctuation(Punct::RBracket))
            .map_with(|first, e| match first {
                None => Self::node(ExprKind::Array(Vec::new()), e.span()),
                Some((elem, ArrayTail::Repeat(len))) => {
                    Self::node(ExprKind::ArrayRepeat { elem: Box::new(elem), len: Box::new(len) }, e.span())
                }
                Some((first, ArrayTail::List(mut rest))) => {
                    rest.insert(0, first);
                    Self::node(ExprKind::Array(rest), e.span())
                }
            })
    }

    /// What follows an array literal's first element.
    fn array_tail<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, ArrayTail, Extra<'a>> + Clone + use<'a, I> {
        choice((
            TokenInput::punctuation(Punct::Semi).ignore_then(handles.expr.clone()).map(ArrayTail::Repeat),
            TokenInput::punctuation(Punct::Comma)
                .ignore_then(
                    handles
                        .expr
                        .clone()
                        .separated_by(TokenInput::punctuation(Punct::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>(),
                )
                .map(ArrayTail::List),
            empty().to(Vec::new()).map(ArrayTail::List),
        ))
    }

    /// `{ name: value, .. }`: a struct literal's fields or an instance's param block. A field is
    /// `name: value`, or the shorthand `name` for `name: name`.
    fn field_values<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Vec<FieldInit>, Extra<'a>> + Clone + use<'a, I> {
        let field = ItemGrammar::rejected_meta(handles)
            .ignore_then(TokenInput::identifier())
            .then(TokenInput::punctuation(Punct::Colon).ignore_then(handles.expr.clone()).or_not())
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
        field
            .separated_by(TokenInput::punctuation(Punct::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace))
    }

    /// A path, or a struct literal `Path { .. }` where `allow_struct` admits one.
    fn path_expression<'a, I: In<'a>>(handles: &Handles<'a, I>, allow_struct: bool) -> P<'a, I, Expr> {
        let path = TypeGrammar::path(handles, true);
        if allow_struct {
            path.then(Self::field_values(handles).or_not())
                .map_with(|(path, fields), e| match fields {
                    Some(fields) => Self::node(ExprKind::Struct { path, fields }, e.span()),
                    None => Self::node(ExprKind::Path(path), e.span()),
                })
                .boxed()
        } else {
            path.map_with(|path, e| Self::node(ExprKind::Path(path), e.span())).boxed()
        }
    }

    /// `emit!("stream", name = value, ..)`.
    fn emit<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        select! { Token::Ident(s) if s == "emit" => () }
            .ignore_then(TokenInput::punctuation(Punct::Not))
            .ignore_then(
                Self::emit_argument(handles)
                    .separated_by(TokenInput::punctuation(Punct::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen)),
            )
            .validate(|args, e, emitter| Self::emit_expression(args, e.span(), emitter))
    }

    /// One argument of `emit!`: `name = value`, `name: value` or an expression.
    fn emit_argument<'a, I: In<'a>>(handles: &Handles<'a, I>) -> P<'a, I, EmitArgument> {
        TokenInput::first_of(vec![
            TokenInput::identifier()
                .then_ignore(TokenInput::punctuation(Punct::Eq))
                .then(handles.expr.clone())
                .map_with(|(name, value), e| FieldInit { name, value, shorthand: false, span: Loc(e.span()) })
                .map(EmitArgument::Named)
                .boxed(),
            TokenInput::identifier()
                .then_ignore(TokenInput::punctuation(Punct::Colon))
                .then_ignore(handles.expr.clone())
                .map_with(|_, e| EmitArgument::Colon(e.span()))
                .boxed(),
            handles.expr.clone().map(EmitArgument::Positional).boxed(),
        ])
    }

    /// `E0602` at `span`: `what` is wrong in `emit!`.
    fn emit_error(span: Span, what: &str) -> Err<'static> {
        Rich::custom(
            span,
            Custom {
                code: "E0602",
                message: format!("{what} in `emit!`"),
                help: Some("write `emit!(\"stream\", name = value, ..)`: a string literal, then `name = value` fields".into()),
            },
        )
    }

    /// The `emit!` node of its arguments over `span`: a string literal, then `name = value` fields;
    /// an error node, every wrong argument reported, otherwise.
    fn emit_expression(args: Vec<EmitArgument>, span: Span, emitter: &mut Emitter<Err<'_>>) -> Expr {
        let mut args = args.into_iter();
        let stream = match args.next() {
            Some(EmitArgument::Positional(Expr { kind: ExprKind::Lit(l @ Lit { kind: LitKind::Str { .. }, .. }), .. })) => l,
            other => {
                let at = match other {
                    Some(EmitArgument::Positional(x)) => x.span.0,
                    Some(EmitArgument::Named(f)) => f.span.0,
                    Some(EmitArgument::Colon(at)) => at,
                    None => {
                        emitter.emit(Self::emit_error(span, "the stream name is missing"));
                        return Self::node(ExprKind::Error, span);
                    }
                };
                emitter.emit(Self::emit_error(at, "the first argument is not a string literal"));
                return Self::node(ExprKind::Error, span);
            }
        };
        let mut fields = Vec::new();
        let mut ok = true;
        for arg in args {
            match arg {
                EmitArgument::Named(f) => fields.push(f),
                EmitArgument::Colon(at) => {
                    emitter.emit(Self::emit_error(at, "a field is not `name = value`"));
                    ok = false;
                }
                EmitArgument::Positional(x) => {
                    emitter.emit(Self::emit_error(x.span.0, "a field is not `name = value`"));
                    ok = false;
                }
            }
        }
        if ok { Self::node(ExprKind::Emit { stream, fields }, span) } else { Self::node(ExprKind::Error, span) }
    }

    /// `include_str!("path")`.
    fn include_string<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        select! { Token::Ident(s) if s == "include_str" => () }
            .ignore_then(TokenInput::punctuation(Punct::Not))
            .ignore_then(
                handles
                    .expr
                    .clone()
                    .separated_by(TokenInput::punctuation(Punct::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(TokenInput::punctuation(Punct::LParen), TokenInput::punctuation(Punct::RParen)),
            )
            .validate(|args, e, emitter| match args.as_slice() {
                [Expr { kind: ExprKind::Lit(l @ Lit { kind: LitKind::Str { .. }, .. }), .. }] => {
                    Self::node(ExprKind::IncludeStr(l.clone()), e.span())
                }
                [x] => {
                    emitter.emit(Self::include_string_error(x.span.0));
                    Self::node(ExprKind::Error, e.span())
                }
                _ => {
                    emitter.emit(Self::include_string_error(e.span()));
                    Self::node(ExprKind::Error, e.span())
                }
            })
    }

    /// `E0611` at `span`: the argument of `include_str!` is not a string literal.
    fn include_string_error(span: Span) -> Err<'static> {
        Rich::custom(
            span,
            Custom {
                code: "E0611",
                message: "the argument of `include_str!` is not a string literal".into(),
                help: Some("write `include_str!(\"path/to/file\")`".into()),
            },
        )
    }

    /// Any other `name!(..)` is a macro: E0901, then the group is skipped.
    fn macro_call<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        TypeGrammar::path(handles, true)
            .filter(|p: &Path| {
                !(p.segments.len() == 1 && matches!(p.segments[0].ident.name.as_str(), "emit" | "include_str"))
            })
            .then_ignore(TokenInput::punctuation(Punct::Not))
            .then_ignore(Recovery::skip_group())
            .validate(|_, e, emitter| {
                emitter.emit(ParseDiagnostics::macro_error(e.span()));
                Self::node(ExprKind::Error, e.span())
            })
    }

    /// `<T as Tr>::f`: a removed form (B.3.6), reported once.
    fn qualified_path<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        TypeGrammar::qualified_path(handles).validate(|_, e, emitter| {
            emitter.emit(ParseDiagnostics::removed("qualified paths", e.span()));
            Self::node(ExprKind::Error, e.span())
        })
    }

    /// `break` with an optional value.
    fn break_expression<'a, I: In<'a>>(me: Rec<'a, I, Expr>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
        TokenInput::keyword(Kw::Break)
            .ignore_then(me.or_not())
            .map_with(|v, e| Self::node(ExprKind::Break(v.map(Box::new)), e.span()))
    }

    /// `return` with an optional value.
    fn return_expression<'a, I: In<'a>>(me: Rec<'a, I, Expr>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone {
        TokenInput::keyword(Kw::Return)
            .ignore_then(me.or_not())
            .map_with(|v, e| Self::node(ExprKind::Return(v.map(Box::new)), e.span()))
    }

    /// `move |a, b: T| -> R { .. }` / `|x| x + 1` / `|| 0`.
    fn closure<'a, I: In<'a>>(handles: &Handles<'a, I>, me: Rec<'a, I, Expr>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::keyword(Kw::Move)
            .map_with(|_, e| e.span())
            .validate(|at, _, emitter| emitter.emit(ReservedMarker::report(Kw::Move, at)))
            .or_not()
            .then(Self::closure_parameters(handles))
            .then(TokenInput::first_of(vec![
                TokenInput::punctuation(Punct::Arrow)
                    .ignore_then(handles.ty.clone())
                    .then(handles.block.clone().map_with(|b: Block, e| {
                        Self::node(ExprKind::Block(b), e.span())
                    }))
                    .map(|(ty, body)| (Some(ty), body))
                    .boxed(),
                me.map(|body| (None, body)).boxed(),
            ]))
            .map_with(|((_, params), (ret, body)), e| Self::node(ExprKind::Closure { params, ret, body: Box::new(body) }, e.span()))
    }

    /// `||` or `|a, b: T|`.
    fn closure_parameters<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Vec<ClosureParam>, Extra<'a>> + Clone + use<'a, I> {
        let parameter = PatternGrammar::binding(handles)
            .then(TokenInput::punctuation(Punct::Colon).ignore_then(handles.ty.clone()).or_not())
            .map_with(|(pat, ty), e| ClosureParam { pat, ty, span: Loc(e.span()) });
        choice((
            TokenInput::punctuation(Punct::OrOr).to(Vec::new()),
            parameter
                .separated_by(TokenInput::punctuation(Punct::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(TokenInput::punctuation(Punct::Or), TokenInput::punctuation(Punct::Or)),
        ))
    }

    // ---- block-like expressions ---------------------------------------------------------------

    /// `if`, `match`, `while`, `loop`, `for` and a plain block: expressions that
    /// end at their closing brace, so a statement needs no `;` after them.
    pub(crate) fn block_like<'a, I: In<'a>>(handles: &Handles<'a, I>) -> P<'a, I, Expr> {
        let loop_expression = TokenInput::keyword(Kw::Loop)
            .ignore_then(handles.block.clone())
            .map_with(|body, e| Self::node(ExprKind::Loop { body }, e.span()));
        TokenInput::first_of(vec![
            Self::if_expression(handles).boxed(),
            Self::match_expression(handles).boxed(),
            Self::while_expression(handles).boxed(),
            loop_expression.boxed(),
            Self::for_expression(handles).boxed(),
            Self::block_expression(handles).boxed(),
        ])
    }

    /// A block as an expression.
    fn block_expression<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        handles.block.clone().map_with(|b: Block, e| Self::node(ExprKind::Block(b), e.span()))
    }

    /// `let pat = expr` or an expression, with no struct literal at the top.
    fn condition<'a, I: In<'a>>(handles: &Handles<'a, I>) -> P<'a, I, Expr> {
        TokenInput::first_of(vec![
            TokenInput::keyword(Kw::Let)
                .ignore_then(handles.pat.clone())
                .then_ignore(TokenInput::punctuation(Punct::Eq))
                .then(handles.expr_ns.clone())
                .map_with(|(pat, e), sp| Self::node(ExprKind::Let { pat, expr: Box::new(e) }, sp.span()))
                .boxed(),
            handles.expr_ns.clone().boxed(),
        ])
    }

    /// `if cond { .. }`, with an `else` block or an `else if` chain.
    fn if_expression<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        let condition = Self::condition(handles);
        let block = handles.block.clone();
        let block_expression = Self::block_expression(handles);
        recursive(move |if_expression| {
            TokenInput::keyword(Kw::If)
                .ignore_then(condition.clone())
                .then(block.clone())
                .then(TokenInput::keyword(Kw::Else).ignore_then(TokenInput::first_of(vec![if_expression.boxed(), block_expression.clone().boxed()])).or_not())
                .map_with(|((cond, then), else_), e| {
                    Self::node(ExprKind::If { cond: Box::new(cond), then, else_: else_.map(Box::new) }, e.span())
                })
        })
    }

    /// `match scrutinee { arm* }`.
    fn match_expression<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        let arms = Self::match_arm(handles)
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace));
        TokenInput::keyword(Kw::Match)
            .ignore_then(handles.expr_ns.clone())
            .then(arms)
            .map_with(|(scrutinee, arms), e| {
                Self::node(ExprKind::Match { scrutinee: Box::new(scrutinee), arms }, e.span())
            })
    }

    /// `pat [if guard] => body`, the comma optional after a block-like body.
    fn match_arm<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Arm, Extra<'a>> + Clone + use<'a, I> {
        ItemGrammar::meta(handles)
            .then(handles.pat.clone())
            .then(TokenInput::keyword(Kw::If).ignore_then(handles.expr.clone()).or_not())
            .then_ignore(TokenInput::punctuation(Punct::FatArrow))
            .then(choice((
                // A block-like body needs no comma.
                Self::block_expression(handles).then_ignore(TokenInput::punctuation(Punct::Comma).or_not()),
                handles
                    .expr
                    .clone()
                    .then_ignore(choice((TokenInput::punctuation(Punct::Comma), TokenInput::punctuation(Punct::RBrace).rewind()))),
            )))
            .map_with(|(((meta, pat), guard), body), e| Arm {
                meta,
                pat,
                guard,
                body,
                span: Loc(e.span()),
            })
    }

    /// `while cond { .. }`.
    fn while_expression<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::keyword(Kw::While)
            .ignore_then(Self::condition(handles))
            .then(handles.block.clone())
            .map_with(|(cond, body), e| Self::node(ExprKind::While { cond: Box::new(cond), body }, e.span()))
    }

    /// `for pat in iter { .. }`.
    fn for_expression<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Expr, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::keyword(Kw::For)
            .ignore_then(PatternGrammar::binding(handles))
            .then_ignore(TokenInput::keyword(Kw::In))
            .then(handles.expr_ns.clone())
            .then(handles.block.clone())
            .map_with(|((pat, iter), body), e| {
                Self::node(ExprKind::For { pat, iter: Box::new(iter), body }, e.span())
            })
    }

    // ---- blocks and statements ----------------------------------------------------------------

    /// A block: `{ stmt* }`.
    pub(crate) fn block<'a, I: In<'a>>(handles: &Handles<'a, I>) -> P<'a, I, Block> {
        Self::statement(handles)
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(TokenInput::punctuation(Punct::LBrace), TokenInput::punctuation(Punct::RBrace))
            .map_with(|stmts, e| Block { stmts, span: Loc(e.span()) })
            .boxed()
    }

    /// One statement: `let`, `;`, a block-like expression, an expression
    /// statement, a `contribution` (`e +> e;`) or a `commit` (`place <- e
    /// [after e];`). The last statement of a block may omit its `;`.
    pub(crate) fn statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> P<'a, I, Stmt> {
        // Every form but an item takes outer docs and attributes (Rust's `stmt`).
        let empty = TokenInput::punctuation(Punct::Semi).map_with(|_, e| Self::statement_node(StmtKind::Empty, e.span()));
        let with_meta = ItemGrammar::meta(handles)
            .then(TokenInput::first_of(vec![
                empty.boxed(),
                Self::let_statement(handles).boxed(),
                Self::signal_statement(handles).boxed(),
                Self::reaction_statement(handles).boxed(),
                Self::block_like_statement(handles).boxed(),
                Self::simple_statement(handles).boxed(),
            ]))
            .map(|(meta, mut s): (Meta, Stmt)| {
                s.meta = meta;
                s
            });
        let statement = TokenInput::first_of(vec![Self::item_statement(handles).boxed(), with_meta.boxed()])
            .labelled("statement")
            .boxed();
        Recovery::recover(statement, false, |span| Stmt {
            meta: Meta::default(),
            kind: StmtKind::Expr { expr: Expr { kind: ExprKind::Error, span: Loc(span) }, semi: true },
            span: Loc(span),
        })
    }

    /// A statement node of `kind` over `span`, with no docs or attributes yet.
    fn statement_node(kind: StmtKind, span: Span) -> Stmt {
        Stmt { meta: Meta::default(), kind, span: Loc(span) }
    }

    /// `let pat [: T] [= init [else { .. }]];`.
    fn let_statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Stmt, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::keyword(Kw::Let)
            .ignore_then(PatternGrammar::binding(handles))
            .then(TokenInput::punctuation(Punct::Colon).ignore_then(handles.ty.clone()).or_not())
            .then(
                TokenInput::punctuation(Punct::Eq)
                    .ignore_then(handles.expr.clone())
                    // `else` is matched without listing it among the expected tokens: it is
                    // rare enough that an error after `let x = 1` names `;` first.
                    .then(select! { Token::Kw(Kw::Else) => () }.ignore_then(handles.block.clone()).or_not())
                    .or_not(),
            )
            .then_ignore(TokenInput::punctuation(Punct::Semi))
            .map_with(|((pat, ty), tail), e| {
                let (init, else_) = match tail {
                    Some((init, else_)) => (Some(init), else_),
                    None => (None, None),
                };
                Self::statement_node(StmtKind::Let { pat, ty, init, else_ }, e.span())
            })
    }

    /// Whether the token starts a block-like expression. Which of the two statement forms applies
    /// is decided by the first token, so a statement is never parsed twice (that would cost
    /// 2^depth when an error sits deep inside nested blocks).
    fn starts_block_like(t: &Token) -> bool {
        matches!(
            t,
            Token::Kw(Kw::If | Kw::Match | Kw::While | Kw::Loop | Kw::For)
                | Token::Punct(Punct::LBrace)
        )
    }

    /// A block-like expression as a statement, its `;` optional.
    fn block_like_statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Stmt, Extra<'a>> + Clone + use<'a, I> {
        any()
            .filter(Self::starts_block_like)
            .rewind()
            .ignore_then(Self::block_like(handles))
            .then(TokenInput::punctuation(Punct::Semi).or_not())
            .map_with(|(expr, semi), e| Self::statement_node(StmtKind::Expr { expr, semi: semi.is_some() }, e.span()))
    }

    /// An expression, then what ends the statement: `+> rhs;`, `<- value [after e];`, `;`, or the
    /// block's `}`.
    fn simple_statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Stmt, Extra<'a>> + Clone + use<'a, I> {
        any()
            .filter(|t: &Token| !Self::starts_block_like(t))
            .rewind()
            .ignore_then(handles.expr.clone())
            .then(Self::statement_tail(handles))
            .map_with(|(lhs, tail), e| {
                let kind = match tail {
                    StatementTail::Contribution(rhs) => StmtKind::Contribution { lhs, rhs },
                    StatementTail::Commit(value, after) => StmtKind::Commit { place: lhs, value, after },
                    StatementTail::Semi => StmtKind::Expr { expr: lhs, semi: true },
                    StatementTail::Last => StmtKind::Expr { expr: lhs, semi: false },
                };
                Self::statement_node(kind, e.span())
            })
    }

    /// What ends a simple statement.
    fn statement_tail<'a, I: In<'a>>(handles: &Handles<'a, I>) -> P<'a, I, StatementTail> {
        let semi = TokenInput::punctuation(Punct::Semi);
        TokenInput::first_of(vec![
            TokenInput::punctuation(Punct::PlusGt)
                .ignore_then(handles.expr.clone())
                .then_ignore(semi.clone())
                .map(StatementTail::Contribution)
                .boxed(),
            TokenInput::punctuation(Punct::LtMinus)
                .ignore_then(handles.expr.clone())
                .then(TokenInput::keyword(Kw::After).ignore_then(handles.expr.clone()).or_not())
                .then_ignore(semi.clone())
                .map(|(v, after)| StatementTail::Commit(v, after))
                .boxed(),
            semi.to(StatementTail::Semi).boxed(),
            TokenInput::punctuation(Punct::RBrace).rewind().to(StatementTail::Last).boxed(),
        ])
    }

    /// `signal name: T [= init];`: a module and `digital` form, parsed here so a context check can
    /// name it where it is not allowed.
    fn signal_statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Stmt, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::keyword(Kw::Signal)
            .ignore_then(TokenInput::identifier())
            .then_ignore(TokenInput::punctuation(Punct::Colon))
            .then(TypeGrammar::sigtype(handles))
            .then(TokenInput::punctuation(Punct::Eq).ignore_then(handles.expr.clone()).or_not())
            .then_ignore(TokenInput::punctuation(Punct::Semi))
            .map_with(|((name, ty), init), e| Self::statement_node(StmtKind::Signal { name, ty, init }, e.span()))
    }

    /// `on event { .. }`: a module and `digital` form, parsed here so a context check can name it
    /// where it is not allowed.
    fn reaction_statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Stmt, Extra<'a>> + Clone + use<'a, I> {
        TokenInput::keyword(Kw::On)
            .ignore_then(handles.expr_ns.clone())
            .then(handles.block.clone())
            .map_with(|(event, body), e| Self::statement_node(StmtKind::Reaction(Reaction { event, body }), e.span()))
    }

    /// A Rust item inside a block: it starts with an item keyword, after any doc comments and
    /// attributes (which are then the item's own).
    fn item_statement<'a, I: In<'a>>(handles: &Handles<'a, I>) -> impl Parser<'a, I, Stmt, Extra<'a>> + Clone + use<'a, I> {
        let item_keyword = any().filter(|t: &Token| {
            matches!(
                t,
                Token::Kw(Kw::Const | Kw::Fn | Kw::Use | Kw::Struct | Kw::Enum | Kw::Impl | Kw::Trait | Kw::Mod | Kw::Module | Kw::Pub | Kw::Extern | Kw::Type | Kw::Static)
            )
        });
        ItemGrammar::meta(handles)
            .ignore_then(item_keyword)
            .rewind()
            .ignore_then(handles.item.clone())
            .map_with(|item, e| Self::statement_node(StmtKind::Item(Box::new(item)), e.span()))
    }
}
