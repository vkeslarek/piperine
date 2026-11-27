//! Public entry points: lex, bound, parse, and turn parse errors into diagnostics.


use super::*;

/// The most alternatives a message lists before it says "or N more".

/// The deepest nesting the parser descends into (E0910).
pub const MAX_DEPTH: usize = 256;

/// The most links of one chain (binary operators, `.`, calls, indexing, `as`,
/// `?`, `else if` arms) on one path of an expression (E0910). A chain is flat
/// in the source, not nesting; the walkers that meet its depth grow the stack.
pub const MAX_CHAIN: usize = 4096;

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
    /// The level being read.
    current: Level,
    /// The enclosing levels, outermost first: the open delimiters.
    outer: Vec<Level>,
    nest: usize,
    link: usize,
}

impl Depth {
    fn new() -> Depth {
        Depth { current: Level::default(), outer: Vec::new(), nest: 0, link: 0 }
    }

    /// The level being read.
    fn top(&mut self) -> &mut Level {
        &mut self.current
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
        self.outer.push(std::mem::take(&mut self.current));
    }

    fn close(&mut self) {
        if let Some(parent) = self.outer.pop() {
            let level = std::mem::replace(&mut self.current, parent);
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

/// The result of [`parse`]: always a file (partial on error) and every
/// diagnostic, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file: File,
    pub diagnostics: Vec<Diagnostic>,
}

/// The entry point of the parser: source text or bytes in, a tree and every diagnostic out.
pub struct SourceParser;

impl SourceParser {
    /// Whether `e` is a `place` (B.3.3): a carrier path followed by `[expr]` and
    /// `.name` steps. A parenthesized place is not one. The one decision for the
    /// definition and the commit alike.
    pub(crate) fn is_place(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Path(_) => true,
            ExprKind::Index { expr, .. } => SourceParser::is_place(expr),
            // `.IDENT` only: a tuple index (`q.0`) names no carrier member.
            ExprKind::Field { expr, name } => !name.name.starts_with(|c: char| c.is_ascii_digit()) && SourceParser::is_place(expr),
            _ => false,
        }
    }

    /// Classify `t` given the token before it. Delimiters and `<`/`>` are not
    /// classified here.
    fn classify(t: &Token, prev: Option<&Token>) -> Count {
        use Punct::*;
        let infix = prev.is_some_and(TokenInput::ends_operand);
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
            if TokenInput::is_opener(t) {
                nest += 1;
            } else if TokenInput::is_closer(t) {
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

    /// For each `<` token, whether a `>` of the same delimiter level and statement
    /// closes it: only then can it open a generic argument list. A `<` that no `>`
    /// closes is a comparison, so a list of comparisons never accumulates levels.
    fn angle_pairs(tokens: &[(Token, Span)]) -> Vec<bool> {
        let mut paired = vec![false; tokens.len()];
        let mut current: Vec<usize> = Vec::new();
        let mut outer: Vec<Vec<usize>> = Vec::new();
        for (i, (t, _)) in tokens.iter().enumerate() {
            match t {
                t if TokenInput::is_opener(t) => outer.push(std::mem::take(&mut current)),
                t if TokenInput::is_closer(t) => {
                    if let Some(parent) = outer.pop() {
                        current = parent;
                    }
                }
                Token::Punct(Punct::Semi) => current.clear(),
                Token::Punct(Punct::Lt) => current.push(i),
                Token::Punct(Punct::Gt) => {
                    if let Some(lt) = current.pop() {
                        paired[lt] = true;
                    }
                }
                _ => {}
            }
        }
        paired
    }

    /// Lex `src`, then bound the tree: a nesting level is a delimiter, a prefix
    /// form or a type argument list; a chain link is an infix or postfix token
    /// (see [`classify`]), counted since the last `;`, `,` or end of a statement.
    /// Past [`MAX_DEPTH`] levels, or [`MAX_CHAIN`] links summed along one path, the
    /// group or the rest of the chain is replaced by one [`Token::Error`] and
    /// `E0910` is reported, so the parser stays shallow in nesting and every tree
    /// walk grows its stack only along a chain.
    pub(crate) fn lex_limited(src: &str) -> (Vec<(Token, Span)>, Vec<Diagnostic>) {
        let (tokens, mut diags) = Lexer::lex(src);
        let tokens = ParseDiagnostics::drop_dangling_docs(tokens, &mut diags);
        // An unterminated string or comment swallows the rest of the file: the
        // delimiters it hid are not a second fault.
        if !diags.iter().any(|d| d.message.starts_with("unterminated")) {
            diags.extend(ParseDiagnostics::unclosed_delimiters(&tokens, src.len()));
        }
        let closes_a_list = SourceParser::angle_pairs(&tokens);
        let mut out = Vec::with_capacity(tokens.len());
        let mut depth = Depth::new();
        let mut i = 0;
        while i < tokens.len() {
            let (tok, span) = &tokens[i];
            let prev = i.checked_sub(1).map(|j| &tokens[j].0);
            // The rest of a chain is replaced by one error token.
            let mut cut = |depth: &mut Depth, diags: &mut Vec<Diagnostic>, nesting: bool| {
                let end = SourceParser::skip_overflow(&tokens, i, false);
                let last = tokens.get(end.saturating_sub(1)).map_or(span.end, |(_, s)| s.end);
                if nesting { ParseDiagnostics::overflow(diags, *span) } else { ParseDiagnostics::chain_overflow(diags, *span) }
                out.push((Token::Error, Span::new(span.start, last)));
                depth.boundary(false);
                end
            };
            if TokenInput::is_opener(tok) {
                // A call or index chained after `)`/`]` adds a link to its operand.
                let after_closer = prev.is_some_and(|t| matches!(t, Token::Punct(Punct::RParen | Punct::RBracket)));
                let link = usize::from(after_closer && !matches!(tok, Token::Punct(Punct::LBrace)));
                if depth.nest + 1 > MAX_DEPTH {
                    let end = SourceParser::skip_overflow(&tokens, i, true);
                    let last = tokens.get(end.saturating_sub(1)).map_or(span.end, |(_, s)| s.end);
                    ParseDiagnostics::overflow(&mut diags, *span);
                    out.push((Token::Error, Span::new(span.start, last)));
                    i = end;
                    continue;
                }
                if depth.link + link > MAX_CHAIN {
                    i = cut(&mut depth, &mut diags, false);
                    continue;
                }
                depth.open(link);
            } else if TokenInput::is_closer(tok) {
                depth.close();
                let continues = tokens.get(i + 1).is_some_and(|(t, _)| SourceParser::classify(t, Some(tok)) != Count::None);
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
                let type_args = closes_a_list[i]
                    && (!prev.is_some_and(TokenInput::ends_operand)
                        || matches!(prev, Some(Token::Ident(_) | Token::RawIdent(_) | Token::Kw(Kw::SelfUpper))));
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
                match SourceParser::classify(tok, prev) {
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

    /// Parse a source file given as bytes: invalid UTF-8 is one diagnostic and an
    /// empty file.
    pub fn parse_bytes(bytes: &[u8]) -> ParseResult {
        match std::str::from_utf8(bytes) {
            Ok(src) => SourceParser::parse(src),
            Err(_) => ParseResult {
                file: File { inner_docs: Vec::new(), items: Vec::new(), span: Loc(Span::new(0, 0)) },
                diagnostics: Lexer::lex_bytes(bytes).1,
            },
        }
    }

    /// Parse a whole source file.
    pub fn parse(src: &str) -> ParseResult {
        let (tokens, diags) = SourceParser::lex_limited(src);
        let input = TokenInput::over(&tokens, src.len());
        let h = Handles::declare();
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
            .then(TokenInput::first_of(vec![h.item.clone().boxed(), stray.boxed()]).repeated().collect::<Vec<_>>())
            .then_ignore(end())
            .map_with(|(inner_docs, items), e| File { inner_docs, items, span: Loc(e.span()) });
        let (file, mut diagnostics) = ParseDiagnostics::finish(crate::stack::Stack::deep(|| file.parse(input).into_output_errors()), diags, &tokens);
        let file = file.unwrap_or(File {
            inner_docs: Vec::new(),
            items: Vec::new(),
            span: Loc(Span::new(0, src.len())),
        });
        diagnostics.extend(ParseCheck::file(&file));
        diagnostics.sort_by_key(|d| d.span().start);
        ParseResult { file, diagnostics }
    }
}
