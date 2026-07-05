//! Behavior-statement lowering: `analog`/`digital` block statements and
//! function-body statements → `IrStmt`, plus the branch phi-merge that
//! keeps a `var` reassigned in an `if` consistent across arms.

use piperine_lang::pom::BehaviorStmt;
use piperine_lang::parse::ast::{BindOp, Expr, Literal, Pattern};

use crate::lower::*;

use super::event::convert_event_spec;
use super::expr::{lower_expr, parse_contrib_dest, scan_noise};
use super::LowerCtx;

pub(crate) fn first_state_ref(expr: &IrExpr) -> Option<StateId> {
    match expr {
        IrExpr::State(id) => Some(*id),
        IrExpr::Unary(_, e) => first_state_ref(e),
        IrExpr::Binary(_, l, r) => first_state_ref(l).or_else(|| first_state_ref(r)),
        IrExpr::Call(_, args) | IrExpr::MathCall(_, args) => args.iter().find_map(first_state_ref),
        IrExpr::Select(c, t, e) => first_state_ref(c).or_else(|| first_state_ref(t)).or_else(|| first_state_ref(e)),
        IrExpr::Array(elems) => elems.iter().find_map(first_state_ref),
        IrExpr::Index(b, i) => first_state_ref(b).or_else(|| first_state_ref(i)),
        IrExpr::Slice(b, s, e, _) => first_state_ref(b).or_else(|| first_state_ref(s)).or_else(|| first_state_ref(e)),
        IrExpr::AcStim { mag, phase } => first_state_ref(mag).or_else(|| first_state_ref(phase)),
        _ => None,
    }
}

pub(crate) fn lower_stmts(stmts: &[BehaviorStmt], ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let mut out = vec![];
    for stmt in stmts {
        out.extend(lower_stmt(stmt, ctx));
    }
    out
}

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
                if !ctx.env.contains_key(name) && ctx.module_vars.contains(name) {
                    let var_id = ctx.lookup_var(name).unwrap_or_else(|| ctx.symbols.add_var(name.clone(), Type::Real));
                    vec![IrStmt::Assign { lval: Lval::Var(var_id), expr: val }]
                } else {
                    ctx.env.insert(name.clone(), val);
                    vec![]
                }
            } else {
                vec![]
            }
        }
        BehaviorStmt::Bind { dest, op: BindOp::Contrib, src } => {
            let (nature, plus, minus) = parse_contrib_dest(dest, ctx);
            scan_noise(src, plus, minus, ctx);
            let expr = lower_expr(src, ctx);
            let kind = if let Some(id) = first_state_ref(&expr) {
                ContribKind::Reactive(id)
            } else {
                ContribKind::Resistive
            };
            vec![IrStmt::Contrib { nature, plus, minus, expr, kind }]
        }
        BehaviorStmt::Bind { dest, op: BindOp::Force, src } => {
            let expr = lower_expr(src, ctx);
            if ctx.is_digital {
                if let Expr::Ident(name) = dest {
                    if let Some(node_id) = ctx.lookup_node(name) {
                        vec![IrStmt::Assign { lval: Lval::Net(node_id), expr }]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                let (nature, plus, minus) = parse_contrib_dest(dest, ctx);
                vec![IrStmt::Force { nature, plus, minus, expr }]
            }
        }
        BehaviorStmt::If { cond, then_body, else_body } => {
            let cond_ir = lower_expr(cond, ctx);
            let pre_env = ctx.env.clone();
            
            let mut then_env = pre_env.clone();
            std::mem::swap(&mut ctx.env, &mut then_env);
            let then_ = lower_stmts(&then_body.stmts, ctx);
            std::mem::swap(&mut ctx.env, &mut then_env);
            
            let mut else_env = pre_env.clone();
            std::mem::swap(&mut ctx.env, &mut else_env);
            let else_ = else_body.as_ref().map(|b| lower_stmts(&b.stmts, ctx)).unwrap_or_default();
            std::mem::swap(&mut ctx.env, &mut else_env);
            
            merge_branch_env(&pre_env, &then_env, &else_env, &cond_ir, &mut ctx.env);
            vec![IrStmt::If { cond: cond_ir, then_, else_ }]
        }
        BehaviorStmt::Match { expr, arms } => {
            lower_match(expr, arms, ctx)
        }
        BehaviorStmt::Event { spec, guard, body } => {
            let kinds = convert_event_spec(spec, ctx);
            
            let mut body_env = ctx.env.clone();
            std::mem::swap(&mut ctx.env, &mut body_env);
            let body_ir = lower_stmts(&body.stmts, ctx);
            std::mem::swap(&mut ctx.env, &mut body_env);
            
            let body_with_guard = if let Some(guard_expr) = guard {
                let guard_ir = lower_expr(guard_expr, ctx);
                vec![IrStmt::If { cond: guard_ir, then_: body_ir, else_: vec![] }]
            } else {
                body_ir
            };
            kinds.into_iter().map(|kind| {
                match kind {
                    super::event::LoweredEvent::Analog(source) => {
                        IrStmt::AnalogEvent(AnalogEvent { source, body: body_with_guard.clone() })
                    }
                    super::event::LoweredEvent::Digital(event) => {
                        IrStmt::ClockedBlock { event, body: body_with_guard.clone() }
                    }
                }
            }).collect()
        }
        BehaviorStmt::Diagnostic { sys, args } => {
            let bare = sys.trim_start_matches('$');
            match bare {
                "bound_step" => {
                    let e = args.first().map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
                    return vec![IrStmt::BoundStep(e)];
                }
                "finish" | "stop" => return vec![IrStmt::Finish],
                "discontinuity" => {
                    let n = args.first().and_then(|a| {
                        if let Expr::Literal(Literal::Int(n)) = a { Some(*n as i32) } else { None }
                    }).unwrap_or(0);
                    return vec![IrStmt::Discontinuity(n as u8)];
                }
                _ => {}
            }
            let severity = match bare {
                "warning" | "warn" => Severity::Warn,
                "error" => Severity::Error,
                "fatal" => Severity::Fatal,
                _ => Severity::Info,
            };
            let (fmt, ir_args) = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                (s.clone(), args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect())
            } else {
                (String::new(), args.iter().map(|a| lower_expr(a, ctx)).collect())
            };
            vec![IrStmt::Diagnostic { severity, format: fmt, args: ir_args }]
        }
        BehaviorStmt::Expr(e) => lower_expr_stmt(e, ctx),
        BehaviorStmt::Return(e) => vec![IrStmt::Return(Some(lower_expr(e, ctx)))],
        BehaviorStmt::For { var, .. } => {
            // Behavioral `for`s unroll at elaboration; one reaching the IR
            // lowering means a phase was skipped — fail loud.
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "unlowered behavioral `for` (loop var)",
                name: var.clone(),
            });
            vec![]
        }
    }
}

pub(crate) fn lower_expr_stmt(expr: &Expr, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    if let Expr::SysCall(name, args) = expr {
        match name.trim_start_matches('$') {
            "bound_step" => {
                let e = args.first().map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
                return vec![IrStmt::BoundStep(e)];
            }
            "finish" | "stop" => return vec![IrStmt::Finish],
            "discontinuity" => {
                let n = args.first().and_then(|a| {
                    if let Expr::Literal(Literal::Int(n)) = a { Some(*n as i32) } else { None }
                }).unwrap_or(0);
                return vec![IrStmt::Discontinuity(n as u8)];
            }
            n if matches!(n, "display" | "write" | "strobe" | "monitor" | "warning" | "warn" | "error" | "fatal" | "info") => {
                let severity = match n {
                    "warning" | "warn" => Severity::Warn,
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

pub(crate) fn lower_match(expr: &Expr, arms: &[piperine_lang::pom::MatchArm], ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let discriminant = lower_expr(expr, ctx);
    let mut default_body = vec![];
    let mut chain: Vec<(IrExpr, Vec<IrStmt>)> = vec![];

    for arm in arms {
        match &arm.pat {
            Pattern::Wildcard => {
                let mut bctx = LowerCtx::new(
                    ctx.symbols,
                    ctx.module_name.clone(),
                    ctx.is_digital,
                    ctx.module_vars.clone(),
                );
                bctx.env = ctx.env.clone();
                bctx.enum_values = ctx.enum_values.clone();
                bctx.consts = ctx.consts.clone();
                default_body = lower_stmts(&arm.body.stmts, &mut bctx);

                // Inherit state variables, errors, and shadows from the arm.
                for s in bctx.states { if !ctx.states.contains(&s) { ctx.states.push(s); } }
                ctx.noise_sources.extend(bctx.noise_sources.iter().cloned());
                ctx.errors.append(&mut bctx.errors);
                ctx.digital_shadows.append(&mut bctx.digital_shadows);
            }
            pattern @ (Pattern::Path(_) | Pattern::Literal(_) | Pattern::BitPattern(_)) => {
                let cond = pattern_match_cond(pattern, &discriminant, ctx);
                let mut bctx = LowerCtx::new(
                    ctx.symbols,
                    ctx.module_name.clone(),
                    ctx.is_digital,
                    ctx.module_vars.clone(),
                );
                bctx.env = ctx.env.clone();
                bctx.enum_values = ctx.enum_values.clone();
                bctx.consts = ctx.consts.clone();
                let body = lower_stmts(&arm.body.stmts, &mut bctx);
                chain.push((cond, body));

                // Inherit state variables, errors, and shadows from the arm.
                for s in bctx.states { if !ctx.states.contains(&s) { ctx.states.push(s); } }
                ctx.noise_sources.extend(bctx.noise_sources.iter().cloned());
                ctx.errors.append(&mut bctx.errors);
                ctx.digital_shadows.append(&mut bctx.digital_shadows);
            }
        }
    }

    if chain.is_empty() {
        return default_body;
    }

    let mut result = default_body;
    for (cond, body) in chain.into_iter().rev() {
        result = vec![IrStmt::If {
            cond,
            then_: body,
            else_: result,
        }];
    }
    result
}

/// The boolean condition under which a match pattern accepts the
/// discriminant. Enum paths compare against the variant discriminant,
/// literals against their value, and a bit pattern (`0b1??0`)
/// mask-compares: `(d & mask) == value` with `?` bits masked out.
fn pattern_match_cond(pattern: &Pattern, discriminant: &IrExpr, ctx: &mut LowerCtx) -> IrExpr {
    match pattern {
        Pattern::Wildcard => IrExpr::Bool(true),
        Pattern::Path(p) => {
            let joined = p.segments.join("::");
            // An enum variant is an integer constant (SPEC §6.4); any
            // other path is a parameter reference.
            let target = match ctx.lookup_enum_value(&joined) {
                Some(value) => IrExpr::Int(value),
                None => IrExpr::Param(ctx.require_ident_as_param(&joined)),
            };
            IrExpr::binary(BinOp::Eq, discriminant.clone(), target)
        }
        Pattern::Literal(v) => {
            IrExpr::binary(BinOp::Eq, discriminant.clone(), IrExpr::Int(*v as i64))
        }
        Pattern::BitPattern(bits) => {
            let mut mask = 0i64;
            let mut value = 0i64;
            for c in bits.chars() {
                mask <<= 1;
                value <<= 1;
                match c {
                    '0' => mask |= 1,
                    '1' => {
                        mask |= 1;
                        value |= 1;
                    }
                    _ => {} // '?' — don't care
                }
            }
            IrExpr::binary(
                BinOp::Eq,
                IrExpr::binary(BinOp::BitAnd, discriminant.clone(), IrExpr::Int(mask)),
                IrExpr::Int(value),
            )
        }
    }
}

pub(crate) fn merge_branch_env(
    pre_env: &std::collections::HashMap<String, IrExpr>,
    then_env: &std::collections::HashMap<String, IrExpr>,
    else_env: &std::collections::HashMap<String, IrExpr>,
    cond: &IrExpr,
    ctx_env: &mut std::collections::HashMap<String, IrExpr>,
) {
    let all_keys: std::collections::HashSet<&String> = then_env.keys().chain(else_env.keys()).collect();
    for key in all_keys {
        let pre_val = pre_env.get(key);
        let then_val = then_env.get(key);
        let else_val = else_env.get(key);
        let then_changed = then_val != pre_val;
        let else_changed = else_val != pre_val;
        if !then_changed && !else_changed {
            continue;
        }
        let tv = then_val.or(pre_val).cloned().unwrap_or(IrExpr::Real(0.0));
        let ev = else_val.or(pre_val).cloned().unwrap_or(IrExpr::Real(0.0));
        ctx_env.insert(key.clone(), IrExpr::Select(Box::new(cond.clone()), Box::new(tv), Box::new(ev)));
    }
}
