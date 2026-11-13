//! Checks that need the whole tree: forms the grammar parses everywhere but
//! a context forbids. The parser accepts `signal`, `on`, `+>` and `<-` as
//! statements in any block; this pass names them where they do not belong.

use super::*;

/// Context diagnostics of a parsed file, in source order.
pub(crate) fn file(file: &File) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    items(&file.items, &mut out);
    out.sort_by_key(|d| d.span().start);
    out
}

fn items(list: &[Item], out: &mut Vec<Diagnostic>) {
    for item in list {
        item_kind(&item.kind, out);
    }
}

fn item_kind(kind: &ItemKind, out: &mut Vec<Diagnostic>) {
    match kind {
        ItemKind::Fn(f) => fn_decl(f, out),
        ItemKind::Impl(i) => impl_items(&i.items, out),
        ItemKind::Trait(t) => impl_items(&t.items, out),
        ItemKind::Mod(m) => items(m.items.as_deref().unwrap_or(&[]), out),
        _ => {}
    }
}

fn impl_items(list: &[ImplItem], out: &mut Vec<Diagnostic>) {
    for item in list {
        if let ImplItemKind::Fn(f) = &item.kind {
            fn_decl(f, out);
        }
    }
}

fn fn_decl(f: &FnDecl, out: &mut Vec<Diagnostic>) {
    if let Some(body) = &f.body {
        block(body, true, out);
    }
}

/// E0121: a function body holds no `signal`, no `<-`, no `+>`, no
/// `bound_step` and no `on` (Part I §3 rule 6).
fn forbidden(what: &str, span: Span, help: &str) -> Diagnostic {
    Diagnostic::new(
        "E0121",
        format!("{what} in a function body"),
        span,
        "a function has no time and no carriers",
    )
    .with_help(help)
}

fn block(b: &Block, in_fn: bool, out: &mut Vec<Diagnostic>) {
    for s in &b.stmts {
        stmt(s, in_fn, out);
    }
}

fn stmt(s: &Stmt, in_fn: bool, out: &mut Vec<Diagnostic>) {
    let span = s.span.0;
    match &s.kind {
        StmtKind::Let { init, .. } => {
            if let Some(e) = init {
                expr(e, in_fn, out);
            }
        }
        StmtKind::Expr { expr: e, .. } => expr(e, in_fn, out),
        StmtKind::Item(item) => {
            // A nested function is a function body of its own.
            item_kind(&item.kind, out);
        }
        StmtKind::Contribution { lhs, rhs } => {
            if in_fn {
                out.push(forbidden(
                    "a contribution (`+>`)",
                    span,
                    "put the contribution in an `analog` block of a `module`",
                ));
            }
            expr(lhs, in_fn, out);
            expr(rhs, in_fn, out);
        }
        StmtKind::Commit { place, value, after } => {
            if in_fn {
                out.push(forbidden(
                    "a commit (`<-`)",
                    span,
                    "put the commit in a reaction (`on event { .. }`) of a `digital` block",
                ));
            }
            expr(place, in_fn, out);
            expr(value, in_fn, out);
            if let Some(a) = after {
                expr(a, in_fn, out);
            }
        }
        StmtKind::Signal { init, .. } => {
            if in_fn {
                out.push(forbidden("`signal`", span, "declare the carrier in the body of a `module`"));
            }
            if let Some(e) = init {
                expr(e, in_fn, out);
            }
        }
        StmtKind::Reaction(r) => {
            if in_fn {
                out.push(forbidden("`on`", span, "a reaction belongs in a `digital` block"));
            }
            expr(&r.event, in_fn, out);
            block(&r.body, in_fn, out);
        }
        StmtKind::Empty => {}
    }
}

fn expr(e: &Expr, in_fn: bool, out: &mut Vec<Diagnostic>) {
    let mut sub = |x: &Expr| expr(x, in_fn, out);
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Continue | ExprKind::Error | ExprKind::IncludeStr(_) => {}
        ExprKind::Unary { expr: x, .. }
        | ExprKind::Cast { expr: x, .. }
        | ExprKind::Field { expr: x, .. }
        | ExprKind::Paren(x)
        | ExprKind::Try(x)
        | ExprKind::Let { expr: x, .. } => sub(x),
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Assign { lhs, rhs, .. } => {
            sub(lhs);
            sub(rhs);
        }
        ExprKind::Index { expr: a, index: b } => {
            sub(a);
            sub(b);
        }
        ExprKind::ArrayRepeat { elem, len } => {
            sub(elem);
            sub(len);
        }
        ExprKind::Range { lo, hi, .. } => {
            lo.iter().chain(hi.iter()).for_each(|x| sub(x));
        }
        ExprKind::Tuple(v) | ExprKind::Array(v) => v.iter().for_each(sub),
        ExprKind::Struct { fields, .. } | ExprKind::Emit { fields, .. } => {
            fields.iter().for_each(|f| sub(&f.value));
        }
        ExprKind::Break(v) | ExprKind::Return(v) => v.iter().for_each(|x| sub(x)),
        ExprKind::Call { callee, args, params } => {
            if in_fn {
                if let ExprKind::Path(p) = &callee.kind {
                    if p.segments.len() == 1 && p.segments[0].ident.name == "bound_step" {
                        out.push(forbidden(
                            "`bound_step`",
                            e.span.0,
                            "call `bound_step` from an `analog` block",
                        ));
                    }
                }
            }
            let mut sub = |x: &Expr| expr(x, in_fn, out);
            sub(callee);
            args.iter().for_each(|a| sub(&a.value));
            params.iter().flatten().for_each(|f| sub(&f.value));
        }
        ExprKind::MethodCall { recv, args, .. } => {
            sub(recv);
            args.iter().for_each(|a| sub(&a.value));
        }
        ExprKind::Block(b) | ExprKind::Loop { body: b } => block(b, in_fn, out),
        ExprKind::If { cond, then, else_ } => {
            sub(cond);
            block(then, in_fn, out);
            if let Some(x) = else_ {
                expr(x, in_fn, out);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            sub(scrutinee);
            for a in arms {
                if let Some(g) = &a.guard {
                    expr(g, in_fn, out);
                }
                expr(&a.body, in_fn, out);
            }
        }
        ExprKind::While { cond, body } => {
            sub(cond);
            block(body, in_fn, out);
        }
        ExprKind::For { iter, body, .. } => {
            sub(iter);
            block(body, in_fn, out);
        }
        ExprKind::Closure { body, .. } => sub(body),
    }
}
