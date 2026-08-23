//! Expression resolution for analog bodies: walks POM `Expr`
//! (`piperine_lang::ast::Expr`) and resolves constants, analog operators
//! (→ `__ddt(id, x)` markers), and V/I/math left as-is (the Builder
//! resolves names to ids at JIT emit time via the `Resolver`).

use piperine_lang::ast::Expr;
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::NumberLit;

use crate::resolve::*;

use super::analog_ops::analog_ops;
use super::LowerCtx;

const NOSPAN: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

/// The shared no-provenance span for synthesized expressions (margin
/// lowering's `min`/division/subtraction wrappers) — the authored spans
/// live on the predicate's own nodes.
pub(crate) fn nospan() -> PreprocessedSpan {
    NOSPAN
}

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
        // `Hier`'s path is a `Vec<String>` of literal segments, not
        // sub-`Expr`s (unlike `Index`'s subscript) — nothing to recurse
        // into, same as `Ident`. It also never reaches this walk in
        // practice: `resolve_expr` resolves every `Hier` to an `Ident`
        // before `scan_noise`'s post-resolve callers run.
        Expr::Number { .. } | Expr::Str { .. } | Expr::Ident { .. } | Expr::PortAccess { .. } | Expr::Hier { .. } => {}
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

        // §6.7/§6.2.1 (T50, LRMW2-42): resolved to a plain node-name
        // `Ident` against this module's own flattened instance tree (or a
        // fail-loud placeholder) — see `LowerCtx::resolve_hier`'s own doc
        // comment for the full resolution rule.
        Expr::Hier { root, path, .. } => ctx.resolve_hier(*root, path),
    }
}

fn resolve_call(name: &str, args: &[Expr], ctx: &mut LowerCtx, span: PreprocessedSpan) -> Expr {
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

/// Inline a user function call by substituting parameters (§4.7). The
/// function body is POM `Stmt`; walk it, substitute `Ident(param_name)`
/// with the corresponding argument expression, and read the return value
/// off the body's assignment to the function's own name — VAMS has no
/// `return` statement, §4.7's own convention (Example 1: `maxValue = (n1 >
/// n2) ? n1 : n2;`). Mirrors the digital `Codegen` path's independent copy
/// (`emit/builder.rs::inline_user_fn`/`inline_fn_stmts`, which already got
/// this right) — kept as a separate copy because this one runs over the
/// *resolved-POM* `Expr` a stage earlier, before `flatten`/`emit` exist.
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

    // Recursive (direct or indirect) calls cannot terminate under this
    // compile-time substitution-based inliner — an implementation limit,
    // not an §4.7 prohibition (the LRM is silent on recursion), so failed
    // loud with the calling clause cited rather than hanging.
    if ctx.inlining_stack.iter().any(|n| n == name) {
        ctx.errors.push(super::LowerError {
            module: ctx.module_name.clone(),
            what: "function (recursive call — not supported by this inliner, LRM §4.7.3)",
            name: name.to_string(),
        });
        return real(0.0);
    }

    // VAMS Annex A's function-formal grammar has no default-value syntax
    // (only `parameter` declarations default, §4.7.1) — arity must match
    // exactly.
    if args.len() != function.params.len() {
        ctx.errors.push(super::LowerError {
            module: ctx.module_name.clone(),
            what: "function call arity (no default arguments exist — LRM §4.7)",
            name: format!("{} expects {} arg(s), got {}", function.name, function.params.len(), args.len()),
        });
        return real(0.0);
    }

    let resolved_args: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
    let mut subst: std::collections::HashMap<String, Expr> = std::collections::HashMap::new();
    for (i, &param_id) in function.params.iter().enumerate() {
        let pname = ctx.symbols.var(param_id).name.clone();
        let initial = match function.directions.get(i) {
            // §4.7.2.2: "All output arguments … are initialised to zero
            // (0)" — the caller's argument is a write-back destination
            // only, never a read.
            Some(piperine_lang::pom::Direction::Output) => real(0.0),
            // §4.7.2.3: `inout` — "do not get initialised to zero", the
            // caller's current value flows in.
            _ => resolved_args[i].clone(),
        };
        subst.insert(pname, initial);
    }

    ctx.inlining_stack.push(name.to_string());
    let mut result: Option<Expr> = None;
    inline_fn_body(&function.body, name, &mut subst, &mut result, ctx);
    ctx.inlining_stack.pop();

    // §4.7.2.1: "If this internal variable is not assigned … the function
    // will return the initialised value of zero (0)" — not an error.
    let result = result.unwrap_or_else(|| real(0.0));

    // `output`/`inout` args (§4.7.2.2/§4.7.2.3): the call-site argument
    // must be an analog variable reference — the final substituted value
    // of the matching formal writes back to it as a companion `Assign`,
    // drained by the enclosing `resolve_stmt`.
    for (i, dir) in function.directions.iter().enumerate() {
        if matches!(dir, piperine_lang::pom::Direction::Input) {
            continue;
        }
        let pname = ctx.symbols.var(function.params[i]).name.clone();
        let final_value = subst.get(&pname).cloned().unwrap_or_else(|| resolved_args[i].clone());
        match &args[i] {
            Expr::Ident { name: arg_name, span } => {
                ctx.write_backs.push(piperine_lang::ast::Stmt::Assign {
                    span: *span,
                    target: Expr::Ident { span: *span, name: arg_name.clone() },
                    value: final_value,
                });
            }
            _ => {
                ctx.errors.push(super::LowerError {
                    module: ctx.module_name.clone(),
                    what: "function output/inout argument must be an analog variable reference \
                           (LRM §4.7.2.2/§4.7.2.3)",
                    name: format!("{} arg #{}", function.name, i + 1),
                });
            }
        }
    }

    result
}

/// Walk a function body's statements, substituting reads and tracking
/// assignments; `*out` becomes `Some(value)` the moment an assignment
/// targets `fn_name` itself (§4.7's return convention) — later assignments
/// (there should be none in a well-formed straight-line body, but the walk
/// doesn't assume that) still update `subst` so a later expression can read
/// them. Control-flow statements (`if`/`case`/loops) inside the body are
/// not supported by this substitution-based inliner — §4.7/§5.2 permit
/// them, but choosing between branch values without actually emitting a
/// conditional is not faithful, so this fails loud (citing §4.7) instead of
/// guessing, rather than a silent no-op (CLAUDE.md's fail-loud rule).
fn inline_fn_body(
    stmts: &[piperine_lang::ast::Stmt],
    fn_name: &str,
    subst: &mut std::collections::HashMap<String, Expr>,
    out: &mut Option<Expr>,
    ctx: &mut LowerCtx,
) {
    use piperine_lang::ast::Stmt as S;
    for s in stmts {
        match s {
            S::Null { .. } => {}
            S::Assign { target: Expr::Ident { name, .. }, value, .. }
            | S::NonBlockingAssign { target: Expr::Ident { name, .. }, value, .. } => {
                // §4.7.3/Annex G item 12: a call inside this body to
                // *another* (or, self-referentially, this very) user
                // function is a real call, not a bare name to leave
                // unresolved — `subst_expr` alone only substitutes free
                // idents, it never inlines a nested `Call`, so this walks
                // the substituted expression and inlines every nested
                // user-function call through the same `inline_user_fn` —
                // which is exactly what makes the recursion guard above
                // reachable for a self-referential body.
                let val = expand_nested_calls(&subst_expr(value, subst), ctx);
                if name == fn_name {
                    *out = Some(val.clone());
                }
                subst.insert(name.clone(), val);
            }
            S::Block { stmts, .. } => inline_fn_body(stmts, fn_name, subst, out, ctx),
            other => {
                ctx.errors.push(super::LowerError {
                    module: ctx.module_name.clone(),
                    what: "function body statement kind (only straight-line assignment \
                           is inlined — control flow not supported, LRM §4.7/§5.2)",
                    name: format!("{fn_name}: {other:?}"),
                });
            }
        }
    }
}

/// Walks an already-substituted function-body expression and inlines every
/// nested user-function `Call` through [`inline_user_fn`] (§4.7.3/Annex G
/// item 12 — a user function may call another, or itself). Math builtins
/// and any other `Call` are left as-is (unchanged from `subst_expr`'s own
/// behavior) — only a name that resolves via `ctx.symbols.fn_by_name` is
/// touched.
fn expand_nested_calls(expr: &Expr, ctx: &mut LowerCtx) -> Expr {
    match expr {
        Expr::Call { name, args, span } => {
            let args: Vec<Expr> = args.iter().map(|a| expand_nested_calls(a, ctx)).collect();
            if let Some(fn_id) = ctx.symbols.fn_by_name(name) {
                inline_user_fn(fn_id, name, &args, ctx)
            } else {
                Expr::Call { span: *span, name: name.clone(), args }
            }
        }
        Expr::Unary { span, op, operand } => {
            Expr::Unary { span: *span, op: *op, operand: Box::new(expand_nested_calls(operand, ctx)) }
        }
        Expr::Binary { span, op, lhs, rhs } => Expr::Binary {
            span: *span,
            op: *op,
            lhs: Box::new(expand_nested_calls(lhs, ctx)),
            rhs: Box::new(expand_nested_calls(rhs, ctx)),
        },
        Expr::Ternary { span, cond, then_branch, else_branch } => Expr::Ternary {
            span: *span,
            cond: Box::new(expand_nested_calls(cond, ctx)),
            then_branch: Box::new(expand_nested_calls(then_branch, ctx)),
            else_branch: Box::new(expand_nested_calls(else_branch, ctx)),
        },
        // Every other shape either cannot contain a call operand in a
        // well-formed function body (Number/Str/Ident) or is outside the
        // "any statements available for conditional execution" subset
        // §4.7.1 permits (index/part-select/concat/replication/pattern) —
        // left as-is, same latitude `subst_expr` already takes.
        _ => expr.clone(),
    }
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
        // `Hier`'s path segments are literal strings (no `Ident` to
        // substitute into), so it passes through unchanged like `Ident`
        // would if `subst` had no entry for it.
        Expr::Number { .. } | Expr::Str { .. } | Expr::PortAccess { .. } | Expr::Hier { .. } => expr.clone(),
    }
}

/// Drains any pending `output`/`inout` function-argument write-backs
/// (§4.7, populated by `inline_user_fn` while resolving `stmt`'s
/// expression(s)) and, if any exist, wraps `stmt` together with them in a
/// synthetic `Block` — run in sequence, exactly as the original call site
/// plus its side-effects. Leaves `stmt` untouched when there are none (the
/// overwhelmingly common case: functions with no `output`/`inout` args).
fn with_write_backs(
    ctx: &mut LowerCtx,
    span: PreprocessedSpan,
    stmt: piperine_lang::ast::Stmt,
) -> piperine_lang::ast::Stmt {
    if ctx.write_backs.is_empty() {
        return stmt;
    }
    let mut stmts = vec![stmt];
    stmts.append(&mut ctx.write_backs);
    piperine_lang::ast::Stmt::Block { span, name: None, decls: Vec::new(), stmts }
}

/// Resolve a single VAMS `Stmt`'s expressions (analog path — keeps POM
/// `Stmt` structure; the flattener/Builder dispatch on it directly).
pub(crate) fn resolve_stmt(stmt: &piperine_lang::ast::Stmt, ctx: &mut LowerCtx) -> piperine_lang::ast::Stmt {
    use piperine_lang::ast::Stmt as S;
    match stmt {
        S::Null { span } => S::Null { span: *span },
        S::Assign { span, target, value } => {
            let resolved =
                S::Assign { span: *span, target: resolve_expr(target, ctx), value: resolve_expr(value, ctx) };
            with_write_backs(ctx, *span, resolved)
        }
        S::NonBlockingAssign { span, target, value } => {
            let resolved = S::NonBlockingAssign {
                span: *span,
                target: resolve_expr(target, ctx),
                value: resolve_expr(value, ctx),
            };
            with_write_backs(ctx, *span, resolved)
        }
        S::Contribution { span, target, value } => {
            // Noise is scanned from the *unresolved* source, on the
            // contribution's own branch — a nested contribution belongs to
            // its own branch, not to ground↔ground, and resolving first
            // would rewrite a `white_noise(…)` call away before it's seen.
            let (_nature, plus, minus) = parse_contrib_dest(target, ctx);
            scan_noise(value, plus, minus, ctx);
            let resolved =
                S::Contribution { span: *span, target: resolve_expr(target, ctx), value: resolve_expr(value, ctx) };
            with_write_backs(ctx, *span, resolved)
        }
        // §5.6.7 (LRMW2-41, T46): `lhs : probe == rhs;` — resolved
        // structurally like `Contribution` (each sub-expression through
        // the same `resolve_expr`); `flatten/analog.rs`'s own walk does
        // the actual desugar into the equivalent direct contribution
        // (§5.6.7's own algebraic identity, `lhs <+ lhs + (probe - rhs)`).
        S::IndirectContribution { span, lhs, probe, rhs } => {
            let resolved = S::IndirectContribution {
                span: *span,
                lhs: resolve_expr(lhs, ctx),
                probe: resolve_expr(probe, ctx),
                rhs: resolve_expr(rhs, ctx),
            };
            with_write_backs(ctx, *span, resolved)
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
        S::Case { span, kind, selector, arms, attrs } => S::Case {
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
            attrs: attrs.clone(),
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
