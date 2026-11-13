//! Public entry points: lex, parse, and turn parse errors into diagnostics.

use chumsky::error::RichReason;

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
pub(crate) fn to_diagnostic(e: &Err<'_>) -> Diagnostic {
    let span = *e.span();
    match e.reason() {
        RichReason::Custom(c) => {
            let mut d = Diagnostic::new(c.code, c.message.clone(), span, c.message.clone());
            d.help = c.help.clone();
            d
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

fn finish<O>(out: (Option<O>, Vec<Err<'_>>), mut diags: Vec<Diagnostic>) -> Parsed<O> {
    let (value, errors) = out;
    diags.extend(errors.iter().map(to_diagnostic));
    diags.sort_by_key(|d| d.span().start);
    (value, diags)
}

/// Parse `src` as one type. Lexical diagnostics come first-class in the result.
pub fn parse_type(src: &str) -> Parsed<Type> {
    let (tokens, diags) = lex(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(ty::build(&h).then_ignore(end()).parse(input).into_output_errors(), diags)
}

/// Parse `src` as one pattern.
pub fn parse_pat(src: &str) -> Parsed<Pat> {
    let (tokens, diags) = lex(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(h.pat.clone().then_ignore(end()).parse(input).into_output_errors(), diags)
}

/// Parse `src` as generic parameters (`<T: Repr = bool, const N: u32>`).
pub fn parse_generics(src: &str) -> Parsed<Generics> {
    let (tokens, diags) = lex(src);
    let input = token_input(&tokens, src.len());
    let h = handles();
    finish(ty::generics(&h).then_ignore(end()).parse(input).into_output_errors(), diags)
}
