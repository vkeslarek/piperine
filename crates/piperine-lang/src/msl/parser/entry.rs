//! Public entry points: lex, parse, and turn parse errors into diagnostics.

use chumsky::error::{RichPattern, RichReason};

use super::*;

/// The parser's input over a token vector: tokens by value, end of input at `len`.
pub(crate) fn token_input<'a>(
    tokens: &'a [(Token, Span)],
    len: usize,
) -> impl In<'a> {
    use chumsky::input::Input;
    tokens.map(Span::new(len, len), |(t, s): &(Token, Span)| (t, s))
}

/// Turn a parse error into the spec's [`Diagnostic`].
pub(crate) fn to_diagnostic(e: &Err<'_>, tokens: &[(Token, Span)]) -> Diagnostic {
    let span = *e.span();
    // An error that carries no found token but sits on one: recover it.
    let at_span = || tokens.iter().find(|(_, s)| *s == span && !s.is_empty()).map(|(t, _)| t.clone());
    match e.reason() {
        RichReason::Custom(c) => {
            let mut d = Diagnostic::new(c.code, c.message.clone(), span, c.message.clone());
            d.help = c.help.clone();
            d
        }
        RichReason::ExpectedFound { found: Some(t), .. }
            if matches!(&**t, Token::Punct(Punct::PlusGt | Punct::LtMinus)) =>
        {
            let op = if matches!(&**t, Token::Punct(Punct::PlusGt)) { "+>" } else { "<-" };
            Diagnostic::new(
                "E0907",
                format!("`{op}` inside an expression"),
                span,
                "no statement form accepts it here",
            )
            .with_help(
                "`+>` is a contribution statement `probe +> value;` in an `analog` block; \
                 `<-` is a commit statement `place <- value;` in a reaction",
            )
        }
        // A form Part I §1 rule 2 leaves out, wherever it is found.
        RichReason::ExpectedFound { found: Some(t), .. }
            if matches!(&**t, Token::Kw(Kw::Unsafe | Kw::Async | Kw::Await | Kw::Dyn)) =>
        {
            name_error(&t, span)
        }
        // `&mut`: the `&` before the `mut` that was found is part of the form.
        RichReason::ExpectedFound { found: Some(t), .. }
            if matches!(&**t, Token::Kw(Kw::Mut))
                && tokens.iter().any(|(p, s)| *p == Token::Punct(Punct::And) && s.end == span.start) =>
        {
            let start = tokens
                .iter()
                .find(|(p, s)| *p == Token::Punct(Punct::And) && s.end == span.start)
                .map_or(span.start, |(_, s)| s.start);
            Diagnostic::new(
                "E0101",
                "a mutable reference is not part of the language",
                Span::new(start, span.end),
                "a syntactic form left out of the language",
            )
            .with_help("values are immutable and owned; Part I §1 rule 2 lists the forms left out")
        }
        RichReason::ExpectedFound { found: Some(t), .. } if names_expected(e, &t) => name_error(&t, span),
        RichReason::ExpectedFound { found: None, .. } if at_span().is_some_and(|t| names_expected(e, &t)) => {
            name_error(&at_span().unwrap_or(Token::Error), span)
        }
        RichReason::ExpectedFound { found, .. } => {
            // A found token the error did not keep is recovered from its span.
            let found = match found {
                Some(t) => Some((**t).clone()),
                None => at_span(),
            };
            let found = found.map_or_else(|| "end of input".to_string(), |t| t.to_string());
            let expected = summarize_expected(e);
            let message = if expected.is_empty() {
                format!("unexpected {found}")
            } else {
                format!("unexpected {found}, expected {}", join_alternatives(&expected))
            };
            Diagnostic::new("E0900", message, span, format!("found {found}"))
        }
    }
}

/// The most alternatives a message lists before it says "or N more".
const MAX_EXPECTED: usize = 6;

/// A token that continues an expression: an operator or a postfix form.
fn is_operator(t: &Token) -> bool {
    use Punct::*;
    matches!(
        t,
        Token::Kw(Kw::As)
            | Token::Punct(
                Plus | Minus | Star | Slash | Percent | Caret | And | Or | AndAnd | OrOr | Shl | PlusEq
                    | MinusEq | StarEq | SlashEq | PercentEq | CaretEq | AndEq | OrEq | ShlEq | Eq
                    | EqEq | Ne | Gt | Lt | Le | Not | Dot | DotDot | DotDotEq | Question | LParen | LBracket
            )
    )
}

/// The expected set of an error as a short list: constructs (labels) and
/// delimiters first, then keywords, with the many operator tokens of an
/// expression collapsed into "an operator" (or dropped when an expression
/// is itself expected).
fn summarize_expected(e: &Err<'_>) -> Vec<String> {
    let mut ranked: Vec<(u8, String)> = Vec::new();
    let mut operators = 0;
    let mut expression = false;
    for p in e.expected() {
        match p {
            RichPattern::Token(t) => {
                if is_operator(t) {
                    operators += 1;
                } else if matches!(**t, Token::Punct(Punct::Semi | Punct::Comma | Punct::RParen | Punct::RBracket | Punct::RBrace | Punct::LBrace)) {
                    ranked.push((0, t.to_string()));
                } else {
                    ranked.push((2, t.to_string()));
                }
            }
            RichPattern::Label(l) => {
                expression |= &**l == "expression";
                ranked.push((1, l.to_string()));
            }
            RichPattern::EndOfInput => ranked.push((3, "end of input".to_string())),
            _ => {}
        }
    }
    if operators >= 3 && !expression {
        ranked.push((1, "an operator".to_string()));
    } else if !expression {
        // Few enough to name: they were dropped from the count above.
        for p in e.expected() {
            if let RichPattern::Token(t) = p {
                if is_operator(t) {
                    ranked.push((2, t.to_string()));
                }
            }
        }
    }
    ranked.sort();
    ranked.dedup();
    ranked.into_iter().map(|(_, s)| s).collect()
}

/// `a`, `a or b`, `a, b or c`, and past [`MAX_EXPECTED`] `a, b, .. or N more`.
fn join_alternatives(items: &[String]) -> String {
    let (shown, more) = if items.len() > MAX_EXPECTED {
        (&items[..MAX_EXPECTED - 1], items.len() - (MAX_EXPECTED - 1))
    } else {
        (items, 0)
    };
    let mut out = match shown {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] if more == 0 => format!("{} or {last}", init.join(", ")),
        all => all.join(", "),
    };
    if more > 0 {
        out.push_str(&format!(" or {more} more"));
    }
    out
}

type Parsed<O> = (Option<O>, Vec<Diagnostic>);

fn finish<O>(
    out: (Option<O>, Vec<Err<'_>>),
    mut diags: Vec<Diagnostic>,
    tokens: &[(Token, Span)],
) -> Parsed<O> {
    let (value, errors) = out;
    // A `Token::Error` was already reported by the lexer or the depth check.
    let reported = |e: &Err<'_>| {
        matches!(e.reason(), RichReason::ExpectedFound { found: Some(t), .. } if matches!(&**t, Token::Error))
    };
    diags.extend(errors.iter().filter(|e| !reported(e)).map(|e| to_diagnostic(e, tokens)));
    diags.sort_by_key(|d| d.span().start);
    // Recovery can report one position twice (a rewound alternative and the
    // enclosing group): one diagnostic per code and span.
    diags.dedup_by(|b, a| a.code == b.code && a.span() == b.span());
    (value, diags)
}

/// Parse `src` as one type. Lexical diagnostics come first-class in the result.
pub fn parse_type(src: &str) -> Parsed<Type> {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(ty::build(&h).then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
}

/// Parse `src` as one pattern.
pub fn parse_pat(src: &str) -> Parsed<Pat> {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(h.pat.clone().then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
}

/// Parse `src` as generic parameters (`<T: Repr = bool, const N: u32>`).
pub fn parse_generics(src: &str) -> Parsed<Generics> {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(ty::generics(&h).then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
}

/// Parse `src` as one expression.
pub fn parse_expr(src: &str) -> Parsed<Expr> {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(h.expr.clone().then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
}

/// Parse `src` as one item of a module body.
pub fn parse_module_item(src: &str) -> Parsed<ModuleItem> {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(msl::module_item(&h).then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
}

/// Parse `src` as one braced block `{ stmt* }`.
pub fn parse_block(src: &str) -> Parsed<Block> {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(h.block.clone().then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
}

/// The deepest delimiter nesting the parser descends into (E0910).
pub const MAX_DEPTH: usize = 256;

/// Lex `src`, then replace every delimited group nested deeper than
/// [`MAX_DEPTH`] by one [`Token::Error`], reporting `E0910`: the parser
/// never descends that far, so it cannot overflow the stack.
fn lex_limited(src: &str) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
    let (tokens, mut diags) = lex(src);
    let opens = |t: &Token| matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace));
    let closes = |t: &Token| matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace));
    let mut out = Vec::with_capacity(tokens.len());
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let (tok, span) = &tokens[i];
        if opens(tok) && depth == MAX_DEPTH {
            // Skip to the matching close, nesting-aware.
            let mut nest = 0usize;
            let mut j = i;
            while j < tokens.len() {
                if opens(&tokens[j].0) {
                    nest += 1;
                } else if closes(&tokens[j].0) {
                    nest -= 1;
                    if nest == 0 {
                        break;
                    }
                }
                j += 1;
            }
            let end = tokens.get(j).map_or(span.end, |(_, s)| s.end);
            diags.push(
                Diagnostic::new(
                    "E0910",
                    format!("nesting deeper than {MAX_DEPTH} levels"),
                    *span,
                    "the parser stops descending here",
                )
                .with_help("split the expression into `let` bindings or functions"),
            );
            out.push((Token::Error, Span::new(span.start, end)));
            i = j + 1;
            continue;
        }
        if opens(tok) {
            depth += 1;
        } else if closes(tok) {
            depth = depth.saturating_sub(1);
        }
        out.push((tok.clone(), *span));
        i += 1;
    }
    (out, diags)
}

/// The result of [`parse`]: always a file (partial on error) and every
/// diagnostic, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file: File,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a source file given as bytes: invalid UTF-8 is one diagnostic and an
/// empty file.
pub fn parse_bytes(bytes: &[u8]) -> ParseResult {
    match std::str::from_utf8(bytes) {
        Ok(src) => parse(src),
        Err(_) => ParseResult {
            file: File { inner_docs: Vec::new(), items: Vec::new(), span: Loc(Span::new(0, 0)) },
            diagnostics: lex_bytes(bytes).1,
        },
    }
}

/// Parse a whole source file.
pub fn parse(src: &str) -> ParseResult {
    let (tokens, diags) = lex_limited(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    let inner_docs = select! { Token::DocInner(text) => text }
        .map_with(|text, e| Doc { text, span: Loc(e.span()) })
        .repeated()
        .collect::<Vec<_>>();
    // A closing delimiter with no opener is skipped and reported once, so the
    // items after it still parse.
    let stray = any()
        .filter(|t: &Token| matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace)))
        .validate(|t, e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                Custom {
                    code: "E0900",
                    message: format!("unmatched {t}"),
                    help: Some("remove it, or add the opening delimiter".into()),
                },
            ));
            Item { meta: Meta::default(), public: false, kind: ItemKind::Error, span: Loc(e.span()) }
        });
    let file = inner_docs
        .then(first_of(vec![h.item.clone().boxed(), stray.boxed()]).repeated().collect::<Vec<_>>())
        .then_ignore(end())
        .map_with(|(inner_docs, items), e| File { inner_docs, items, span: Loc(e.span()) });
    let (file, mut diagnostics) = finish(file.parse(input).into_output_errors(), diags, &tokens);
    let file = file.unwrap_or(File {
        inner_docs: Vec::new(),
        items: Vec::new(),
        span: Loc(Span::new(0, src.len())),
    });
    diagnostics.extend(check::file(&file));
    diagnostics.sort_by_key(|d| d.span().start);
    ParseResult { file, diagnostics }
}

/// Labels of the positions that take a name or a value.
const NAME_LABELS: &[&str] = &["identifier", "pattern", "path", "expression", "type"];

/// A keyword or `Unknown` found where a name was expected.
fn names_expected(e: &Err<'_>, found: &Token) -> bool {
    matches!(found, Token::Kw(_) | Token::Unknown)
        && e.expected().any(|p| matches!(p, RichPattern::Label(l) if NAME_LABELS.contains(&&**l)))
}

fn name_error(found: &Token, span: Span) -> Diagnostic {
    match found {
        Token::Unknown => Diagnostic::new(
            "E0908",
            "`Unknown` is a literal, not a name",
            span,
            "cannot name a binding, item or field `Unknown`",
        )
        .with_help("choose another name"),
        Token::Kw(Kw::Unsafe | Kw::Async | Kw::Await | Kw::Dyn) => Diagnostic::new(
            "E0101",
            format!("{found} is not part of the language"),
            span,
            "a syntactic form left out of the language",
        )
        .with_help("values are immutable and owned; Part I §1 rule 2 lists the forms left out"),
        other => Diagnostic::new(
            "E0903",
            format!("{other} is a reserved keyword and cannot be used as an identifier"),
            span,
            "reserved keyword",
        )
        .with_help("choose another name; raw identifiers (`r#name`) are not supported"),
    }
}
