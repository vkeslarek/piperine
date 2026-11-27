//! Parse diagnostics: the spec's [`Diagnostic`] of a parse error, the removed forms of B.3.6, and the
//! reports of the token stream (unclosed delimiters, dangling docs, nesting overflow).

use chumsky::error::{RichPattern, RichReason};

use super::*;

/// Labels of the positions that take a name or a value.
const NAME_LABELS: &[&str] = &["identifier", "pattern", "path", "expression", "type"];

/// The most alternatives a diagnostic names as expected.
const MAX_EXPECTED: usize = 6;

/// The primary label of an unclosed-delimiter diagnostic.
pub(crate) const UNCLOSED: &str = "this delimiter is never closed";

/// The builder of every diagnostic of the parse.
pub(crate) struct ParseDiagnostics;

impl ParseDiagnostics {
    /// The diagnostic of a removed form, by the start of its B.3.6 row.
    pub(crate) fn removed(row: &str, span: Span) -> Err<'static> {
        let f = REMOVED_FORMS.iter().find(|f| f.row == row).unwrap_or_else(|| panic!("B.3.6 has no row `{row}`"));
        Rich::custom(span, Custom { code: f.code, message: f.message.into(), help: Some(f.help.into()) })
    }

    /// The diagnostic of a left side that is not a `place`.
    pub(crate) fn not_a_place() -> Custom {
        Custom {
            code: "E0900",
            message: "expected a place: a carrier, `x[i]` or `x.f`".into(),
            help: Some("a place takes no parentheses and no operators: write `q`, `q[i]` or `q.f`".into()),
        }
    }

    /// The `E0901` of a macro invocation or definition.
    pub(crate) fn macro_error(span: Span) -> Err<'static> {
        Rich::custom(
            span,
            Custom {
                code: "E0901",
                message: "macros are not part of the language".into(),
                help: Some("the only `!` forms are the built-ins `emit!` and `include_str!`".into()),
            },
        )
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
                ParseDiagnostics::name_error(&t, span)
            }
            // A Rust keyword no production of the language accepts anywhere.
            RichReason::ExpectedFound { found: Some(t), .. } if matches!(&**t, Token::Kw(k) if ParseDiagnostics::unused_keyword(*k)) => {
                ParseDiagnostics::name_error(&t, span)
            }
            RichReason::ExpectedFound { found: Some(t), .. } if ParseDiagnostics::names_expected(e, &t) => ParseDiagnostics::name_error(&t, span),
            RichReason::ExpectedFound { found: None, .. } if at_span().is_some_and(|t| ParseDiagnostics::names_expected(e, &t)) => {
                ParseDiagnostics::name_error(&at_span().unwrap_or(Token::Error), span)
            }
            RichReason::ExpectedFound { found, .. } => {
                // A found token the error did not keep is recovered from its span.
                let found = match found {
                    Some(t) => Some((**t).clone()),
                    None => at_span(),
                };
                let found = found.map_or_else(|| "end of input".to_string(), |t| t.to_string());
                let expected = ParseDiagnostics::summarize_expected(e);
                let message = if expected.is_empty() {
                    format!("unexpected {found}")
                } else {
                    format!("unexpected {found}, expected {}", ParseDiagnostics::join_alternatives(&expected))
                };
                Diagnostic::new("E0900", message, span, format!("found {found}"))
            }
        }
    }

    /// A token that continues an expression: an operator or a postfix form.
    pub(crate) fn is_operator(t: &Token) -> bool {
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
    pub(crate) fn summarize_expected(e: &Err<'_>) -> Vec<String> {
        let mut ranked: Vec<(u8, String)> = Vec::new();
        let mut operators = 0;
        let mut expression = false;
        for p in e.expected() {
            match p {
                RichPattern::Token(t) => {
                    if ParseDiagnostics::is_operator(t) {
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
                    if ParseDiagnostics::is_operator(t) {
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
    pub(crate) fn join_alternatives(items: &[String]) -> String {
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

    /// The result of a parse: the value, and the lexical diagnostics merged with the parser's errors,
    /// the one fault seen twice reported once, in source order.
    pub(crate) fn finish<O>(
        out: (Option<O>, Vec<Err<'_>>),
        mut diags: Vec<Diagnostic>,
        tokens: &[(Token, Span)],
    ) -> Parsed<O> {
        let (value, errors) = out;
        // A `Token::Error` was already reported by the lexer or the depth check.
        let reported = |e: &Err<'_>| {
            matches!(e.reason(), RichReason::ExpectedFound { found: Some(t), .. } if matches!(&**t, Token::Error))
        };
        // An error at the end of input after a lexical error is that same fault
        // seen again; after an unclosed delimiter it is the same fault too, and
        // the diagnostic that names the opening position replaces it. Without
        // such an error the parser already named the fault: the unclosed-delimiter
        // reports are dropped.
        let at_eof = |e: &Err<'_>| {
            matches!(e.reason(), RichReason::ExpectedFound { found: None, .. })
                && !tokens.iter().any(|(_, s)| s == e.span())
        };
        let unclosed = |d: &Diagnostic| d.primary.1 == UNCLOSED;
        let eof_error = errors.iter().any(|e| at_eof(e));
        let ended_early = eof_error && diags.iter().any(|d| unclosed(d) || d.message.starts_with("unterminated"))
            || diags.iter().any(|d| d.message.starts_with("unterminated"));
        let mut parsed: Vec<Diagnostic> = errors
            .iter()
            .filter(|e| !reported(e) && !(ended_early && at_eof(e)))
            .map(|e| ParseDiagnostics::to_diagnostic(e, tokens))
            .collect();
        // The first parser error inside an unclosed group is that same fault
        // seen from within (recovery resumes at the next `;`): the diagnostic at
        // the opener replaces it, and stays.
        let mut absorbed = Vec::new();
        for u in diags.iter().filter(|d| unclosed(d)) {
            // Only a group that another closing delimiter met: one that ran to the
            // end of input is reported by the parser where it stopped.
            let Some((met, _)) = u.secondary.first().filter(|(_, label)| label.starts_with("reached")) else { continue };
            let end = met.start;
            let inside = parsed.iter().position(|d| d.span().start > u.span().start && d.span().start <= end);
            if let Some(at) = inside {
                parsed.remove(at);
                absorbed.push(u.span());
            }
        }
        diags.retain(|d| !unclosed(d) || eof_error || absorbed.contains(&d.span()));
        diags.extend(parsed);
        diags.sort_by_key(|d| d.span().start);
        // Recovery can report one position twice (a rewound alternative and the
        // enclosing group): one diagnostic per code and span.
        diags.dedup_by(|b, a| a.code == b.code && a.span() == b.span());
        (value, diags)
    }

    /// `E0910` for nesting deeper than [`MAX_DEPTH`] levels.
    pub(crate) fn overflow(diags: &mut Vec<Diagnostic>, span: Span) {
        diags.push(
            Diagnostic::new(
                "E0910",
                format!("nesting deeper than {MAX_DEPTH} levels"),
                span,
                "the parser stops descending here",
            )
            .with_help("split the expression into `let` bindings or functions"),
        );
    }

    /// `E0910` for an expression chain longer than [`MAX_CHAIN`] links.
    pub(crate) fn chain_overflow(diags: &mut Vec<Diagnostic>, span: Span) {
        diags.push(
            Diagnostic::new(
                "E0910",
                format!("an expression chain longer than {MAX_CHAIN} links"),
                span,
                "the parser stops here",
            )
            .with_help("split the expression into `let` bindings or functions"),
        );
    }

    /// The closing delimiter that matches an opener.
    pub(crate) fn closer_of(t: &Token) -> &'static str {
        match t {
            Token::Punct(Punct::LParen) => ")",
            Token::Punct(Punct::LBracket) => "]",
            _ => "}",
        }
    }

    /// Unclosed delimiters, each reported at its opener (with the closer that
    /// met it or the end of input as a second label), as `rustc` does. A closer
    /// that matches no opener is left to the parser (`unmatched`).
    pub(crate) fn unclosed_delimiters(tokens: &[(Token, Span)], src_len: usize) -> Vec<Diagnostic> {
        let kind = |t: &Token| match t {
            Token::Punct(Punct::LParen | Punct::RParen) => 0,
            Token::Punct(Punct::LBracket | Punct::RBracket) => 1,
            _ => 2,
        };
        let report = |open: usize, found: Option<&Token>, at: Span| {
            let (t, span) = &tokens[open];
            let closer = ParseDiagnostics::closer_of(t);
            let (found, label) = match found {
                Some(f) => (f.to_string(), "reached this closing delimiter first"),
                None => ("end of input".to_string(), "the input ends here"),
            };
            Diagnostic::new("E0900", format!("unexpected {found}, expected `{closer}`"), *span, UNCLOSED)
                .with_label(at, label)
                .with_help(format!("add the closing `{closer}` for {t}"))
        };
        let mut out = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for (i, (t, span)) in tokens.iter().enumerate() {
            if TokenInput::is_opener(t) {
                stack.push(i);
            } else if TokenInput::is_closer(t) {
                let Some(depth) = stack.iter().rposition(|&o| kind(&tokens[o].0) == kind(t)) else { continue };
                for open in stack.drain(depth..).skip(1) {
                    out.push(report(open, Some(t), *span));
                }
            }
        }
        for open in stack {
            out.push(report(open, None, Span::new(src_len, src_len)));
        }
        out
    }

    /// A run of outer doc comments followed by `}`, `else` or the end of input documents
    /// nothing: one `E0900` at the run, and the comments leave the stream so the
    /// parser reports nothing more.
    pub(crate) fn drop_dangling_docs(tokens: Vec<(Token, Span)>, diags: &mut Vec<Diagnostic>) -> Vec<(Token, Span)> {
        let is_doc = |t: &Token| matches!(t, Token::DocOuter(_) | Token::DocOuterBlock(_));
        let mut out = Vec::with_capacity(tokens.len());
        let mut i = 0;
        while i < tokens.len() {
            if !is_doc(&tokens[i].0) {
                out.push(tokens[i].clone());
                i += 1;
                continue;
            }
            let mut end = i;
            while end < tokens.len() && is_doc(&tokens[end].0) {
                end += 1;
            }
            let dangling = tokens.get(end).is_none_or(|(t, _)| matches!(t, Token::Punct(Punct::RBrace) | Token::Kw(Kw::Else)));
            if dangling {
                let span = Span::new(tokens[i].1.start, tokens[end - 1].1.end);
                diags.push(
                    Diagnostic::new("E0900", "this doc comment documents nothing", span, "no item follows it")
                        .with_help("put the comment before an item, a field, a statement or a reaction, or make it a `//` comment"),
                );
            } else {
                out.extend(tokens[i..end].iter().cloned());
            }
            i = end;
        }
        out
    }

    /// A Rust strict or reserved keyword that no production of MSL uses (E0903
    /// wherever one is found); `unsafe`, `async`, `await` and `dyn` are E0101.
    pub(crate) fn unused_keyword(k: Kw) -> bool {
        use Kw::*;
        matches!(
            k,
            Type | Static | Mut | Ref | Move | Abstract | Become | Box | Do | Final | Macro | Override | Priv
                | Typeof | Unsized | Virtual | Yield | Try | Gen
        )
    }

    /// The seven keywords the language adds, the only words with a raw identifier (A.1 rule 6).
    pub(crate) fn added_keyword(k: Kw) -> bool {
        use Kw::*;
        matches!(k, Module | Nature | Signal | On | Out | Flip | After)
    }

    /// A keyword found where a name was expected.
    pub(crate) fn names_expected(e: &Err<'_>, found: &Token) -> bool {
        matches!(found, Token::Kw(_))
            && e.expected().any(|p| matches!(p, RichPattern::Label(l) if NAME_LABELS.contains(&&**l)))
    }

    /// The diagnostic of a keyword found where a name was expected: `E0101` for a form the language
    /// leaves out, `E0903` for a reserved word.
    pub(crate) fn name_error(found: &Token, span: Span) -> Diagnostic {
        match found {
            Token::Kw(Kw::Unsafe | Kw::Async | Kw::Await | Kw::Dyn) => Diagnostic::new(
                "E0101",
                format!("{found} is not part of the language"),
                span,
                "a syntactic form left out of the language",
            )
            .with_help("Part I §1 rule 2 lists the forms left out"),
            other => {
                let help = match other {
                    Token::Kw(k) if ReservedMarker::of(*k).is_some() => ReservedMarker::of(*k).map_or_else(String::new, |m| m.help.to_string()),
                    Token::Kw(k) if ParseDiagnostics::added_keyword(*k) => format!("choose another name, or write the keyword as a raw identifier: `r#{}`", other.to_string().trim_matches('`')),
                    _ => "choose another name".to_string(),
                };
                Diagnostic::new(
                    "E0903",
                    format!("{other} is a reserved keyword and cannot be used as an identifier"),
                    span,
                    "reserved keyword",
                )
                .with_help(help)
            }
        }
    }
}
