//! Expression resolution for analog bodies: walks POM `Expr`
//! (`piperine_lang::ast::Expr`) and resolves constants, analog operators
//! (→ `__ddt(id, x)` markers), and V/I/math left as-is (the Builder
//! resolves names to ids at JIT emit time via the `Resolver`).

use piperine_lang::ast::Expr;
use piperine_lang::error::Span;
use piperine_lang::lexer::NumberLit;

use crate::resolve::*;

use super::analog_ops::analog_ops;
use super::LowerCtx;

const NOSPAN: Span = Span { start: 0, end: 0 };

fn real(v: f64) -> Expr {
    Expr::Number { span: NOSPAN, value: NumberLit::Real(v) }
}

/// A contribution's `<+` destination is always an access-function call
/// (`V(p,n)`/`I(br)`/a custom discipline access name) — LRM §5.4.1.
pub(crate) fn parse_contrib_dest(dest: &Expr, ctx: &mut LowerCtx) -> (NatureId, NodeId, NodeId) {
    let Expr::AccessFunction { function, pos, neg, .. } = dest else {
        let nature = ctx.symbols.add_nature("I", NatureKind::Flow);
        return (nature, NodeId::GROUND, NodeId::GROUND);
    };
    let nature_kind = match function.as_str() {
        "V" => NatureKind::Potential,
        _ => NatureKind::Flow,
    };
    let nature = ctx.symbols.add_nature(function.as_str(), nature_kind);

    let plus_name = ident_from_expr(Some(pos)).unwrap_or_else(|| "?".into());
    let minus_name = neg.as_deref().and_then(|n| ident_from_expr(Some(n))).unwrap_or_else(|| "0".into());

    let plus = ctx.require_node(&plus_name);
    let minus = ctx.require_node(&minus_name);

    (nature, plus, minus)
}

/// A hierarchical/scoped name (`load.p`) is already one joined `Ident`
/// string by the time it reaches this layer — the grammar does the
/// joining (`ast/expr.rs`'s own doc comment); there is no separate
/// `Field`-style AST node to unwrap, unlike PHDL.
pub(crate) fn ident_from_expr(e: Option<&Expr>) -> Option<String> {
    match e? {
        Expr::Ident { name, .. } => Some(name.clone()),
        _ => None,
    }
}

pub(crate) fn scan_noise(expr: &Expr, plus: NodeId, minus: NodeId, ctx: &mut LowerCtx) {
    walk(expr, &mut |e| {
        let Expr::Call { name, args, .. } = e else { return true };
        match name.as_str() {
            "white_noise" => {
                let psd = args.first().map(|a| resolve_expr(a, ctx)).unwrap_or_else(|| real(0.0));
                let label = args.get(1).and_then(|a| {
                    if let Expr::Str { value, .. } = a { Some(value.clone()) } else { None }
                });
                ctx.noise_sources.push(NoiseSource { plus, minus, kind: NoiseKind::White { psd }, label });
                false
            }
            "flicker_noise" => {
                let psd = args.first().map(|a| resolve_expr(a, ctx)).unwrap_or_else(|| real(0.0));
                let exponent = args.get(1).map(|a| resolve_expr(a, ctx)).unwrap_or_else(|| real(1.0));
                let label = args.get(2).and_then(|a| {
                    if let Expr::Str { value, .. } = a { Some(value.clone()) } else { None }
                });
                ctx.noise_sources.push(NoiseSource { plus, minus, kind: NoiseKind::Flicker { psd, exponent }, label });
                false
            }
            _ => true,
        }
    });
}

/// Pre-order walk over a VAMS `Expr`; `f` returns `false` to skip a node's
/// children. No shared `Walk` trait exists post-MD-45 (deleted with
/// `piperine_lang::parse::ast::Walk`) — every module in `resolve/pom/`
/// that needs a tree walk carries its own, matching `diff.rs`'s precedent
/// (T29).
fn walk(expr: &Expr, f: &mut impl FnMut(&Expr) -> bool) {
    if !f(expr) {
        return;
    }
    match expr {
        // PortAccess never reaches codegen: elab::lower::vams rewrites
        // every occurrence into a KCL-equivalent sum of AccessFunction
        // reads before a Module leaves the elaborator (LRMC-C1-10). A leaf
        // arm here only satisfies exhaustiveness for the shared Expr type.
        Expr::Number { .. } | Expr::Str { .. } | Expr::Ident { .. } | Expr::PortAccess { .. } => {}
        Expr::Unary { operand, .. } => walk(operand, f),
        Expr::Binary { lhs, rhs, .. } => {
            walk(lhs, f);
            walk(rhs, f);
        }
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            walk(cond, f);
            walk(then_branch, f);
            walk(else_branch, f);
        }
        Expr::Concat { parts, .. } | Expr::Replication { parts, .. } => {
            for p in parts {
                walk(p, f);
            }
        }
        Expr::AssignmentPattern { replicate, parts, .. } => {
            if let Some(r) = replicate {
                walk(r, f);
            }
            for p in parts {
                walk(p, f);
            }
        }
        Expr::Index { base, index, .. } => {
            walk(base, f);
            walk(index, f);
        }
        Expr::PartSelect { base, msb, lsb, .. } => {
            walk(base, f);
            walk(msb, f);
            walk(lsb, f);
        }
        Expr::Call { args, .. } | Expr::SystemFunctionCall { args, .. } => {
            for a in args {
                walk(a, f);
            }
        }
        Expr::AccessFunction { pos, neg, .. } => {
            walk(pos, f);
            if let Some(n) = neg {
                walk(n, f);
            }
        }
    }
}

/// Resolve a POM `Expr` for the analog path: constants → literals, analog
/// operators → `__ddt(id, x)` markers, user functions → inlined (once a
/// later phase lowers `analog function`/`function` decls into
/// `SymbolTable`; today `ctx.symbols.fn_by_name` never matches — MD-45,
/// see `body.rs`), V/I/math kept as-is (Builder resolves at emit time).
pub(crate) fn resolve_expr(expr: &Expr, ctx: &mut LowerCtx) -> Expr {
    match expr {
        // Never reaches codegen — see `walk`'s own PortAccess arm above.
        Expr::Number { .. } | Expr::Str { .. } | Expr::PortAccess { .. } => expr.clone(),

        Expr::Ident { name, .. } => {
            if let Some(c) = ctx.consts.get(name) {
                c.clone()
            } else if let Some(id) = ctx.lookup_node(name) {
                if ctx.is_digital {
                    expr.clone()
                } else if ctx.symbols.node(id).domain == crate::resolve::Domain::Digital {
                    let var = ctx.shadow_var_for(id, name);
                    Expr::Ident { span: NOSPAN, name: ctx.symbols.var(var).name.clone() }
                } else {
                    // Analog node in an analog body — keep as Ident so the
                    // flattener (parse_dest) and Builder can resolve it.
                    expr.clone()
                }
            } else {
                expr.clone()
            }
        }

        Expr::Unary { span, op, operand } => {
            Expr::Unary { span: *span, op: *op, operand: Box::new(resolve_expr(operand, ctx)) }
        }

        Expr::Binary { span, op, lhs, rhs } => Expr::Binary {
            span: *span,
            op: *op,
            lhs: Box::new(resolve_expr(lhs, ctx)),
            rhs: Box::new(resolve_expr(rhs, ctx)),
        },

        Expr::Ternary { span, cond, then_branch, else_branch } => Expr::Ternary {
            span: *span,
            cond: Box::new(resolve_expr(cond, ctx)),
            then_branch: Box::new(resolve_expr(then_branch, ctx)),
            else_branch: Box::new(resolve_expr(else_branch, ctx)),
        },

        Expr::Concat { span, parts } => {
            Expr::Concat { span: *span, parts: parts.iter().map(|p| resolve_expr(p, ctx)).collect() }
        }
        Expr::Replication { span, count, parts } => Expr::Replication {
            span: *span,
            count: Box::new(resolve_expr(count, ctx)),
            parts: parts.iter().map(|p| resolve_expr(p, ctx)).collect(),
        },
        // Structural pass-through only — kept as a distinct node (not
        // desugared) so a later consumer (e.g. the `laplace_*` coefficient
        // arguments, LRMC-C1-12/C5-01) can still pattern-match on it; no
        // semantic handling of assignment patterns is implemented here
        // (that's LRMC-C1-06's elaboration follow-up task).
        Expr::AssignmentPattern { span, replicate, parts } => Expr::AssignmentPattern {
            span: *span,
            replicate: replicate.as_ref().map(|r| Box::new(resolve_expr(r, ctx))),
            parts: parts.iter().map(|p| resolve_expr(p, ctx)).collect(),
        },
        Expr::Index { span, base, index } => Expr::Index {
            span: *span,
            base: Box::new(resolve_expr(base, ctx)),
            index: Box::new(resolve_expr(index, ctx)),
        },
        Expr::PartSelect { span, base, msb, lsb } => Expr::PartSelect {
            span: *span,
            base: Box::new(resolve_expr(base, ctx)),
            msb: Box::new(resolve_expr(msb, ctx)),
            lsb: Box::new(resolve_expr(lsb, ctx)),
        },

        Expr::Call { name, args, .. } => resolve_call(name, args, ctx, expr.span()),

        Expr::AccessFunction { span, function, pos, neg } => Expr::AccessFunction {
            span: *span,
            function: function.clone(),
            pos: Box::new(resolve_expr(pos, ctx)),
            neg: neg.as_ref().map(|n| Box::new(resolve_expr(n, ctx))),
        },

        Expr::SystemFunctionCall { span, name, args } => Expr::SystemFunctionCall {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| resolve_expr(a, ctx)).collect(),
        },
    }
}

fn resolve_call(name: &str, args: &[Expr], ctx: &mut LowerCtx, span: Span) -> Expr {
    if name == "V" || name == "I" {
        // Should not normally reach here (V/I are AccessFunction nodes, not
        // Call nodes, in the VAMS grammar) — kept as a defensive fallback,
        // resolving args the same way, so a marker-construction site that
        // still spells a branch read as `Call{"V", ...}` is not silently
        // dropped.
        let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
        return Expr::Call { span, name: name.to_string(), args: resolved };
    }

    if let Some(op) = analog_ops().lookup(name) {
        return op.lower(args, ctx);
    }

    if crate::resolve::math::math_fn(name).is_some() {
        let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
        return Expr::Call { span, name: name.to_string(), args: resolved };
    }

    if let Some(fn_id) = ctx.symbols.fn_by_name(name) {
        return inline_user_fn(fn_id, name, args, ctx);
    }

    // An unrecognized call: a general system task/function
    // (`$bound_step`/`$temperature`/…, ch. 9) or a not-yet-lowered
    // user-defined `analog function` (MD-45 — VAMS function/task decls
    // aren't lowered into the POM yet, see body.rs's own note); resolve
    // args and pass the call through unresolved-name-and-all rather than a
    // silent 0.0 — the emitter's own name resolution is the next fail-loud
    // gate for a name that truly doesn't exist.
    let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
    Expr::Call { span, name: name.to_string(), args: resolved }
}

/// Inline a user function call by substituting parameters. The function
/// body is POM `Stmt`; walk it and substitute `Ident(param_name)` with the
/// corresponding argument expression. Reachable once a later phase lowers
/// `analog function`/`function` declarations into `SymbolTable` (MD-45) —
/// `ctx.symbols.fn_by_name` never matches today, so this path is currently
/// dead-but-not-deleted: the mechanism is generic over any future producer,
/// same latitude T21 used for `ElabContext`'s emptied-but-kept registries.
fn inline_user_fn(fn_id: FnId, name: &str, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
    let function = match ctx.symbols.try_fn(fn_id) {
        Some(f) => f.clone(),
        None => {
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "function",
                name: name.to_string(),
            });
            return real(0.0);
        }
    };

    let mut full_args: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
    while full_args.len() < function.params.len() {
        let i = full_args.len();
        match function.defaults.get(i).and_then(|d| d.as_ref()) {
            Some(default) => full_args.push(default.clone()),
            None => {
                ctx.errors.push(super::LowerError {
                    module: ctx.module_name.clone(),
                    what: "function arg (missing, no default)",
                    name: format!("{} arg #{}", function.name, i + 1),
                });
                return real(0.0);
            }
        }
    }

    let mut subst: std::collections::HashMap<String, Expr> = std::collections::HashMap::new();
    for (&param_id, arg) in function.params.iter().zip(full_args) {
        let pname = ctx.symbols.var(param_id).name.clone();
        subst.insert(pname, arg);
    }

    let body = function.body.clone();
    inline_fn_body(&body, &mut subst)
}

fn inline_fn_body(
    stmts: &[piperine_lang::ast::Stmt],
    subst: &mut std::collections::HashMap<String, Expr>,
) -> Expr {
    use piperine_lang::ast::Stmt as S;
    for s in stmts {
        match s {
            S::Assign { target: Expr::Ident { name, .. }, value, .. } => {
                let val = subst_expr(value, subst);
                subst.insert(name.clone(), val);
            }
            S::Block { stmts, .. } => {
                let result = inline_fn_body(stmts, subst);
                if !matches!(result, Expr::Number { value: NumberLit::Real(v), .. } if v == 0.0) {
                    return result;
                }
            }
            _ => {}
        }
    }
    real(0.0)
}

pub(crate) fn subst_expr(expr: &Expr, subst: &std::collections::HashMap<String, Expr>) -> Expr {
    match expr {
        Expr::Ident { name, .. } => subst.get(name).cloned().unwrap_or_else(|| expr.clone()),
        Expr::Unary { span, op, operand } => {
            Expr::Unary { span: *span, op: *op, operand: Box::new(subst_expr(operand, subst)) }
        }
        Expr::Binary { span, op, lhs, rhs } => Expr::Binary {
            span: *span,
            op: *op,
            lhs: Box::new(subst_expr(lhs, subst)),
            rhs: Box::new(subst_expr(rhs, subst)),
        },
        Expr::Ternary { span, cond, then_branch, else_branch } => Expr::Ternary {
            span: *span,
            cond: Box::new(subst_expr(cond, subst)),
            then_branch: Box::new(subst_expr(then_branch, subst)),
            else_branch: Box::new(subst_expr(else_branch, subst)),
        },
        Expr::Call { span, name, args } => Expr::Call {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| subst_expr(a, subst)).collect(),
        },
        Expr::SystemFunctionCall { span, name, args } => Expr::SystemFunctionCall {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| subst_expr(a, subst)).collect(),
        },
        Expr::AccessFunction { span, function, pos, neg } => Expr::AccessFunction {
            span: *span,
            function: function.clone(),
            pos: Box::new(subst_expr(pos, subst)),
            neg: neg.as_ref().map(|n| Box::new(subst_expr(n, subst))),
        },
        Expr::Index { span, base, index } => Expr::Index {
            span: *span,
            base: Box::new(subst_expr(base, subst)),
            index: Box::new(subst_expr(index, subst)),
        },
        Expr::PartSelect { span, base, msb, lsb } => Expr::PartSelect {
            span: *span,
            base: Box::new(subst_expr(base, subst)),
            msb: Box::new(subst_expr(msb, subst)),
            lsb: Box::new(subst_expr(lsb, subst)),
        },
        Expr::Concat { span, parts } => {
            Expr::Concat { span: *span, parts: parts.iter().map(|p| subst_expr(p, subst)).collect() }
        }
        Expr::Replication { span, count, parts } => Expr::Replication {
            span: *span,
            count: Box::new(subst_expr(count, subst)),
            parts: parts.iter().map(|p| subst_expr(p, subst)).collect(),
        },
        Expr::AssignmentPattern { span, replicate, parts } => Expr::AssignmentPattern {
            span: *span,
            replicate: replicate.as_ref().map(|r| Box::new(subst_expr(r, subst))),
            parts: parts.iter().map(|p| subst_expr(p, subst)).collect(),
        },
        // Never reaches codegen — see `walk`'s own PortAccess arm above.
        Expr::Number { .. } | Expr::Str { .. } | Expr::PortAccess { .. } => expr.clone(),
    }
}

/// Resolve a single VAMS `Stmt`'s expressions (analog path — keeps POM
/// `Stmt` structure; the flattener/Builder dispatch on it directly).
pub(crate) fn resolve_stmt(stmt: &piperine_lang::ast::Stmt, ctx: &mut LowerCtx) -> piperine_lang::ast::Stmt {
    use piperine_lang::ast::Stmt as S;
    match stmt {
        S::Null { span } => S::Null { span: *span },
        S::Assign { span, target, value } => {
            S::Assign { span: *span, target: resolve_expr(target, ctx), value: resolve_expr(value, ctx) }
        }
        S::NonBlockingAssign { span, target, value } => S::NonBlockingAssign {
            span: *span,
            target: resolve_expr(target, ctx),
            value: resolve_expr(value, ctx),
        },
        S::Contribution { span, target, value } => {
            // Noise is scanned from the *unresolved* source, on the
            // contribution's own branch — a nested contribution belongs to
            // its own branch, not to ground↔ground, and resolving first
            // would rewrite a `white_noise(…)` call away before it's seen.
            let (_nature, plus, minus) = parse_contrib_dest(target, ctx);
            scan_noise(value, plus, minus, ctx);
            S::Contribution { span: *span, target: resolve_expr(target, ctx), value: resolve_expr(value, ctx) }
        }
        S::Block { span, name, decls, stmts } => S::Block {
            span: *span,
            name: name.clone(),
            decls: decls.clone(),
            stmts: stmts.iter().map(|s| resolve_stmt(s, ctx)).collect(),
        },
        S::If { span, cond, then_branch, else_branch } => S::If {
            span: *span,
            cond: resolve_expr(cond, ctx),
            then_branch: Box::new(resolve_stmt(then_branch, ctx)),
            else_branch: else_branch.as_ref().map(|b| Box::new(resolve_stmt(b, ctx))),
        },
        S::Case { span, kind, selector, arms } => S::Case {
            span: *span,
            kind: *kind,
            selector: resolve_expr(selector, ctx),
            arms: arms
                .iter()
                .map(|arm| piperine_lang::ast::CaseArm {
                    span: arm.span,
                    labels: arm.labels.as_ref().map(|ls| ls.iter().map(|l| resolve_expr(l, ctx)).collect()),
                    body: Box::new(resolve_stmt(&arm.body, ctx)),
                })
                .collect(),
        },
        S::For { span, init, cond, step, body } => S::For {
            span: *span,
            init: Box::new(resolve_stmt(init, ctx)),
            cond: resolve_expr(cond, ctx),
            step: Box::new(resolve_stmt(step, ctx)),
            body: Box::new(resolve_stmt(body, ctx)),
        },
        S::While { span, cond, body } => {
            S::While { span: *span, cond: resolve_expr(cond, ctx), body: Box::new(resolve_stmt(body, ctx)) }
        }
        S::Repeat { span, count, body } => {
            S::Repeat { span: *span, count: resolve_expr(count, ctx), body: Box::new(resolve_stmt(body, ctx)) }
        }
        S::Forever { span, body } => S::Forever { span: *span, body: Box::new(resolve_stmt(body, ctx)) },
        S::EventControl { span, events, body } => S::EventControl {
            span: *span,
            events: events.iter().map(|e| resolve_event_expr(e, ctx)).collect(),
            body: Box::new(resolve_stmt(body, ctx)),
        },
        S::DelayControl { span, delay, body } => S::DelayControl {
            span: *span,
            delay: resolve_expr(delay, ctx),
            body: Box::new(resolve_stmt(body, ctx)),
        },
        S::Disable { span, name } => S::Disable { span: *span, name: name.clone() },
        S::TaskCall { span, name, args } => S::TaskCall {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| resolve_expr(a, ctx)).collect(),
        },
        // No expression to resolve — `name` is an event identifier, not a
        // value.
        S::EventTrigger { span, name } => S::EventTrigger { span: *span, name: name.clone() },
    }
}

fn resolve_event_expr(e: &piperine_lang::ast::EventExpr, ctx: &mut LowerCtx) -> piperine_lang::ast::EventExpr {
    use piperine_lang::ast::EventExpr as E;
    match e {
        E::Level(x) => E::Level(resolve_expr(x, ctx)),
        E::Posedge(x) => E::Posedge(resolve_expr(x, ctx)),
        E::Negedge(x) => E::Negedge(resolve_expr(x, ctx)),
        E::ImplicitAll => E::ImplicitAll,
    }
}
