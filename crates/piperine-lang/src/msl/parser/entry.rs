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
        // A Rust keyword no production of the language accepts anywhere.
        RichReason::ExpectedFound { found: Some(t), .. } if matches!(&**t, Token::Kw(k) if unused_keyword(*k)) => {
            name_error(&t, span)
        }
        // `Unknown` where a declaration or field name goes: `Unknown: T = ..`.
        RichReason::ExpectedFound { found: Some(t), .. }
            if matches!(&**t, Token::Unknown)
                && tokens
                    .iter()
                    .position(|(_, s)| *s == span)
                    .and_then(|i| tokens.get(i + 1))
                    .is_some_and(|(n, _)| matches!(n, Token::Punct(Punct::Colon))) =>
        {
            name_error(&t, span)
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
        .map(|e| to_diagnostic(e, tokens))
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

/// The deepest nesting the parser descends into (E0910).
pub const MAX_DEPTH: usize = 256;

/// The most links of one chain (binary operators, `.`, calls, indexing, `as`,
/// `?`, `else if` arms) on one path of an expression (E0910). A chain is flat
/// in the source, not nesting; the walkers that meet its depth grow the stack.
pub const MAX_CHAIN: usize = 4096;

fn is_open(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace))
}

fn is_close(t: &Token) -> bool {
    matches!(t, Token::Punct(Punct::RParen | Punct::RBracket | Punct::RBrace))
}

/// Whether a token ends an operand, so the token after it is infix or
/// postfix; otherwise the next operator token is a prefix form.
fn ends_operand(t: &Token) -> bool {
    match t {
        Token::Ident(_) | Token::RawIdent(_) | Token::Int(_) | Token::Float { .. } | Token::Str { .. }
        | Token::Unknown | Token::StaticLifetime => true,
        Token::Kw(Kw::SelfLower | Kw::SelfUpper | Kw::True | Kw::False) => true,
        Token::Punct(p) => matches!(p, Punct::RParen | Punct::RBracket | Punct::RBrace | Punct::Question),
        _ => false,
    }
}

/// How a token counts against the two bounds of E0910.
#[derive(Clone, Copy, PartialEq)]
enum Count {
    /// Neither: separators, names, literals, `<`/`>` handled by the caller.
    None,
    /// One level of tree under the token: a prefix operator, a closure bar,
    /// `return`, `break`, `move`, `ref`, `mut`.
    Nest,
    /// One link of a flat chain: an infix operator, `.`, `?`, `as`, `else`.
    Link,
}

/// Classify `t` given the token before it. Delimiters and `<`/`>` are not
/// classified here.
fn classify(t: &Token, prev: Option<&Token>) -> Count {
    use Punct::*;
    let infix = prev.is_some_and(ends_operand);
    match t {
        Token::Kw(Kw::Return | Kw::Break | Kw::Move | Kw::Ref | Kw::Mut) => Count::Nest,
        Token::Kw(Kw::As | Kw::Else) => Count::Link,
        Token::Punct(Dot | Question) => Count::Link,
        Token::Punct(Minus | Not | Star | And | AndAnd | Or | OrOr | Tilde) if !infix => Count::Nest,
        Token::Punct(p) if !matches!(
            p,
            Comma | Semi | Colon | ColonColon | Arrow | FatArrow | Pound | At | Dollar | Underscore
                | LParen | RParen | LBracket | RBracket | LBrace | RBrace | Lt | Gt
        ) => Count::Link,
        _ => Count::None,
    }
}

/// Index where the skip of an overflowing group (`whole_group`: past its
/// closer) or of the rest of a chain (up to a `;`, `,` or closing delimiter
/// of the starting level, unconsumed) ends; nested groups are skipped whole.
fn skip_overflow(tokens: &[(Token, Span)], from: usize, whole_group: bool) -> usize {
    let mut nest = 0usize;
    let mut j = from;
    while j < tokens.len() {
        let t = &tokens[j].0;
        if is_open(t) {
            nest += 1;
        } else if is_close(t) {
            if nest == 0 {
                return j;
            }
            nest -= 1;
            if nest == 0 && whole_group {
                return j + 1;
            }
        } else if nest == 0 && !whole_group && matches!(t, Token::Punct(Punct::Semi | Punct::Comma)) {
            return j;
        }
        j += 1;
    }
    j
}

fn overflow(diags: &mut Vec<Diagnostic>, span: Span) {
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

fn chain_overflow(diags: &mut Vec<Diagnostic>, span: Span) {
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

/// The counts of one open delimiter level since its last boundary.
#[derive(Default)]
struct Level {
    nest: usize,
    link: usize,
    /// Of `nest`, the tentative levels opened by a `<` not yet closed.
    angles: usize,
    /// The first `=` of a level is a binding's, free of charge.
    eq_seen: bool,
}

/// The two E0910 counts along the path to the current token.
struct Depth {
    levels: Vec<Level>,
    nest: usize,
    link: usize,
}

impl Depth {
    fn new() -> Depth {
        Depth { levels: vec![Level::default()], nest: 0, link: 0 }
    }

    fn top(&mut self) -> &mut Level {
        self.levels.last_mut().expect("the outermost level is never popped")
    }

    /// A chain or statement ends: the level's counts are free again. `keep_angles`
    /// leaves the open `<` of a generic list that a `,` continues.
    fn boundary(&mut self, keep_angles: bool) {
        let (nest, link) = {
            let top = self.top();
            let kept = if keep_angles { top.angles } else { 0 };
            let taken = (top.nest - kept, top.link);
            top.nest = kept;
            top.angles = kept;
            top.link = 0;
            top.eq_seen = false;
            taken
        };
        self.nest -= nest;
        self.link -= link;
    }

    /// An operator that no type argument list holds: the pending `<` were comparisons.
    fn release_angles(&mut self) {
        let top = self.top();
        let angles = std::mem::take(&mut top.angles);
        top.nest -= angles;
        self.nest -= angles;
    }

    fn open(&mut self, link: usize) {
        self.top().link += link;
        self.link += link;
        self.nest += 1;
        self.levels.push(Level::default());
    }

    fn close(&mut self) {
        if self.levels.len() > 1 {
            let level = self.levels.pop().unwrap_or_default();
            self.nest -= level.nest + 1;
            self.link -= level.link;
        }
    }

    fn add(&mut self, count: Count) {
        let top = self.top();
        match count {
            Count::Nest => top.nest += 1,
            Count::Link => top.link += 1,
            Count::None => return,
        }
        match count {
            Count::Nest => self.nest += 1,
            _ => self.link += 1,
        }
    }

    fn angle(&mut self) {
        self.top().angles += 1;
        self.add(Count::Nest);
    }

    fn close_angle(&mut self) -> bool {
        if self.top().angles == 0 {
            return false;
        }
        let top = self.top();
        top.angles -= 1;
        top.nest -= 1;
        self.nest -= 1;
        true
    }
}

/// The primary label of an unclosed-delimiter diagnostic.
const UNCLOSED: &str = "this delimiter is never closed";

/// The closing delimiter that matches an opener.
fn closer_of(t: &Token) -> &'static str {
    match t {
        Token::Punct(Punct::LParen) => ")",
        Token::Punct(Punct::LBracket) => "]",
        _ => "}",
    }
}

/// Unclosed delimiters, each reported at its opener (with the closer that
/// met it or the end of input as a second label), as `rustc` does. A closer
/// that matches no opener is left to the parser (`unmatched`).
fn unclosed_delimiters(tokens: &[(Token, Span)], src_len: usize) -> Vec<Diagnostic> {
    let kind = |t: &Token| match t {
        Token::Punct(Punct::LParen | Punct::RParen) => 0,
        Token::Punct(Punct::LBracket | Punct::RBracket) => 1,
        _ => 2,
    };
    let report = |open: usize, found: Option<&Token>, at: Span| {
        let (t, span) = &tokens[open];
        let closer = closer_of(t);
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
        if is_open(t) {
            stack.push(i);
        } else if is_close(t) {
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

/// A run of outer doc comments followed by `}` or the end of input documents
/// nothing: one `E0900` at the run, and the comments leave the stream so the
/// parser reports nothing more.
fn drop_dangling_docs(tokens: Vec<(Token, Span)>, diags: &mut Vec<Diagnostic>) -> Vec<(Token, Span)> {
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
        let dangling = tokens.get(end).is_none_or(|(t, _)| matches!(t, Token::Punct(Punct::RBrace)));
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

/// Lex `src`, then bound the tree: a nesting level is a delimiter, a prefix
/// form or a type argument list; a chain link is an infix or postfix token
/// (see [`classify`]), counted since the last `;`, `,` or end of a statement.
/// Past [`MAX_DEPTH`] levels, or [`MAX_CHAIN`] links summed along one path, the
/// group or the rest of the chain is replaced by one [`Token::Error`] and
/// `E0910` is reported, so the parser stays shallow in nesting and every tree
/// walk grows its stack only along a chain.
fn lex_limited(src: &str) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
    let (tokens, mut diags) = lex(src);
    let tokens = drop_dangling_docs(tokens, &mut diags);
    // An unterminated string or comment swallows the rest of the file: the
    // delimiters it hid are not a second fault.
    if !diags.iter().any(|d| d.message.starts_with("unterminated")) {
        diags.extend(unclosed_delimiters(&tokens, src.len()));
    }
    let mut out = Vec::with_capacity(tokens.len());
    let mut depth = Depth::new();
    let mut i = 0;
    while i < tokens.len() {
        let (tok, span) = &tokens[i];
        let prev = i.checked_sub(1).map(|j| &tokens[j].0);
        // The rest of a chain is replaced by one error token.
        let mut cut = |depth: &mut Depth, diags: &mut Vec<Diagnostic>, nesting: bool| {
            let end = skip_overflow(&tokens, i, false);
            let last = tokens.get(end.saturating_sub(1)).map_or(span.end, |(_, s)| s.end);
            if nesting { overflow(diags, *span) } else { chain_overflow(diags, *span) }
            out.push((Token::Error, Span::new(span.start, last)));
            depth.boundary(false);
            end
        };
        if is_open(tok) {
            // A call or index chained after `)`/`]` adds a link to its operand.
            let after_closer = prev.is_some_and(|t| matches!(t, Token::Punct(Punct::RParen | Punct::RBracket)));
            let link = usize::from(after_closer && !matches!(tok, Token::Punct(Punct::LBrace)));
            if depth.nest + 1 > MAX_DEPTH {
                let end = skip_overflow(&tokens, i, true);
                let last = tokens.get(end.saturating_sub(1)).map_or(span.end, |(_, s)| s.end);
                overflow(&mut diags, *span);
                out.push((Token::Error, Span::new(span.start, last)));
                i = end;
                continue;
            }
            if depth.link + link > MAX_CHAIN {
                i = cut(&mut depth, &mut diags, false);
                continue;
            }
            depth.open(link);
        } else if is_close(tok) {
            depth.close();
            let continues = tokens.get(i + 1).is_some_and(|(t, _)| classify(t, Some(tok)) != Count::None);
            if matches!(tok, Token::Punct(Punct::RBrace)) && !continues {
                depth.boundary(false);
            }
        } else if matches!(tok, Token::Punct(Punct::Semi)) {
            depth.boundary(false);
        } else if matches!(tok, Token::Punct(Punct::Comma)) {
            depth.boundary(true);
        } else if matches!(tok, Token::Punct(Punct::Eq)) && !depth.top().eq_seen {
            depth.top().eq_seen = true;
        } else if matches!(tok, Token::Punct(Punct::Gt)) && depth.close_angle() {
        } else if matches!(tok, Token::Punct(Punct::Lt)) {
            let type_args = !prev.is_some_and(ends_operand)
                || matches!(prev, Some(Token::Ident(_) | Token::RawIdent(_) | Token::Kw(Kw::SelfUpper)));
            if type_args {
                if depth.nest + 1 > MAX_DEPTH {
                    i = cut(&mut depth, &mut diags, true);
                    continue;
                }
                depth.angle();
            } else {
                if depth.link + 1 > MAX_CHAIN {
                    i = cut(&mut depth, &mut diags, false);
                    continue;
                }
                depth.add(Count::Link);
            }
        } else {
            match classify(tok, prev) {
                Count::None => {}
                Count::Nest if depth.nest + 1 > MAX_DEPTH => {
                    i = cut(&mut depth, &mut diags, true);
                    continue;
                }
                Count::Link if depth.link + 1 > MAX_CHAIN => {
                    i = cut(&mut depth, &mut diags, false);
                    continue;
                }
                count => {
                    if count == Count::Link {
                        depth.release_angles();
                    }
                    depth.add(count);
                }
            }
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
    let inner_docs = select! { Token::DocInner(text) => (text, false), Token::DocInnerBlock(text) => (text, true) }
        .map_with(|(text, block), e| Doc { text, block, span: Loc(e.span()) })
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

/// A Rust strict or reserved keyword that no production of MSL uses (E0903
/// wherever one is found); `unsafe`, `async`, `await` and `dyn` are E0101.
fn unused_keyword(k: Kw) -> bool {
    use Kw::*;
    matches!(
        k,
        Type | Static | Abstract | Become | Box | Do | Final | Macro | Override | Priv | Typeof | Unsized
            | Virtual | Yield | Try | Gen
    )
}

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
