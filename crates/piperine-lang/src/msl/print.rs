//! Canonical printer: an [`ast::File`] back to MSL source text.
//!
//! The layout is fixed (design §print): 4-space indentation, one item per
//! blank-line-separated block, one statement or declaration per line, and
//! comma-separated lists on one line when they fit in 80 columns and hold no
//! line break, one element per line with a trailing comma otherwise. Doc
//! comments and attributes are kept; plain comments are not part of the
//! tree. Printing then parsing gives back an equal tree (spans aside), and
//! printing that tree gives the same text.
//!
//! A node's text is built bottom-up as a `String`; a multi-line child is
//! indented by its parent.
//!
//! # One node at a time
//!
//! The layout is a [`Printer`] over [`Hooks`]. Every container (a file, a
//! braced body, a block, a match, a comma-separated list) lays out its
//! children one by one: it asks [`Hooks::verbatim`] whether a child keeps the
//! source's text, gives each child's text (terminator included) to
//! [`Hooks::around`], and asks [`Hooks::inside`] for lines before its closing
//! delimiter. [`print`] runs the printer with hooks that change nothing; the
//! formatter (`msl::format`) runs it with hooks that put comments back.

use super::ast::*;
use super::span::Span;

const WIDTH: usize = 80;

/// Print a file in the canonical layout.
pub fn print(file: &File) -> String {
    Printer::new(&Canonical).file(file)
}

// ---- hooks ------------------------------------------------------------------------------------

/// How a container separates its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// Items of a file or a `mod`: always one blank line between two.
    Items,
    /// One child per line inside braces: members, statements, arms, fields.
    Lines,
    /// A comma-separated list: on one line when it fits, one element per line otherwise.
    List,
}

/// Where a container's closing delimiter stands in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Close {
    /// At this byte offset (a `}` the node's span ends with).
    At(usize),
    /// The first token after this offset (the last child's end) that is no `,` or `;`.
    After(usize),
    /// The end of the file.
    End,
    /// Not known: an empty container whose delimiters no span gives.
    Unknown,
}

/// What a container asks while it lays out its children. Text handed over
/// and returned is in the printer's escaped form (see `enc`).
pub(crate) trait Hooks {
    /// `Some(text)` when the child at `extent` is printed as the source
    /// writes it instead of in the printer's layout.
    fn verbatim(&self, extent: Span) -> Option<String>;
    /// The final text of the child at `extent`, from its laid out `text`
    /// (terminator included); `prev` is the previous sibling's extent.
    fn around(&self, extent: Span, text: String, prev: Option<Span>, shape: Shape) -> String;
    /// Lines to print before the closing delimiter of a container whose
    /// children have the extents `kids`.
    fn inside(&self, close: Close, kids: &[Span], shape: Shape) -> Vec<String>;
}

/// The hooks of [`print`]: the canonical layout, nothing added.
struct Canonical;

impl Hooks for Canonical {
    fn verbatim(&self, _: Span) -> Option<String> {
        None
    }

    fn around(&self, _: Span, text: String, _: Option<Span>, _: Shape) -> String {
        text
    }

    fn inside(&self, _: Close, _: &[Span], _: Shape) -> Vec<String> {
        Vec::new()
    }
}

/// One child of a container: its source range and its layout.
struct Kid<'k> {
    extent: Span,
    lay: Box<dyn Fn() -> String + 'k>,
}

impl<'k> Kid<'k> {
    fn new(extent: Span, lay: impl Fn() -> String + 'k) -> Kid<'k> {
        Kid { extent, lay: Box::new(lay) }
    }
}

/// The canonical layout over a set of [`Hooks`].
pub(crate) struct Printer<'h> {
    hooks: &'h dyn Hooks,
}

// ---- verbatim text ----------------------------------------------------------------------------
//
// A string literal or doc comment is printed as written: layout (indentation,
// line fitting) must neither see nor touch its line breaks. While a node's text
// is built, a `\0` marks an escape: `\0n`, `\0r` stand for a line break and a
// carriage return inside such text, `\00` for a NUL; [`print`] restores them.

fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\0' => out.push_str("\00"),
            '\n' => out.push_str("\0n"),
            '\r' => out.push_str("\0r"),
            c => out.push(c),
        }
    }
    out
}

fn dec(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\0' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            _ => out.push('\0'),
        }
    }
    out
}

/// A doc comment in its own form; `kind` is `"/"` (outer) or `"!"` (inner).
fn doc(d: &Doc, kind: &str) -> String {
    let text = enc(&d.text);
    if d.block {
        let star = if kind == "!" { "!" } else { "*" };
        format!("/*{star}{text}*/")
    } else {
        format!("//{kind}{text}")
    }
}

// ---- layout helpers ---------------------------------------------------------------------------

/// Indent every non-blank line of `s` by one level.
fn indent(s: &str) -> String {
    s.lines()
        .map(|l| if l.is_empty() { String::new() } else { format!("    {l}") })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `{ .. }` holding one entry per line, or `{}` when empty.
fn braces(lines: &[String]) -> String {
    if lines.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{}\n}}", indent(&lines.join("\n")))
    }
}

impl<'h> Printer<'h> {
    pub(crate) fn new(hooks: &'h dyn Hooks) -> Printer<'h> {
        Printer { hooks }
    }

    /// `text` in the escaped form a hook hands back: its line breaks are
    /// printed as written, never indented.
    pub(crate) fn escape(text: &str) -> String {
        enc(text)
    }

    /// The source range of a node with its documentation and attributes.
    fn extent(meta: &Meta, span: Loc) -> Span {
        let start = meta
            .docs
            .iter()
            .map(|d| d.span.0.start)
            .chain(meta.attrs.iter().map(|a| a.span.0.start))
            .fold(span.0.start, usize::min);
        Span::new(start, span.0.end)
    }

    /// The `}` a node's span ends with.
    fn last_brace(span: Loc) -> Option<usize> {
        span.0.end.checked_sub(1)
    }

    /// Where a container's closing delimiter is: `at` when a span gives it,
    /// else the first token after its last child.
    fn close(at: Option<usize>, kids: &[Span]) -> Close {
        match (at, kids.last()) {
            (Some(o), _) => Close::At(o),
            (None, Some(k)) => Close::After(k.end),
            (None, None) => Close::Unknown,
        }
    }

    /// A whole file: inner docs, then the items one blank line apart.
    pub(crate) fn file(&self, file: &File) -> String {
        let mut out = String::new();
        let mut prev = None;
        for d in &file.inner_docs {
            let text = self.hooks.around(d.span.0, doc(d, "!"), prev, Shape::Lines);
            out.push_str(&format!("{text}\n"));
            prev = Some(d.span.0);
        }
        if !file.inner_docs.is_empty() && !file.items.is_empty() {
            out.push('\n');
        }
        let kids = file.items.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.item(i))).collect();
        let (items, item_spans) = self.children(kids, "", Shape::Items);
        let spans: Vec<Span> = file.inner_docs.iter().map(|d| d.span.0).chain(item_spans).collect();
        let items: Vec<String> = items.into_iter().map(|(_, t)| t).collect();
        out.push_str(&items.join("\n\n"));
        if !items.is_empty() {
            out.push('\n');
        }
        for line in self.hooks.inside(Close::End, &spans, Shape::Items) {
            out.push_str(&format!("{line}\n"));
        }
        dec(&out)
    }

    /// The children of a container, each with its extent and final text (the
    /// terminator `term` added), and their extents.
    fn children(&self, kids: Vec<Kid<'_>>, term: &str, shape: Shape) -> (Vec<(Span, String)>, Vec<Span>) {
        let spans: Vec<Span> = kids.iter().map(|k| k.extent).collect();
        let mut out = Vec::with_capacity(kids.len());
        let mut prev = None;
        for k in kids {
            let text = match self.hooks.verbatim(k.extent) {
                Some(t) if t.ends_with(term) => t,
                Some(t) => format!("{t}{term}"),
                None => format!("{}{term}", (k.lay)()),
            };
            out.push((k.extent, self.hooks.around(k.extent, text, prev, shape)));
            prev = Some(k.extent);
        }
        (out, spans)
    }

    /// The lines of a braced container: its children, then what goes before
    /// its closing delimiter (`at`, when a span gives it).
    fn lines(&self, kids: Vec<Kid<'_>>, term: &str, at: Option<usize>) -> Vec<String> {
        let (out, spans) = self.children(kids, term, Shape::Lines);
        let mut lines: Vec<String> = out.into_iter().map(|(_, t)| t).collect();
        lines.extend(self.hooks.inside(Self::close(at, &spans), &spans, Shape::Lines));
        lines
    }

    /// `open items close`: on one line when it fits, one item per line
    /// otherwise (and whenever the hooks add to an item or before `close`).
    fn list(&self, open: &str, kids: Vec<Kid<'_>>, close_: &str) -> String {
        let spans: Vec<Span> = kids.iter().map(|k| k.extent).collect();
        let items: Vec<String> = kids.iter().map(|k| (k.lay)()).collect();
        let (lines, added) = self.list_lines(&items, &spans);
        let inline = items.join(", ");
        if !added && inline.len() + open.len() + close_.len() <= WIDTH && !inline.contains('\n') {
            return format!("{open}{inline}{close_}");
        }
        format!("{open}\n{}\n{close_}", lines.join("\n"))
    }

    /// The lines of a list printed one element per line, indented, and
    /// whether the hooks added anything.
    fn list_lines(&self, items: &[String], spans: &[Span]) -> (Vec<String>, bool) {
        let mut added = false;
        let mut lines = Vec::with_capacity(items.len());
        let mut prev = None;
        for (item, span) in items.iter().zip(spans) {
            let plain = format!("{item},");
            let text = self.hooks.around(*span, plain.clone(), prev, Shape::List);
            added |= text != plain;
            lines.push(indent(&text));
            prev = Some(*span);
        }
        let inside = self.hooks.inside(Self::close(None, spans), spans, Shape::List);
        added |= !inside.is_empty();
        lines.extend(inside.iter().map(|l| indent(l)));
        (lines, added)
    }

    fn ident(&self, i: &Ident) -> String {
        i.spelled()
    }

    fn meta(&self, m: &Meta) -> String {
        let mut out = String::new();
        for d in &m.docs {
            out.push_str(&format!("{}\n", doc(d, "/")));
        }
        for a in &m.attrs {
            out.push_str(&format!("{}\n", self.attr(a)));
        }
        out
    }

    fn attr(&self, a: &Attr) -> String {
        let input = match &a.input {
            AttrInput::None => String::new(),
            AttrInput::Eq(e) => format!(" = {}", self.expr(e)),
            AttrInput::List(args) => {
                let items: Vec<String> = args
                    .iter()
                    .map(|a| match a {
                        AttrArg::Path(p) => self.path(p),
                        AttrArg::Lit(l) => self.lit(l),
                        AttrArg::NameValue(n, l) => format!("{} = {}", n.spelled(), self.lit(l)),
                    })
                    .collect();
                format!("({})", items.join(", "))
            }
        };
        format!("#[{}{input}]", self.path(&a.path))
    }

    // ---- paths, generics, types ---------------------------------------------------------------

    fn path(&self, p: &Path) -> String {
        p.segments
            .iter()
            .map(|s| format!("{}{}", s.ident.spelled(), s.args.as_ref().map_or(String::new(), |g| self.generic_args(g))))
            .collect::<Vec<_>>()
            .join("::")
    }

    fn generic_args(&self, g: &GenericArgs) -> String {
        match g {
            GenericArgs::Angle { args, turbofish, .. } => {
                let items: Vec<String> = args
                    .iter()
                    .map(|a| match a {
                        GenericArg::Type(t) => self.ty(t),
                        GenericArg::Const(e) => self.expr(e),
                    })
                    .collect();
                // `<-` is the commit operator: a leading minus needs a space.
                let gap = if items.first().is_some_and(|i| i.starts_with('-')) { " " } else { "" };
                format!("{}<{gap}{}>", if *turbofish { "::" } else { "" }, items.join(", "))
            }
            GenericArgs::Paren { inputs, output, .. } => {
                let items: Vec<String> = inputs.iter().map(|t| self.ty(t)).collect();
                let ret = output.as_ref().map_or(String::new(), |o| format!(" -> {}", self.ty(o)));
                format!("({}){ret}", items.join(", "))
            }
        }
    }

    fn generics(&self, g: &Option<Generics>) -> String {
        let Some(g) = g else { return String::new() };
        let items: Vec<String> = g
            .params
            .iter()
            .map(|p| match &p.kind {
                GenericParamKind::Type { bounds, default } => {
                    let mut s = p.name.spelled();
                    if !bounds.is_empty() {
                        s.push_str(&format!(": {}", bounds.iter().map(|b| self.path(b)).collect::<Vec<_>>().join(" + ")));
                    }
                    if let Some(d) = default {
                        s.push_str(&format!(" = {}", self.ty(d)));
                    }
                    s
                }
                GenericParamKind::Const { ty: t, default } => {
                    let mut s = format!("const {}: {}", p.name.spelled(), self.ty(t));
                    if let Some(d) = default {
                        s.push_str(&format!(" = {}", self.expr(d)));
                    }
                    s
                }
            })
            .collect();
        format!("<{}>", items.join(", "))
    }

    fn ty(&self, t: &Type) -> String {
        match &t.kind {
            TypeKind::Path(p) => self.path(p),
            TypeKind::Array { elem, len } => format!("[{}; {}]", self.ty(elem), self.expr(len)),
            TypeKind::Tuple(items) => match items.as_slice() {
                [one] => format!("({},)", self.ty(one)),
                _ => format!("({})", items.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", ")),
            },
            TypeKind::Flip(inner) => format!("flip {}", self.ty(inner)),
            TypeKind::FnPtr { params, ret } => {
                let ret = ret.as_ref().map_or(String::new(), |r| format!(" -> {}", self.ty(r)));
                format!("fn({}){ret}", params.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", "))
            }
            TypeKind::Infer => "_".to_string(),
        }
    }

    // ---- patterns -----------------------------------------------------------------------------

    /// A pattern where alternatives are not allowed at the top: parenthesized.
    fn pat1(&self, p: &Pat) -> String {
        match p.kind {
            PatKind::Or(_) => format!("({})", self.pat(p)),
            _ => self.pat(p),
        }
    }

    fn pat(&self, p: &Pat) -> String {
        match &p.kind {
            PatKind::Wild => "_".to_string(),
            PatKind::Rest => "..".to_string(),
            PatKind::Ident { name, sub } => {
                let mut s = name.spelled();
                if let Some(sub) = sub {
                    s.push_str(&format!(" @ {}", self.pat1(sub)));
                }
                s
            }
            PatKind::Lit(e) => self.expr(e),
            PatKind::Path(p) => self.path(p),
            PatKind::Tuple(items) => match items.as_slice() {
                [one] => format!("({},)", self.pat(one)),
                _ => format!("({})", items.iter().map(|p| self.pat(p)).collect::<Vec<_>>().join(", ")),
            },
            PatKind::TupleStruct { path: p, pats } => {
                format!("{}({})", self.path(p), pats.iter().map(|p| self.pat(p)).collect::<Vec<_>>().join(", "))
            }
            PatKind::Struct { path: p, fields, rest } => {
                let mut items: Vec<String> = fields
                    .iter()
                    .map(|f| match &f.pat {
                        Some(q) => format!("{}: {}", f.name.spelled(), self.pat(q)),
                        None => f.name.spelled(),
                    })
                    .collect();
                if *rest {
                    items.push("..".to_string());
                }
                if items.is_empty() {
                    format!("{} {{}}", self.path(p))
                } else {
                    format!("{} {{ {} }}", self.path(p), items.join(", "))
                }
            }
            PatKind::Or(alts) => alts.iter().map(|p| self.pat1_alt(p)).collect::<Vec<_>>().join(" | "),
            PatKind::Range { lo, hi, inclusive } => {
                let op = if *inclusive { "..=" } else { ".." };
                format!(
                    "{}{op}{}",
                    lo.as_ref().map_or(String::new(), |e| self.expr(e)),
                    hi.as_ref().map_or(String::new(), |e| self.expr(e))
                )
            }
            PatKind::Array(items) => format!("[{}]", items.iter().map(|p| self.pat(p)).collect::<Vec<_>>().join(", ")),
        }
    }

    /// An alternative of an `|` pattern: itself never an alternative.
    fn pat1_alt(&self, p: &Pat) -> String {
        self.pat1(p)
    }

    // ---- expressions --------------------------------------------------------------------------

    fn lit(&self, l: &Lit) -> String {
        match &l.kind {
            LitKind::Int(s) => s.clone(),
            LitKind::Float { raw, .. } => raw.clone(),
            LitKind::Str { text, raw_hashes: Some(n) } => {
                let hashes = "#".repeat(usize::from(*n));
                let body = super::lexer::unescape(text).0;
                format!("r{hashes}\"{body}\"{hashes}")
            }
            LitKind::Str { text, raw_hashes: None } => format!("\"{}\"", enc(text)),
            LitKind::Bool(b) => b.to_string(),
        }
    }

    fn args<'a>(&'a self, a: &'a [Arg]) -> Vec<Kid<'a>> {
        a.iter()
            .map(|a| {
                Kid::new(a.span.0, move || match &a.name {
                    Some(n) => format!("{}: {}", n.spelled(), self.expr(&a.value)),
                    None => self.expr(&a.value),
                })
            })
            .collect()
    }

    fn exprs<'a>(&'a self, items: &'a [Expr]) -> Vec<Kid<'a>> {
        items.iter().map(|e| Kid::new(e.span.0, move || self.expr(e))).collect()
    }

    fn field_inits(&self, f: &[FieldInit]) -> String {
        if f.is_empty() {
            return "{}".to_string();
        }
        let items: Vec<String> = f
            .iter()
            .map(|f| match f.shorthand {
                true => f.name.spelled(),
                false => format!("{}: {}", f.name.spelled(), self.expr(&f.value)),
            })
            .collect();
        let spans: Vec<Span> = f.iter().map(|f| f.span.0).collect();
        let (lines, added) = self.list_lines(&items, &spans);
        let inline = format!("{{ {} }}", items.join(", "));
        if !added && inline.len() <= WIDTH && !inline.contains('\n') {
            inline
        } else {
            format!("{{\n{}\n}}", lines.join("\n"))
        }
    }

    fn expr(&self, e: &Expr) -> String {
        crate::stack::Stack::deep(|| self.expr_inner(e))
    }

    fn expr_inner(&self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Lit(l) => self.lit(l),
            ExprKind::Path(p) => self.path(p),
            ExprKind::Unary { op, expr: x } => {
                let sym = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                let inner = self.expr(x);
                // `- -x` is fine but kept apart.
                let sep = if sym == "-" && inner.starts_with('-') { " " } else { "" };
                format!("{sym}{sep}{inner}")
            }
            ExprKind::Binary { op, lhs, rhs } => format!("{} {} {}", self.expr(lhs), binop(*op), self.expr(rhs)),
            ExprKind::Assign { op, lhs, rhs } => {
                let sym = op.map_or("=".to_string(), |o| format!("{}=", binop(o)));
                format!("{} {sym} {}", self.expr(lhs), self.expr(rhs))
            }
            ExprKind::Cast { expr: x, ty: t } => format!("{} as {}", self.expr(x), self.ty(t)),
            ExprKind::Call { callee, args: a, params } => {
                let mut s = format!("{}{}", self.expr(callee), self.list("(", self.args(a), ")"));
                if let Some(p) = params {
                    s.push(' ');
                    s.push_str(&self.field_inits(p));
                }
                s
            }
            ExprKind::MethodCall { recv, method, generics: g, args: a } => {
                format!(
                    "{}.{}{}{}",
                    self.expr(recv),
                    method.spelled(),
                    g.as_ref().map_or(String::new(), |g| self.generic_args(g)),
                    self.list("(", self.args(a), ")")
                )
            }
            ExprKind::Field { expr: x, name } => format!("{}.{}", self.expr(x), name.spelled()),
            ExprKind::Index { expr: x, index } => format!("{}[{}]", self.expr(x), self.expr(index)),
            ExprKind::Range { lo, hi, inclusive } => format!(
                "{}{}{}",
                lo.as_ref().map_or(String::new(), |x| self.expr(x)),
                if *inclusive { "..=" } else { ".." },
                hi.as_ref().map_or(String::new(), |x| self.expr(x))
            ),
            ExprKind::Paren(x) => format!("({})", self.expr(x)),
            ExprKind::Tuple(items) => match items.as_slice() {
                [one] => format!("({},)", self.expr(one)),
                _ => self.list("(", self.exprs(items), ")"),
            },
            ExprKind::Array(items) => self.list("[", self.exprs(items), "]"),
            ExprKind::ArrayRepeat { elem, len } => format!("[{}; {}]", self.expr(elem), self.expr(len)),
            ExprKind::Struct { path: p, fields } => format!("{} {}", self.path(p), self.field_inits(fields)),
            ExprKind::Block(b) => self.block(b),
            ExprKind::If { cond, then, else_ } => {
                let mut s = format!("if {} {}", self.expr(cond), self.block(then));
                if let Some(x) = else_ {
                    s.push_str(&format!(" else {}", self.expr(x)));
                }
                s
            }
            ExprKind::Let { pat: p, expr: x } => format!("let {} = {}", self.pat(p), self.expr(x)),
            ExprKind::Match { scrutinee, arms } => {
                let kids = arms.iter().map(|a| Kid::new(Self::extent(&a.meta, a.span), move || self.arm(a))).collect();
                let lines = self.lines(kids, ",", Self::last_brace(e.span));
                format!("match {} {}", self.expr(scrutinee), braces(&lines))
            }
            ExprKind::While { cond, body } => format!("while {} {}", self.expr(cond), self.block(body)),
            ExprKind::Loop { body } => format!("loop {}", self.block(body)),
            ExprKind::For { pat: p, iter, body } => {
                format!("for {} in {} {}", self.pat1(p), self.expr(iter), self.block(body))
            }
            ExprKind::Break(v) => v.as_ref().map_or("break".to_string(), |x| format!("break {}", self.expr(x))),
            ExprKind::Continue => "continue".to_string(),
            ExprKind::Return(v) => v.as_ref().map_or("return".to_string(), |x| format!("return {}", self.expr(x))),
            ExprKind::Closure { params, ret, body } => {
                let ps: Vec<String> = params
                    .iter()
                    .map(|p| match &p.ty {
                        Some(t) => format!("{}: {}", self.pat1(&p.pat), self.ty(t)),
                        None => self.pat1(&p.pat),
                    })
                    .collect();
                let head = if ps.is_empty() { "||".to_string() } else { format!("|{}|", ps.join(", ")) };
                match ret {
                    Some(r) => format!("{head} -> {} {}", self.ty(r), self.expr(body)),
                    None => format!("{head} {}", self.expr(body)),
                }
            }
            ExprKind::Try(x) => format!("{}?", self.expr(x)),
            ExprKind::Emit { stream, fields } => {
                let mut kids = vec![Kid::new(stream.span.0, move || self.lit(stream))];
                kids.extend(
                    fields
                        .iter()
                        .map(|f| Kid::new(f.span.0, move || format!("{} = {}", f.name.spelled(), self.expr(&f.value)))),
                );
                format!("emit!{}", self.list("(", kids, ")"))
            }
            ExprKind::IncludeStr(l) => format!("include_str!({})", self.lit(l)),
            ExprKind::Error => "/* error */".to_string(),
        }
    }

    /// A match arm, without the `,` the match adds.
    fn arm(&self, a: &Arm) -> String {
        let mut s = self.attrs_only(&a.meta);
        s.push_str(&self.pat(&a.pat));
        if let Some(g) = &a.guard {
            s.push_str(&format!(" if {}", self.expr(g)));
        }
        s.push_str(&format!(" => {}", self.expr(&a.body)));
        s
    }

    /// Documentation and attributes, each on its own line, in front of a node.
    fn attrs_only(&self, m: &Meta) -> String {
        self.meta(m)
    }

    // ---- blocks and statements ----------------------------------------------------------------

    fn block(&self, b: &Block) -> String {
        braces(&self.lines(self.stmts(&b.stmts), "", Self::last_brace(b.span)))
    }

    fn stmts<'a>(&'a self, stmts: &'a [Stmt]) -> Vec<Kid<'a>> {
        stmts.iter().map(|s| Kid::new(Self::extent(&s.meta, s.span), move || self.stmt(s))).collect()
    }

    fn stmt(&self, s: &Stmt) -> String {
        format!("{}{}", self.meta(&s.meta), self.stmt_kind(&s.kind))
    }

    fn stmt_kind(&self, k: &StmtKind) -> String {
        match k {
            StmtKind::Let { pat: p, ty: t, init, else_ } => {
                let mut out = format!("let {}", self.pat1(p));
                if let Some(t) = t {
                    out.push_str(&format!(": {}", self.ty(t)));
                }
                if let Some(i) = init {
                    out.push_str(&format!(" = {}", self.expr(i)));
                }
                if let Some(b) = else_ {
                    out.push_str(&format!(" else {}", self.block(b)));
                }
                out.push(';');
                out
            }
            StmtKind::Expr { expr: e, semi } => {
                format!("{}{}", self.expr(e), if *semi { ";" } else { "" })
            }
            StmtKind::Item(i) => self.item(i),
            StmtKind::Contribution { lhs, rhs } => format!("{} +> {};", self.expr(lhs), self.expr(rhs)),
            StmtKind::Commit { place, value, after } => {
                let mut out = format!("{} <- {}", self.expr(place), self.expr(value));
                if let Some(a) = after {
                    out.push_str(&format!(" after {}", self.expr(a)));
                }
                out.push(';');
                out
            }
            StmtKind::Empty => ";".to_string(),
            StmtKind::Signal { name, ty: t, init } => self.signal(name, t, init),
            StmtKind::Reaction(r) => self.reaction(r),
        }
    }

    fn signal(&self, name: &Ident, t: &Type, init: &Option<Expr>) -> String {
        let init = init.as_ref().map_or(String::new(), |e| format!(" = {}", self.expr(e)));
        format!("signal {}: {}{init};", name.spelled(), self.ty(t))
    }

    fn reaction(&self, r: &Reaction) -> String {
        format!("on {} {}", self.expr(&r.event), self.block(&r.body))
    }

    // ---- items --------------------------------------------------------------------------------

    fn item(&self, i: &Item) -> String {
        format!("{}{}{}", self.meta(&i.meta), vis(i.public), self.item_kind(&i.kind, Self::last_brace(i.span)))
    }

    /// An item's own text; `brace` is where the `}` closing its body stands.
    fn item_kind(&self, k: &ItemKind, brace: Option<usize>) -> String {
        match k {
            ItemKind::Fn(f) => self.fn_decl(f),
            ItemKind::Struct(s) => {
                format!("struct {}{}{}", s.name.spelled(), self.generics(&s.generics), self.fields(&s.fields, true, brace))
            }
            ItemKind::Enum(e) => {
                let kids = e.variants.iter().map(|v| Kid::new(Self::extent(&v.meta, v.span), move || self.variant(v))).collect();
                let lines = self.lines(kids, ",", brace);
                format!("enum {}{} {}", e.name.spelled(), self.generics(&e.generics), braces(&lines))
            }
            ItemKind::Impl(i) => {
                let head = match &i.trait_ {
                    Some(t) => format!("{} for {}", self.path(t), self.ty(&i.self_ty)),
                    None => self.ty(&i.self_ty),
                };
                format!("impl{} {head} {}", self.generics(&i.generics), braces(&self.impl_items(&i.items, brace)))
            }
            ItemKind::Trait(t) => {
                let supers = if t.supertraits.is_empty() {
                    String::new()
                } else {
                    format!(": {}", t.supertraits.iter().map(|p| self.path(p)).collect::<Vec<_>>().join(" + "))
                };
                format!(
                    "trait {}{}{supers} {}",
                    t.name.spelled(),
                    self.generics(&t.generics),
                    braces(&self.impl_items(&t.items, brace))
                )
            }
            ItemKind::Const(c) => self.const_item(c),
            ItemKind::Use(u) => format!("use {};", self.use_tree(u)),
            ItemKind::Mod(m) => match &m.items {
                None => format!("mod {};", m.name.spelled()),
                Some(items) => {
                    let kids = items.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.item(i))).collect();
                    let (out, spans) = self.children(kids, "", Shape::Items);
                    let blocks: Vec<String> = out.into_iter().map(|(_, t)| t).collect();
                    let mut body = blocks.join("\n\n");
                    for line in self.hooks.inside(Self::close(brace, &spans), &spans, Shape::Items) {
                        body.push_str(&format!("\n{line}"));
                    }
                    if body.is_empty() {
                        format!("mod {} {{}}", m.name.spelled())
                    } else {
                        format!("mod {} {{\n{}\n}}", m.name.spelled(), indent(body.trim_start_matches('\n')))
                    }
                }
            },
            ItemKind::Nature(n) => self.nature(n, brace),
            ItemKind::Module(m) => self.module(m, brace),
            ItemKind::Behavior(b) => self.behavior(b, brace),
            ItemKind::Error => "/* error */".to_string(),
        }
    }

    fn fn_decl(&self, f: &FnDecl) -> String {
        let mut s = String::new();
        if f.is_const {
            s.push_str("const ");
        }
        if let Some(abi) = &f.abi {
            s.push_str(&format!("extern {} ", self.lit(abi)));
        }
        s.push_str(&format!("fn {}{}", f.name.spelled(), self.generics(&f.generics)));
        let params: Vec<Kid<'_>> = f
            .params
            .iter()
            .filter_map(|p| match p {
                FnParam::SelfValue(at) => Some(Kid::new(at.0, || "self".to_string())),
                FnParam::Named { name, ty: t, span } => {
                    Some(Kid::new(span.0, move || format!("{}: {}", name.spelled(), self.ty(t))))
                }
                FnParam::Error(_) => None,
            })
            .collect();
        s.push_str(&self.list("(", params, ")"));
        if let Some(r) = &f.ret {
            s.push_str(&format!(" -> {}", self.ty(r)));
        }
        if let Some(w) = &f.where_ {
            s.push_str(&format!(" where {}", self.expr(w)));
        }
        match &f.body {
            Some(b) => s.push_str(&format!(" {}", self.block(b))),
            None => s.push(';'),
        }
        s
    }

    fn const_item(&self, c: &ConstItem) -> String {
        let value = c.value.as_ref().map_or(String::new(), |v| format!(" = {}", self.expr(v)));
        format!("const {}: {}{value};", c.name.spelled(), self.ty(&c.ty))
    }

    fn impl_items(&self, items: &[ImplItem], brace: Option<usize>) -> Vec<String> {
        let kids = items
            .iter()
            .map(|i| {
                Kid::new(Self::extent(&i.meta, i.span), move || {
                    let body = match &i.kind {
                        ImplItemKind::Fn(f) => self.fn_decl(f),
                        ImplItemKind::Const(c) => self.const_item(c),
                    };
                    format!("{}{}{body}", self.meta(&i.meta), vis(i.public))
                })
            })
            .collect();
        self.lines(kids, "", brace)
    }

    fn field(&self, f: &Field) -> String {
        let body = match &f.name {
            Some(n) => format!("{}: {}", n.spelled(), self.ty(&f.ty)),
            None => self.ty(&f.ty),
        };
        format!("{}{}{}{body}", self.meta(&f.meta), vis(f.public), dir(&f.dir))
    }

    /// The body of a struct (`unit_semi`: a unit struct or tuple struct ends in
    /// `;`) or of an enum variant; `brace` is where its `}` stands, when known.
    fn fields(&self, f: &Fields, unit_semi: bool, brace: Option<usize>) -> String {
        match f {
            Fields::Named(fs) => {
                let kids = fs.iter().map(|f| Kid::new(Self::extent(&f.meta, f.span), move || self.field(f))).collect();
                format!(" {}", braces(&self.lines(kids, ",", brace)))
            }
            Fields::Tuple(fs) => {
                let kids = fs.iter().map(|f| Kid::new(Self::extent(&f.meta, f.span), move || self.field(f))).collect();
                format!("{}{}", self.list("(", kids, ")"), if unit_semi { ";" } else { "" })
            }
            Fields::Unit => if unit_semi { ";".to_string() } else { String::new() },
        }
    }

    /// An enum variant, without the `,` the enum adds.
    fn variant(&self, v: &Variant) -> String {
        let disc = v.discriminant.as_ref().map_or(String::new(), |d| format!(" = {}", self.expr(d)));
        format!("{}{}{}{disc}", self.meta(&v.meta), v.name.spelled(), self.fields(&v.fields, false, None))
    }

    fn use_tree(&self, u: &UseTree) -> String {
        let prefix: Vec<String> = u.path.iter().map(|i| i.spelled()).collect();
        let joined = prefix.join("::");
        match &u.kind {
            UseKind::Simple { alias } => match alias {
                Some(a) => format!("{joined} as {}", a.spelled()),
                None => joined,
            },
            UseKind::Glob => if joined.is_empty() { "*".to_string() } else { format!("{joined}::*") },
            UseKind::Group(trees) => {
                let inner = trees.iter().map(|t| self.use_tree(t)).collect::<Vec<_>>().join(", ");
                if joined.is_empty() { format!("{{{inner}}}") } else { format!("{joined}::{{{inner}}}") }
            }
        }
    }

    fn nature(&self, n: &NatureDecl, brace: Option<usize>) -> String {
        let kids = n
            .fields
            .iter()
            .map(|f| Kid::new(Self::extent(&f.meta, f.span), move || format!("{}{}", self.meta(&f.meta), self.nature_field(&f.kind))))
            .collect();
        let lines = self.lines(kids, ",", brace);
        format!("nature {}{} {}", n.name.spelled(), self.generics(&n.generics), braces(&lines))
    }

    /// A nature field, without the `,` the nature adds.
    fn nature_field(&self, k: &NatureFieldKind) -> String {
        match k {
            NatureFieldKind::Quantity { which, name, attrs } => {
                let w = match which {
                    Quantity::Potential => "potential",
                    Quantity::Flow => "flow",
                };
                let block = if attrs.is_empty() { String::new() } else { format!(" {}", self.field_inits(attrs)) };
                format!("{w}: {}{block}", name.spelled())
            }
            NatureFieldKind::Value(t) => format!("value: {}", self.ty(t)),
            NatureFieldKind::Resolve(e) => format!("resolve: {}", self.expr(e)),
        }
    }

    fn module(&self, m: &ModuleDecl, brace: Option<usize>) -> String {
        let mut s = String::new();
        if let Some(abi) = &m.abi {
            s.push_str(&format!("extern {} ", self.lit(abi)));
        }
        s.push_str(&format!("module {}{}", m.name.spelled(), self.generics(&m.generics)));
        let ports = m
            .ports
            .iter()
            .map(|p| {
                Kid::new(Self::extent(&p.meta, p.span), move || {
                    let default = p.default.as_ref().map_or(String::new(), |d| format!(" = {}", self.expr(d)));
                    format!("{}{}{}: {}{default}", self.meta(&p.meta), dir(&p.dir), p.name.spelled(), self.ty(&p.ty))
                })
            })
            .collect();
        s.push_str(&self.list("(", ports, ")"));
        if let Some(r) = &m.ret {
            s.push_str(&format!(" -> {}", self.ty(r)));
        }
        if let Some(w) = &m.where_ {
            s.push_str(&format!(" where {}", self.expr(w)));
        }
        let mut kids: Vec<Kid<'_>> =
            m.body.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.module_item(i))).collect();
        if let Some(r) = &m.result {
            kids.push(Kid::new(r.span.0, move || self.expr(r)));
        }
        s.push_str(&format!(" {}", braces(&self.lines(kids, "", brace))));
        s
    }

    fn module_item(&self, i: &ModuleItem) -> String {
        let body = match &i.kind {
            ModuleItemKind::Param { name, ty: t, default, where_ } => {
                let mut s = format!("{}: {}", name.spelled(), self.ty(t));
                if let Some(d) = default {
                    s.push_str(&format!(" = {}", self.expr(d)));
                }
                if let Some(w) = where_ {
                    s.push_str(&format!(" where {}", self.expr(w)));
                }
                s.push(';');
                s
            }
            ModuleItemKind::Signal { name, ty: t, init } => self.signal(name, t, init),
            ModuleItemKind::Let { pat: p, value } => format!("let {} = {};", self.pat1(p), self.expr(value)),
            ModuleItemKind::Def { place, value, after } => {
                let after = after.as_ref().map_or(String::new(), |a| format!(" after {}", self.expr(a)));
                format!("{} = {}{after};", self.expr(place), self.expr(value))
            }
            ModuleItemKind::Gen(g) => {
                self.generation(g, &|i: &ModuleItem| Self::extent(&i.meta, i.span), &|i| self.module_item(i))
            }
            ModuleItemKind::Error => "/* error */".to_string(),
        };
        format!("{}{}{body}", self.meta(&i.meta), vis(i.public))
    }

    /// `for`/`if` generation over items with the extent `ext`, printed by `leaf`.
    fn generation<X>(&self, g: &Gen<X>, ext: &dyn Fn(&X) -> Span, leaf: &dyn Fn(&X) -> String) -> String {
        let body = |items: &[X], brace: Option<usize>| {
            let kids = items.iter().map(|x| Kid::new(ext(x), move || leaf(x))).collect();
            braces(&self.lines(kids, "", brace))
        };
        match &g.kind {
            GenKind::For { pat: p, iter, body: b } => {
                format!("for {} in {} {}", self.pat1(p), self.expr(iter), body(b, Self::last_brace(g.span)))
            }
            GenKind::If { cond, then, else_ } => {
                let then_brace = if else_.is_none() { Self::last_brace(g.span) } else { None };
                let mut s = format!("if {} {}", self.expr(cond), body(then, then_brace));
                match else_.as_deref() {
                    Some(GenElse::Block(b)) => s.push_str(&format!(" else {}", body(b, Self::last_brace(g.span)))),
                    Some(GenElse::If(inner)) => s.push_str(&format!(" else {}", self.generation(inner, ext, leaf))),
                    None => {}
                }
                s
            }
        }
    }

    fn behavior(&self, b: &BehaviorBlock, brace: Option<usize>) -> String {
        let (word, kids) = match &b.body {
            BehaviorBody::Analog(stmts) => ("analog", self.stmts(stmts)),
            BehaviorBody::Digital(items) => (
                "digital",
                items.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.digital_item(i))).collect(),
            ),
        };
        let lines = self.lines(kids, "", brace);
        let names = if b.names.is_empty() {
            String::new()
        } else {
            format!("<{}>", b.names.iter().map(|i| self.ident(i)).collect::<Vec<_>>().join(", "))
        };
        format!("{word}{names} for {} {}", self.path(&b.target), braces(&lines))
    }

    fn digital_item(&self, i: &DigitalItem) -> String {
        let body = match &i.kind {
            DigitalItemKind::Reaction(r) => self.reaction(r),
            DigitalItemKind::Gen(g) => {
                self.generation(g, &|i: &DigitalItem| Self::extent(&i.meta, i.span), &|i| self.digital_item(i))
            }
            DigitalItemKind::Error => "/* error */".to_string(),
        };
        format!("{}{body}", self.meta(&i.meta))
    }
}

fn binop(op: BinOp) -> &'static str {
    use BinOp::*;
    match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Rem => "%", BitXor => "^", BitAnd => "&",
        BitOr => "|", Shl => "<<", Shr => ">>", And => "&&", Or => "||", Eq => "==", Ne => "!=",
        Lt => "<", Le => "<=", Gt => ">", Ge => ">=",
    }
}

fn vis(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}

fn dir(d: &Option<Dir>) -> &'static str {
    match d {
        Some(Dir::In) => "in ",
        Some(Dir::Out) => "out ",
        None => "",
    }
}
