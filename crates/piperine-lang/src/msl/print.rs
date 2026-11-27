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
//! delimiter. [`Printer::print`] runs the printer with hooks that change nothing; the
//! formatter (`msl::format`) runs it with hooks that put comments back.

use super::ast::*;
use super::span::Span;

mod expressions;
mod hardware;
mod items;

pub(super) const WIDTH: usize = 80;

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
/// and returned is in the printer's escaped form (see [`Printer::escape`]).
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

/// The hooks of [`Printer::print`]: the canonical layout, nothing added.
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
pub struct Printer<'h> {
    hooks: &'h dyn Hooks,
}

impl<'h> Printer<'h> {
    pub(crate) fn new(hooks: &'h dyn Hooks) -> Printer<'h> {
        Printer { hooks }
    }

    /// `text` in the escaped form a hook hands back: its line breaks are printed as
    /// written, never indented. While a node's text is built, a `\0` marks an
    /// escape: `\0n`, `\0r` stand for a line break and a carriage return inside
    /// verbatim text (a string literal or doc comment), `\00` for a NUL;
    /// [`Printer::restore`] puts them back.
    pub(crate) fn escape(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '\0' => out.push_str("\00"),
                '\n' => out.push_str("\0n"),
                '\r' => out.push_str("\0r"),
                c => out.push(c),
            }
        }
        out
    }

    /// The text of an escaped node with its line breaks and carriage returns back.
    fn restore(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars();
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
    fn documentation_comment(doc: &Doc, kind: &str) -> String {
        let text = Self::escape(&doc.text);
        if doc.block {
            let star = if kind == "!" { "!" } else { "*" };
            format!("/*{star}{text}*/")
        } else {
            format!("//{kind}{text}")
        }
    }

    /// Indent every non-blank line of `text` by one level.
    pub(super) fn indent(text: &str) -> String {
        text.lines()
            .map(|line| if line.is_empty() { String::new() } else { format!("    {line}") })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `{ .. }` holding one entry per line, or `{}` when empty.
    pub(super) fn braces(lines: &[String]) -> String {
        if lines.is_empty() {
            "{}".to_string()
        } else {
            format!("{{\n{}\n}}", Self::indent(&lines.join("\n")))
        }
    }

    /// The spelling of a binary operator.
    pub(super) fn binary_operator(operator: BinOp) -> &'static str {
        use BinOp::*;
        match operator {
            Add => "+", Sub => "-", Mul => "*", Div => "/", Rem => "%", BitXor => "^", BitAnd => "&",
            BitOr => "|", Shl => "<<", Shr => ">>", And => "&&", Or => "||", Eq => "==", Ne => "!=",
            Lt => "<", Le => "<=", Gt => ">", Ge => ">=",
        }
    }

    /// `pub ` for a public item, nothing otherwise.
    pub(super) fn visibility(public: bool) -> &'static str {
        if public { "pub " } else { "" }
    }

    /// The direction keyword of a port, with its trailing space; nothing when it has none.
    pub(super) fn direction(direction: &Option<Dir>) -> &'static str {
        match direction {
            Some(Dir::In) => "in ",
            Some(Dir::Out) => "out ",
            None => "",
        }
    }

    /// Print a file in the canonical layout.
    pub fn print(file: &File) -> String {
        Printer::new(&Canonical).file(file)
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
            let text = self.hooks.around(d.span.0, Self::documentation_comment(d, "!"), prev, Shape::Lines);
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
        Self::restore(&out)
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
            lines.push(Self::indent(&text));
            prev = Some(*span);
        }
        let inside = self.hooks.inside(Self::close(None, spans), spans, Shape::List);
        added |= !inside.is_empty();
        lines.extend(inside.iter().map(|l| Self::indent(l)));
        (lines, added)
    }

    fn ident(&self, i: &Ident) -> String {
        i.spelled()
    }

    fn meta(&self, m: &Meta) -> String {
        let mut out = String::new();
        for d in &m.docs {
            out.push_str(&format!("{}\n", Self::documentation_comment(d, "/")));
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
}
