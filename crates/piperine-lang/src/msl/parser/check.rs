//! Checks that need the whole tree: forms the grammar parses everywhere but
//! a context forbids. The parser accepts `signal`, `on`, `+>` and `<-` as
//! statements in any block; this pass names them where they do not belong.
//!
//! A statement of the wrong kind in a function body is `E0121` (Part I §3
//! rule 6); in any other block that is not its own (`+>` outside `analog`,
//! `<-` and `on` outside a reaction, `bound_step` outside `analog`, any of
//! them in a closure that is not inside a function) it is `E0404` (Part IV
//! §10 rule 8). A `module` inside a block is `E0303`.

use super::*;

/// The kind of block a statement sits in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ctx {
    /// A module body: the statements a module body does not hold.
    Module,
    /// The body of a `fn` (closures inside it included).
    Fn,
    Analog,
    Reaction,
    /// A closure outside a function.
    Closure,
    /// Anywhere else: a module body, a `const` initializer.
    Other,
}

impl Ctx {
    fn name(self) -> &'static str {
        match self {
            Ctx::Module => "a module body",
            Ctx::Fn => "a function body",
            Ctx::Analog => "an `analog` block",
            Ctx::Reaction => "a reaction",
            Ctx::Closure => "a closure",
            Ctx::Other => "this block",
        }
    }

    /// The diagnostic of an `on` written in this context: a statement of the wrong kind, except an
    /// `on` whose event expression holds a call of an event module (`on every(..) { .. }`,
    /// `on every(..) && en { .. }`, `on (every(..)) { .. }`) in a reaction, which is that module
    /// placed in a reaction body (`E0350`, §8.3.4).
    fn nested_reaction(self, reaction: &Reaction, span: Span) -> Diagnostic {
        let placed = match self {
            Ctx::Reaction => Self::called_module(&reaction.event),
            _ => None,
        };
        match placed {
            Some(name) => Diagnostic::new("E0350", format!("`{name}` is placed in a reaction body: it places no instance (§8.3.4)"), span, "placement in a reaction body")
                .with_help("a reaction is a top-level item of a `digital` block; a reaction body holds commits and `emit!`"),
            None => misplaced(self, "`on`", span, "a reaction belongs in a `digital` block", "a reaction is a top-level item of a `digital` block"),
        }
    }
}

impl Ctx {
    /// The name of the first one-segment call in an event expression, looking through parentheses,
    /// unary and binary operators: the call of an event module the event places.
    fn called_module(event: &Expr) -> Option<&str> {
        match &event.kind {
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Path(path) if path.segments.len() == 1 => Some(path.segments[0].ident.name.as_str()),
                _ => None,
            },
            ExprKind::Paren(inner) | ExprKind::Unary { expr: inner, .. } => Self::called_module(inner),
            ExprKind::Binary { lhs, rhs, .. } => Self::called_module(lhs).or_else(|| Self::called_module(rhs)),
            _ => None,
        }
    }
}

/// Context diagnostics of a parsed file, in source order.
pub(crate) fn file(file: &File) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    items(&file.items, &mut out);
    out.sort_by_key(|d| d.span().start);
    out
}

fn items(list: &[Item], out: &mut Vec<Diagnostic>) {
    let mut seen = std::collections::HashSet::new();
    for item in list {
        item_kind(&item.kind, out);
        if let ItemKind::Fn(f) = &item.kind {
            overload(f, &mut seen, out);
        }
    }
}

/// E0143: a second function of the same name in one scope is an overload.
fn overload(f: &FnDecl, seen: &mut std::collections::HashSet<String>, out: &mut Vec<Diagnostic>) {
    if !seen.insert(f.name.name.clone()) {
        out.push(
            Diagnostic::new(
                "E0143",
                format!("an overload of `{}` in a function signature", f.name.name),
                f.name.span.0,
                "this name is already a function in this scope",
            )
            .with_help("functions are not overloaded: give each its own name"),
        );
    }
}

/// E0142: no positional argument follows a named one (Part I §3.2.4).
fn argument_order(args: &[Arg], out: &mut Vec<Diagnostic>) {
    let mut named = None;
    for a in args {
        match (&a.name, &named) {
            (Some(n), None) => named = Some(n.name.clone()),
            (None, Some(n)) => out.push(
                Diagnostic::new(
                    "E0142",
                    "a positional argument after a named one",
                    a.span.0,
                    format!("follows the named argument `{n}`"),
                )
                .with_help("write the positional arguments first: `f(1, 2, name: 3)`"),
            ),
            _ => {}
        }
    }
}

fn item_kind(kind: &ItemKind, out: &mut Vec<Diagnostic>) {
    match kind {
        ItemKind::Fn(f) => fn_decl(f, out),
        ItemKind::Impl(i) => impl_items(&i.items, out),
        ItemKind::Trait(t) => impl_items(&t.items, out),
        ItemKind::Mod(m) => items(m.items.as_deref().unwrap_or(&[]), out),
        ItemKind::Module(m) => module(m, out),
        ItemKind::Behavior(b) => match &b.body {
            BehaviorBody::Analog(stmts) => stmts.iter().for_each(|s| stmt(s, Ctx::Analog, out)),
            BehaviorBody::Digital(list) => digital(list, out),
        },
        ItemKind::Const(c) => {
            if let Some(v) = &c.value {
                expr(v, Ctx::Other, out);
            }
        }
        _ => {}
    }
}

fn module(m: &ModuleDecl, out: &mut Vec<Diagnostic>) {
    let mut e = |x: &Expr| expr(x, Ctx::Other, out);
    m.where_.iter().for_each(&mut e);
    m.ports.iter().filter_map(|p| p.default.as_ref()).for_each(&mut e);
    m.result.iter().for_each(&mut e);
    module_items(&m.body, out);
}

fn module_items(list: &[ModuleItem], out: &mut Vec<Diagnostic>) {
    for item in list {
        let mut e = |x: &Expr| expr(x, Ctx::Other, out);
        match &item.kind {
            ModuleItemKind::Param { default, where_, .. } => {
                default.iter().chain(where_.iter()).for_each(&mut e);
            }
            ModuleItemKind::Signal { init, .. } => init.iter().for_each(&mut e),
            ModuleItemKind::Let { value, .. } => e(value),
            ModuleItemKind::Def { place, value, after } => {
                e(place);
                e(value);
                after.iter().for_each(&mut e);
            }
            ModuleItemKind::Gen(g) => {
                let (exprs, lists) = generation(g);
                exprs.into_iter().for_each(|x| expr(x, Ctx::Other, out));
                lists.into_iter().for_each(|l| module_items(l, out));
            }
            ModuleItemKind::Error => {}
        }
    }
}

/// The expressions (conditions, iterator) and the item lists of a generation,
/// its `else` chain included.
fn generation<X>(g: &Gen<X>) -> (Vec<&Expr>, Vec<&[X]>) {
    let mut exprs = Vec::new();
    let mut lists: Vec<&[X]> = Vec::new();
    let mut next = Some(g);
    while let Some(g) = next {
        next = None;
        match &g.kind {
            GenKind::For { iter, body, .. } => {
                exprs.push(iter);
                lists.push(body);
            }
            GenKind::If { cond, then, else_ } => {
                exprs.push(cond);
                lists.push(then);
                match else_.as_deref() {
                    Some(GenElse::Block(b)) => lists.push(b),
                    Some(GenElse::If(inner)) => next = Some(inner),
                    None => {}
                }
            }
        }
    }
    (exprs, lists)
}

fn digital(list: &[DigitalItem], out: &mut Vec<Diagnostic>) {
    for item in list {
        match &item.kind {
            DigitalItemKind::Reaction(r) => {
                expr(&r.event, Ctx::Reaction, out);
                block(&r.body, Ctx::Reaction, out);
            }
            DigitalItemKind::Gen(g) => {
                let (exprs, lists) = generation(g);
                exprs.into_iter().for_each(|x| expr(x, Ctx::Reaction, out));
                lists.into_iter().for_each(|l| digital(l, out));
            }
            DigitalItemKind::Error => {}
        }
    }
}

fn impl_items(list: &[ImplItem], out: &mut Vec<Diagnostic>) {
    let mut seen = std::collections::HashSet::new();
    for item in list {
        if let ImplItemKind::Fn(f) = &item.kind {
            fn_decl(f, out);
            overload(f, &mut seen, out);
        }
    }
}

fn fn_decl(f: &FnDecl, out: &mut Vec<Diagnostic>) {
    if let Some(body) = &f.body {
        block(body, Ctx::Fn, out);
    }
}

/// The diagnostic of a statement of the wrong kind: `E0121` in a function
/// body, `E0404` in any other block.
pub(crate) fn misplaced(ctx: Ctx, what: &str, span: Span, fn_help: &str, help: &str) -> Diagnostic {
    if ctx == Ctx::Fn {
        Diagnostic::new(
            "E0121",
            format!("{what} in a function body"),
            span,
            "a function has no time and no carriers",
        )
        .with_help(fn_help)
    } else {
        Diagnostic::new("E0404", format!("{what} in {}", ctx.name()), span, "a statement of the wrong kind here")
            .with_help(help)
    }
}

fn block(b: &Block, ctx: Ctx, out: &mut Vec<Diagnostic>) {
    for s in &b.stmts {
        stmt(s, ctx, out);
    }
}

fn stmt(s: &Stmt, ctx: Ctx, out: &mut Vec<Diagnostic>) {
    let span = s.span.0;
    match &s.kind {
        StmtKind::Let { init, else_, .. } => {
            if let Some(e) = init {
                expr(e, ctx, out);
            }
            if let Some(b) = else_ {
                block(b, ctx, out);
            }
        }
        StmtKind::Expr { expr: e, .. } => expr(e, ctx, out),
        StmtKind::Item(item) => {
            if matches!(item.kind, ItemKind::Module(_)) {
                out.push(
                    Diagnostic::new(
                        "E0303",
                        "a module declaration inside a block",
                        item.span.0,
                        "modules are declared at file scope",
                    )
                    .with_help("move the `module` out of the block, or into a `mod`"),
                );
            }
            // A nested function is a function body of its own.
            item_kind(&item.kind, out);
        }
        StmtKind::Contribution { lhs, rhs } => {
            if ctx != Ctx::Analog {
                out.push(misplaced(
                    ctx,
                    "a contribution (`+>`)",
                    span,
                    "put the contribution in an `analog` block of a `module`",
                    "a contribution belongs in an `analog` block; a reaction commits with `<-`",
                ));
            }
            expr(lhs, ctx, out);
            expr(rhs, ctx, out);
        }
        StmtKind::Commit { place, value, after } => {
            if !is_place(place) {
                let c = not_a_place();
                out.push(Diagnostic::new(c.code, c.message, place.span.0, "not a place").with_help(c.help.unwrap_or_default()));
            }
            if ctx != Ctx::Reaction {
                out.push(misplaced(
                    ctx,
                    "a commit (`<-`)",
                    span,
                    "put the commit in a reaction (`on event { .. }`) of a `digital` block",
                    "a commit belongs in a reaction (`on event { .. }`) of a `digital` block; an `analog` block contributes with `+>`",
                ));
            }
            expr(place, ctx, out);
            expr(value, ctx, out);
            if let Some(a) = after {
                expr(a, ctx, out);
            }
        }
        StmtKind::Signal { init, .. } => {
            out.push(misplaced(
                ctx,
                "`signal`",
                span,
                "declare the carrier in the body of a `module`",
                "declare the carrier in the body of a `module`: `signal name: Type;`",
            ));
            if let Some(e) = init {
                expr(e, ctx, out);
            }
        }
        StmtKind::Reaction(r) => {
            out.push(ctx.nested_reaction(r, span));
            expr(&r.event, ctx, out);
            block(&r.body, ctx, out);
        }
        StmtKind::Empty => {}
    }
}

fn expr(e: &Expr, ctx: Ctx, out: &mut Vec<Diagnostic>) {
    crate::stack::deep(|| expr_inner(e, ctx, out))
}

fn expr_inner(e: &Expr, ctx: Ctx, out: &mut Vec<Diagnostic>) {
    let mut sub = |x: &Expr| expr(x, ctx, out);
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
            if let ExprKind::Path(p) = &callee.kind {
                if ctx != Ctx::Analog && p.segments.len() == 1 && p.segments[0].ident.name == "bound_step" {
                    out.push(misplaced(
                        ctx,
                        "`bound_step`",
                        e.span.0,
                        "call `bound_step` from an `analog` block",
                        "`bound_step` bounds the analog time step: call it from an `analog` block",
                    ));
                }
            }
            argument_order(args, out);
            let mut sub = |x: &Expr| expr(x, ctx, out);
            sub(callee);
            args.iter().for_each(|a| sub(&a.value));
            params.iter().flatten().for_each(|f| sub(&f.value));
        }
        ExprKind::MethodCall { recv, args, .. } => {
            argument_order(args, out);
            let mut sub = |x: &Expr| expr(x, ctx, out);
            sub(recv);
            args.iter().for_each(|a| sub(&a.value));
        }
        ExprKind::Block(b) | ExprKind::Loop { body: b } => block(b, ctx, out),
        ExprKind::If { cond, then, else_ } => {
            sub(cond);
            block(then, ctx, out);
            if let Some(x) = else_ {
                expr(x, ctx, out);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            sub(scrutinee);
            for a in arms {
                if let Some(g) = &a.guard {
                    expr(g, ctx, out);
                }
                expr(&a.body, ctx, out);
            }
        }
        ExprKind::While { cond, body } => {
            sub(cond);
            block(body, ctx, out);
        }
        ExprKind::For { iter, body, .. } => {
            sub(iter);
            block(body, ctx, out);
        }
        // A closure is a function: inside a function body it stays part of
        // it; elsewhere it is a block of its own kind.
        ExprKind::Closure { body, .. } => {
            let inner = if ctx == Ctx::Fn { Ctx::Fn } else { Ctx::Closure };
            expr(body, inner, out);
        }
    }
}
