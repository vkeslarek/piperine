//! The printer of patterns, literals, expressions, blocks and statements.

use super::{Kid, Printer, WIDTH};
use crate::msl::ast::*;
use crate::msl::span::Span;

impl Printer<'_> {
    /// A pattern where alternatives are not allowed at the top: parenthesized.
    pub(super) fn pat1(&self, p: &Pat) -> String {
        match p.kind {
            PatKind::Or(_) => format!("({})", self.pat(p)),
            _ => self.pat(p),
        }
    }

    pub(super) fn pat(&self, p: &Pat) -> String {
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
    pub(super) fn pat1_alt(&self, p: &Pat) -> String {
        self.pat1(p)
    }

    // ---- expressions --------------------------------------------------------------------------

    pub(super) fn lit(&self, l: &Lit) -> String {
        match &l.kind {
            LitKind::Int(s) => s.clone(),
            LitKind::Float { raw, .. } => raw.clone(),
            LitKind::Str { text, raw_hashes: Some(n) } => {
                let hashes = "#".repeat(usize::from(*n));
                let body = crate::msl::lexer::Lexer::unescape(text).0;
                format!("r{hashes}\"{body}\"{hashes}")
            }
            LitKind::Str { text, raw_hashes: None } => format!("\"{}\"", Self::escape(text)),
            LitKind::Bool(b) => b.to_string(),
        }
    }

    pub(super) fn args<'a>(&'a self, a: &'a [Arg]) -> Vec<Kid<'a>> {
        a.iter()
            .map(|a| {
                Kid::new(a.span.0, move || match &a.name {
                    Some(n) => format!("{}: {}", n.spelled(), self.expr(&a.value)),
                    None => self.expr(&a.value),
                })
            })
            .collect()
    }

    pub(super) fn exprs<'a>(&'a self, items: &'a [Expr]) -> Vec<Kid<'a>> {
        items.iter().map(|e| Kid::new(e.span.0, move || self.expr(e))).collect()
    }

    pub(super) fn field_inits(&self, f: &[FieldInit]) -> String {
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

    pub(super) fn expr(&self, e: &Expr) -> String {
        crate::stack::Stack::deep(|| self.expr_inner(e))
    }

    pub(super) fn expr_inner(&self, e: &Expr) -> String {
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
            ExprKind::Binary { op, lhs, rhs } => format!("{} {} {}", self.expr(lhs), Self::binary_operator(*op), self.expr(rhs)),
            ExprKind::Assign { op, lhs, rhs } => {
                let sym = op.map_or("=".to_string(), |o| format!("{}=", Self::binary_operator(o)));
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
                format!("match {} {}", self.expr(scrutinee), Self::braces(&lines))
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
    pub(super) fn arm(&self, a: &Arm) -> String {
        let mut s = self.attrs_only(&a.meta);
        s.push_str(&self.pat(&a.pat));
        if let Some(g) = &a.guard {
            s.push_str(&format!(" if {}", self.expr(g)));
        }
        s.push_str(&format!(" => {}", self.expr(&a.body)));
        s
    }

    /// Documentation and attributes, each on its own line, in front of a node.
    pub(super) fn attrs_only(&self, m: &Meta) -> String {
        self.meta(m)
    }

    // ---- blocks and statements ----------------------------------------------------------------

    pub(super) fn block(&self, b: &Block) -> String {
        Self::braces(&self.lines(self.stmts(&b.stmts), "", Self::last_brace(b.span)))
    }

    pub(super) fn stmts<'a>(&'a self, stmts: &'a [Stmt]) -> Vec<Kid<'a>> {
        stmts.iter().map(|s| Kid::new(Self::extent(&s.meta, s.span), move || self.stmt(s))).collect()
    }

    pub(super) fn stmt(&self, s: &Stmt) -> String {
        format!("{}{}", self.meta(&s.meta), self.stmt_kind(&s.kind))
    }

    pub(super) fn stmt_kind(&self, k: &StmtKind) -> String {
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
}
