//! Behavior-statement lowering: `analog`/`digital` block statements and
//! function-body statements → `IrStmt`, plus the branch phi-merge that
//! keeps a `var` reassigned in an `if` consistent across arms.

use crate::pom::BehaviorStmt;
use crate::parse::ast::{BindOp, Expr, Literal, Pattern};

use piperine_codegen::ir::*;

use super::event::convert_event_spec;
use super::expr::{lower_expr, parse_contrib_dest, scan_noise};
use super::LowerCtx;

// ─── Statement lowering ───────────────────────────────────────────────────────

/// Lower a slice of behavior statements into a flat vector of [`IrStmt`].
pub(crate) fn lower_stmts(stmts: &[BehaviorStmt], ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let mut out = vec![];
    for stmt in stmts {
        out.extend(lower_stmt(stmt, ctx));
    }
    out
}

/// Lower a single behavior statement into zero or more [`IrStmt`]s,
/// handling variable declarations, bind operations, conditionals, events,
/// and diagnostics.
pub(crate) fn lower_stmt(stmt: &BehaviorStmt, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    match stmt {
        BehaviorStmt::VarDecl { name, default: Some(expr), .. } => {
            let val = lower_expr(expr, ctx);
            ctx.env.insert(name.clone(), val);
            vec![]
        }
        BehaviorStmt::VarDecl { name, default: None, .. } => {
            ctx.env.insert(name.clone(), IrExpr::Real(0.0));
            vec![]
        }
        BehaviorStmt::Bind { dest, op: BindOp::Assign, src } => {
            if let Expr::Ident(name) = dest {
                let val = lower_expr(src, ctx);
                // GAPS §I.15 — a write to a module-level persistent `var`
                // (and not shadowed by a same-named behavior-local `var`)
                // is real runtime state: emit an `IrStmt::Assign` instead
                // of inlining into `env`. `merge_branch_ctx` only looks at
                // `env`, so this deliberately bypasses phi-merging — the
                // write is unconditional IR, not a value substitution.
                if !ctx.env.contains_key(name) && ctx.module_vars.contains(name) {
                    vec![IrStmt::Assign { lval: name.clone(), expr: val, delay: None, event: None }]
                } else {
                    ctx.env.insert(name.clone(), val);
                    vec![]
                }
            } else {
                vec![]
            }
        }
        BehaviorStmt::Bind { dest, op: BindOp::Contrib, src } => {
            let (nature, plus, minus) = parse_contrib_dest(dest);
            scan_noise(src, &plus, &minus, ctx);
            let expr = lower_expr(src, ctx);
            let kind = if let Some(id) = first_state_ref(&expr) {
                ContribKind::Reactive(id)
            } else {
                ContribKind::Resistive
            };
            vec![IrStmt::Contrib { nature, plus, minus, expr, kind }]
        }
        BehaviorStmt::Bind { dest, op: BindOp::Force, src } => {
            // Two semantics for `<-`:
            //   * inside `analog { ... }`           →  IrStmt::Force (analog)
            //   * inside `digital { ... }`          →  IrStmt::Assign (digital drive)
            // We know which one we're in via the `LowerCtx.is_digital`
            // flag, set in `ppr_to_ir` before calling lower_stmts.
            let expr = lower_expr(src, ctx);
            if ctx.is_digital {
                if let Expr::Ident(name) = dest {
                    vec![IrStmt::Assign { lval: name.clone(), expr, delay: None, event: None }]
                } else {
                    vec![]
                }
            } else {
                let (nature, plus, minus) = parse_contrib_dest(dest);
                vec![IrStmt::Force { nature, plus, minus, expr }]
            }
        }

        BehaviorStmt::If { cond, then_body, else_body } => {
            let cond_ir = lower_expr(cond, ctx);
            let pre_env = ctx.env.clone();
            let mut then_ctx = ctx.clone();
            let then_ = lower_stmts(then_body, &mut then_ctx);
            let mut else_ctx = ctx.clone();
            let else_ = else_body
                .as_ref()
                .map(|b| lower_stmts(b, &mut else_ctx))
                .unwrap_or_default();
            merge_branch_ctx(&pre_env, &then_ctx, &else_ctx, &cond_ir, ctx);
            vec![IrStmt::If { cond: cond_ir, then_, else_, label: None }]
        }

        BehaviorStmt::Match { expr, arms } => {
            lower_match(expr, arms, ctx)
        }

        BehaviorStmt::Event { spec, guard, body } => {
            let kinds = convert_event_spec(spec, ctx);
            let body_ir = lower_stmts(body, &mut ctx.clone());
            // Wrap body in guard if present
            let body_with_guard = if let Some(guard_expr) = guard {
                let guard_ir = lower_expr(guard_expr, &mut ctx.clone());
                vec![IrStmt::If { cond: guard_ir, then_: body_ir, else_: vec![], label: None }]
            } else {
                body_ir
            };
            kinds.into_iter().map(|kind| IrStmt::AnalogEvent {
                kind,
                body: body_with_guard.clone(),
            }).collect()
        }

        BehaviorStmt::Diagnostic { sys, args } => {
            let bare = sys.trim_start_matches('$');
            // Special system tasks that are not display-family
            match bare {
                "bound_step" => {
                    let e = args.first()
                        .map(|a| lower_expr(a, ctx))
                        .unwrap_or(IrExpr::Real(0.0));
                    return vec![IrStmt::BoundStep(e)];
                }
                "finish" | "stop" => return vec![IrStmt::Finish],
                "discontinuity" => {
                    let n = args.first()
                        .and_then(|a| {
                            if let Expr::Literal(Literal::Int(n)) = a {
                                Some(*n as i32)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    return vec![IrStmt::Discontinuity(n)];
                }
                _ => {}
            }
            let severity = match bare {
                "warning" | "warn" => Severity::Warning,
                "error" => Severity::Error,
                "fatal" => Severity::Fatal,
                _ => Severity::Info,
            };
            // Extract format string from first arg if it's a string literal
            let (fmt, ir_args) = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                (s.clone(), args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect())
            } else {
                (String::new(), args.iter().map(|a| lower_expr(a, ctx)).collect())
            };
            vec![IrStmt::Diagnostic { severity, format: fmt, args: ir_args }]
        }

        BehaviorStmt::Expr(e) => lower_expr_stmt(e, ctx),

        // GAPS §D.5 — preserve the trailing `Return(e)` of a fn body so
        // the inliner can find the fn's return value.
        BehaviorStmt::Return(e) => vec![IrStmt::Return(Some(lower_expr(e, ctx)))],
    }
}

/// Lower an expression statement, converting known system-call expressions
/// (e.g. `$bound_step`, `$display`) into their IR statement forms.
pub(crate) fn lower_expr_stmt(expr: &Expr, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    if let Expr::SysCall(name, args) = expr {
        match name.trim_start_matches('$') {
            "bound_step" => {
                let e = args.first()
                    .map(|a| lower_expr(a, ctx))
                    .unwrap_or(IrExpr::Real(0.0));
                return vec![IrStmt::BoundStep(e)];
            }
            "finish" | "stop" => return vec![IrStmt::Finish],
            "discontinuity" => {
                let n = args.first()
                    .and_then(|a| {
                        if let Expr::Literal(Literal::Int(n)) = a {
                            Some(*n as i32)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                return vec![IrStmt::Discontinuity(n)];
            }
            n if matches!(n, "display" | "write" | "strobe" | "monitor" | "warning" | "warn" | "error" | "fatal" | "info") => {
                let severity = match n {
                    "warning" | "warn" => Severity::Warning,
                    "error" => Severity::Error,
                    "fatal" => Severity::Fatal,
                    _ => Severity::Info,
                };
                let (fmt, ir_args) = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                    (s.clone(), args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect())
                } else {
                    (String::new(), args.iter().map(|a| lower_expr(a, ctx)).collect())
                };
                return vec![IrStmt::Diagnostic { severity, format: fmt, args: ir_args }];
            }
            _ => {}
        }
    }
    vec![]
}

/// Desugar a `match` into an if/else-if chain.
pub(crate) fn lower_match(expr: &Expr, arms: &[crate::pom::MatchArm], ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let discriminant = lower_expr(expr, ctx);
    let mut default_body = vec![];
    let mut chain: Vec<(IrExpr, Vec<IrStmt>)> = vec![];

    for arm in arms {
        match arm.pattern() {
            Pattern::Wildcard => {
                default_body = lower_stmts(arm.body(), &mut ctx.clone());
            }
            Pattern::Path(p) => {
                let pat_expr = IrExpr::Param(p.segments.join("::"));
                let body = lower_stmts(arm.body(), &mut ctx.clone());
                chain.push((pat_expr, body));
            }
        }
    }

    // Build if/else-if chain from the arms
    if chain.is_empty() {
        return default_body;
    }

    // Build from the inside out
    let mut result = default_body;
    for (pat, body) in chain.into_iter().rev() {
        let cond = IrExpr::Binary(IrBinOp::Eq, Box::new(discriminant.clone()), Box::new(pat));
        result = vec![IrStmt::If {
            cond,
            then_: body,
            else_: result,
            label: None,
        }];
    }
    result
}

// ─── Phi-node env merge ───────────────────────────────────────────────────────

/// Merge the environment and state-variable lists of two branch contexts
/// (`then` / `else`) after lowering, inserting `Select` IR nodes for
/// variables that changed in either arm.
pub(crate) fn merge_branch_ctx(
    pre_env: &std::collections::HashMap<String, IrExpr>,
    then_ctx: &LowerCtx,
    else_ctx: &LowerCtx,
    cond: &IrExpr,
    ctx: &mut LowerCtx,
) {
    let all_keys: std::collections::HashSet<&String> = then_ctx.env.keys()
        .chain(else_ctx.env.keys())
        .collect();

    for key in all_keys {
        let pre_val = pre_env.get(key);
        let then_val = then_ctx.env.get(key);
        let else_val = else_ctx.env.get(key);

        let then_changed = then_val != pre_val;
        let else_changed = else_val != pre_val;
        if !then_changed && !else_changed {
            continue;
        }

        let tv = then_val.or(pre_val).cloned().unwrap_or(IrExpr::Real(0.0));
        let ev = else_val.or(pre_val).cloned().unwrap_or(IrExpr::Real(0.0));
        ctx.env.insert(key.clone(), IrExpr::Select(
            Box::new(cond.clone()),
            Box::new(tv),
            Box::new(ev),
        ));
    }

    for sv in then_ctx.state_vars.iter().chain(else_ctx.state_vars.iter()) {
        if !ctx.state_vars.iter().any(|s| s.id == sv.id) {
            ctx.state_vars.push(sv.clone());
        }
    }
    ctx.noise_sources.extend(then_ctx.noise_sources.iter().cloned());
    ctx.noise_sources.extend(else_ctx.noise_sources.iter().cloned());
    ctx.counter = ctx.counter.max(then_ctx.counter).max(else_ctx.counter);
}

