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
        RichReason::ExpectedFound { found: Some(t), .. } if names_expected(e, &t) => name_error(&t, span),
        RichReason::ExpectedFound { found: None, .. } if at_span().is_some_and(|t| names_expected(e, &t)) => {
            name_error(&at_span().unwrap_or(Token::Error), span)
        }
        RichReason::ExpectedFound { found, .. } => {
            let found = match found {
                Some(t) => t.to_string(),
                None => "end of input".to_string(),
            };
            let expected: Vec<String> = e.expected().map(|p| p.to_string()).collect();
            let message = if expected.is_empty() {
                format!("unexpected {found}")
            } else {
                format!("unexpected {found}, expected {}", expected.join(", "))
            };
            Diagnostic::new("E0900", message, span, format!("found {found}"))
        }
    }
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
    (value, diags)
}

/// Parse `src` as one type. Lexical diagnostics come first-class in the result.
pub fn parse_type(src: &str) -> Parsed<Type> {
    big_stack(|| {
        let (tokens, diags) = lex_limited(src);
        let input = token_input(&tokens, src.len());
        let h = handles();
        finish(ty::build(&h).then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
    })
}

/// Parse `src` as one pattern.
pub fn parse_pat(src: &str) -> Parsed<Pat> {
    big_stack(|| {
        let (tokens, diags) = lex_limited(src);
        let input = token_input(&tokens, src.len());
        let h = handles();
        finish(h.pat.clone().then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
    })
}

/// Parse `src` as generic parameters (`<T: Repr = bool, const N: u32>`).
pub fn parse_generics(src: &str) -> Parsed<Generics> {
    big_stack(|| {
        let (tokens, diags) = lex_limited(src);
        let input = token_input(&tokens, src.len());
        let h = handles();
        finish(ty::generics(&h).then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
    })
}

/// Parse `src` as one expression.
pub fn parse_expr(src: &str) -> Parsed<Expr> {
    big_stack(|| {
        let (tokens, diags) = lex_limited(src);
        let input = token_input(&tokens, src.len());
        let h = handles();
        finish(h.expr.clone().then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
    })
}

/// Parse `src` as one braced block `{ stmt* }`.
pub fn parse_block(src: &str) -> Parsed<Block> {
    big_stack(|| {
        let (tokens, diags) = lex_limited(src);
        let input = token_input(&tokens, src.len());
        let h = handles();
        finish(h.block.clone().then_ignore(end()).parse(input).into_output_errors(), diags, &tokens)
    })
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

/// Stack for the parser thread. Parsing recurses through one function per
/// grammar layer, and a debug build spends tens of kilobytes per nesting
/// level, so a source at the depth limit needs far more than the default.
/// The stack is address space, committed only as it is touched.
const PARSER_STACK: usize = 512 << 20;

/// Run `f` on a thread with [`PARSER_STACK`]; inline when that thread cannot
/// be created.
fn big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    let slot = std::sync::Mutex::new(Some(f));
    let take = || slot.lock().ok().and_then(|mut g| g.take());
    std::thread::scope(|scope| {
        let spawned = std::thread::Builder::new()
            .stack_size(PARSER_STACK)
            .spawn_scoped(scope, || take().map(|f| f()));
        match spawned {
            Ok(handle) => match handle.join() {
                Ok(Some(out)) => out,
                Ok(None) => unreachable!("the parse closure runs once"),
                Err(panic) => std::panic::resume_unwind(panic),
            },
            Err(_) => take().map(|f| f()).expect("the parse closure runs once"),
        }
    })
}

/// The result of [`parse`]: always a file (partial on error) and every
/// diagnostic, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file: File,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a whole source file.
pub fn parse(src: &str) -> ParseResult {
    big_stack(|| {
        let (tokens, diags) = lex_limited(src);
        let input = token_input(&tokens, src.len());
        let h = handles();
        let inner_docs = select! { Token::DocInner(text) => text }
            .map_with(|text, e| Doc { text, span: Loc(e.span()) })
            .repeated()
            .collect::<Vec<_>>();
        let file = inner_docs
            .then(h.item.clone().repeated().collect::<Vec<_>>())
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
    })
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
        ),
        other => Diagnostic::new(
            "E0903",
            format!("{other} is a reserved keyword and cannot be used as an identifier"),
            span,
            "reserved keyword",
        )
        .with_help("choose another name; raw identifiers (`r#name`) are not supported"),
    }
}
