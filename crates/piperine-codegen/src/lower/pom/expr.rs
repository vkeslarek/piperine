//! Expression resolution for analog bodies: walks POM `Expr` and resolves
//! constants, enum values, analog operators (→ `__ddt(id, x)` markers), and
//! user function inlining — keeping the POM `Expr` structure. The Builder
//! resolves names to ids at JIT emit time via the `Resolver`.

use piperine_lang::parse::ast::{ArrayBody, Block, Expr, Literal, Stmt};
use crate::lower::*;

use super::analog_ops::analog_ops;
use super::LowerCtx;

pub(crate) fn parse_contrib_dest(dest: &Expr, ctx: &mut LowerCtx) -> (NatureId, NodeId, NodeId) {
    if let Expr::Call(func, args) = dest
        && let Expr::Ident(name) = func.as_ref() {
            let nature_kind = match name.as_str() {
                "V" => NatureKind::Potential,
                "I" => NatureKind::Flow,
                _ => NatureKind::Flow,
            };
            let nature = ctx.symbols.add_nature(name.as_str(), nature_kind);

            let plus_name = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
            let minus_name = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());

            let plus = ctx.require_node(&plus_name);
            let minus = ctx.require_node(&minus_name);

            return (nature, plus, minus);
        }

    let nature = ctx.symbols.add_nature("I", NatureKind::Flow);
    (nature, NodeId::GROUND, NodeId::GROUND)
}

pub(crate) fn ident_from_expr(e: Option<&Expr>) -> Option<String> {
    match e? {
        Expr::Ident(s) => Some(s.clone()),
        Expr::Field(base, field) => {
            match base.as_ref() {
                Expr::Ident(base_name) => Some(format!("{base_name}.{field}")),
                Expr::Index(inner, idx) => {
                    if let Expr::Ident(base_name) = inner.as_ref()
                        && let Expr::Literal(Literal::Int(i)) = idx.as_ref() {
                            return Some(format!("{base_name}[{i}].{field}"));
                        }
                    None
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn scan_noise(expr: &Expr, plus: NodeId, minus: NodeId, ctx: &mut LowerCtx) {
    use piperine_lang::parse::ast::Walk;
    expr.walk(&mut |e| {
        if let Expr::Call(func, args) = e
            && let Expr::Ident(name) = func.as_ref() {
                match name.as_str() {
                    "white_noise" => {
                        let psd = args.first()
                            .map(|a| resolve_expr(a, ctx))
                            .unwrap_or(Expr::Literal(Literal::Real(0.0)));
                        let label = args.get(1).and_then(|a| {
                            if let Expr::Literal(Literal::String(s)) = a { Some(s.clone()) } else { None }
                        });
                        ctx.noise_sources.push(NoiseSource {
                            plus,
                            minus,
                            kind: NoiseKind::White { psd },
                            label,
                        });
                        return Walk::SkipChildren;
                    }
                    "flicker_noise" => {
                        let psd = args.first()
                            .map(|a| resolve_expr(a, ctx))
                            .unwrap_or(Expr::Literal(Literal::Real(0.0)));
                        let exponent = args.get(1)
                            .map(|a| resolve_expr(a, ctx))
                            .unwrap_or(Expr::Literal(Literal::Real(1.0)));
                        let label = args.get(2).and_then(|a| {
                            if let Expr::Literal(Literal::String(s)) = a { Some(s.clone()) } else { None }
                        });
                        ctx.noise_sources.push(NoiseSource {
                            plus,
                            minus,
                            kind: NoiseKind::Flicker { psd, exponent },
                            label,
                        });
                        return Walk::SkipChildren;
                    }
                    _ => {}
                }
            }
        Walk::Continue
    });
}

/// Resolve a POM `Expr` for the analog path: constants/enum values → literals,
/// analog operators → `__ddt(id, x)` markers, user functions → inlined, V/I/math
/// kept as-is (Builder resolves at emit time).
pub(crate) fn resolve_expr(expr: &Expr, ctx: &mut LowerCtx) -> Expr {
    match expr {
        Expr::Literal(_) => expr.clone(),

        Expr::Ident(name) => {
            if let Some(value) = ctx.lookup_enum_value(name) {
                Expr::Literal(Literal::Int(value as u64))
            } else if let Some(c) = ctx.consts.get(name) {
                c.clone()
            } else if let Some(id) = ctx.lookup_node(name) {
                if ctx.is_digital {
                    expr.clone()
                } else if ctx.symbols.node(id).domain == crate::lower::Domain::Digital {
                    let var = ctx.shadow_var_for(id, name);
                    Expr::Ident(ctx.symbols.var(var).name.clone())
                } else {
                    // Analog node in an analog body — keep as Ident so
                    // the flattener (parse_dest) and Builder can resolve it.
                    expr.clone()
                }
            } else {
                expr.clone()
            }
        }

        Expr::Path(p) => {
            let joined = p.segments.join("::");
            if let Some(value) = ctx.lookup_enum_value(&joined) {
                Expr::Literal(Literal::Int(value as u64))
            } else {
                expr.clone()
            }
        }

        Expr::Unary(op, inner) => {
            Expr::Unary(op.clone(), Box::new(resolve_expr(inner, ctx)))
        }

        Expr::Binary(lhs, op, rhs) => {
            Expr::Binary(
                Box::new(resolve_expr(lhs, ctx)),
                op.clone(),
                Box::new(resolve_expr(rhs, ctx)),
            )
        }

        Expr::Call(func, args) => resolve_call(func, args, ctx),

        Expr::SysCall(name, args) => {
            let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
            Expr::SysCall(name.clone(), resolved)
        }

        Expr::If { cond, then_body, else_body } => {
            Expr::If {
                cond: Box::new(resolve_expr(cond, ctx)),
                then_body: resolve_block(then_body, ctx),
                else_body: resolve_block(else_body, ctx),
            }
        }

        Expr::Block(b) => Expr::Block(resolve_block(b, ctx)),

        Expr::Index(base, idx) => {
            Expr::Index(
                Box::new(resolve_expr(base, ctx)),
                Box::new(resolve_expr(idx, ctx)),
            )
        }

        Expr::Slice(base, range) => {
            Expr::Slice(
                Box::new(resolve_expr(base, ctx)),
                piperine_lang::parse::ast::Range {
                    start: Box::new(resolve_expr(&range.start, ctx)),
                    end: Box::new(resolve_expr(&range.end, ctx)),
                    inclusive: range.inclusive,
                },
            )
        }

        Expr::Field(base, field) => {
            let qualified = match base.as_ref() {
                Expr::Ident(base_name) => format!("{base_name}.{field}"),
                Expr::Index(inner, idx) => {
                    if let (Expr::Ident(base_name), Expr::Literal(Literal::Int(i))) =
                        (inner.as_ref(), idx.as_ref())
                    {
                        format!("{base_name}[{i}].{field}")
                    } else {
                        let base_name = expr_to_name(base);
                        format!("{base_name}_{field}")
                    }
                }
                _ => {
                    let base_name = expr_to_name(base);
                    format!("{base_name}_{field}")
                }
            };
            // Named instance port access (`load.p`): resolve to the
            // parent-scope node it's connected to, so the Builder can
            // find it by name in the SymbolTable.
            if let Some(node_id) = ctx.lookup_node(&qualified) {
                let node_name = ctx.symbols.node(node_id).name.clone();
                Expr::Ident(node_name)
            } else {
                Expr::Ident(qualified.replace('.', "_"))
            }
        }

        Expr::Array(body) => resolve_array(body, ctx),

        Expr::Cast(_target, inner) => resolve_expr(inner, ctx),

        Expr::BundleLit { ty, .. } => {
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "bundle literal in expression position",
                name: ty.name.clone(),
            });
            Expr::Literal(Literal::Real(0.0))
        }
        Expr::Lambda { .. } | Expr::Tuple(_) | Expr::MapLit(_) | Expr::SetLit(_) => {
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "value-layer expression (lambda/tuple/map/set)",
                name: expr_to_name(expr),
            });
            Expr::Literal(Literal::Real(0.0))
        }
    }
}

fn resolve_array(body: &ArrayBody, ctx: &mut LowerCtx) -> Expr {
    match body {
        ArrayBody::List(exprs) => {
            Expr::Array(ArrayBody::List(exprs.iter().map(|e| resolve_expr(e, ctx)).collect()))
        }
        ArrayBody::Repeat(v, n) => {
            Expr::Array(ArrayBody::Repeat(
                Box::new(resolve_expr(v, ctx)),
                Box::new(resolve_expr(n, ctx)),
            ))
        }
        ArrayBody::Comprehension(expr, var, range) => {
            if let (Some(start), Some(end)) = (
                eval_const_int(&range.start),
                eval_const_int(&range.end),
            ) {
                let inclusive = range.inclusive as i64;
                let mut elems = vec![];
                for i in start..(end + inclusive) {
                    let mut iter_ctx = LowerCtx::new(
                        ctx.symbols,
                        ctx.module_name.clone(),
                        ctx.is_digital,
                        ctx.module_vars.clone(),
                    );
                    iter_ctx.env = ctx.env.clone();
                    iter_ctx.enum_values = ctx.enum_values.clone();
                    iter_ctx.consts = ctx.consts.clone();
                    iter_ctx.env.insert(var.clone(), Expr::Literal(Literal::Int(i as u64)));
                    elems.push(resolve_expr(expr, &mut iter_ctx));
                    ctx.errors.append(&mut iter_ctx.errors);
                    ctx.digital_shadows.append(&mut iter_ctx.digital_shadows);
                }
                Expr::Array(ArrayBody::List(elems))
            } else {
                Expr::Array(ArrayBody::List(vec![]))
            }
        }
    }
}

pub(crate) fn eval_const_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int(n)) => Some(*n as i64),
        Expr::Literal(Literal::Bool(true)) => Some(1),
        Expr::Literal(Literal::Bool(false)) => Some(0),
        Expr::Literal(Literal::Real(f)) => Some(*f as i64),
        _ => None,
    }
}

pub(crate) fn expr_to_name(expr: &Expr) -> String {
    match expr {
        Expr::Ident(s) => s.clone(),
        Expr::Path(p) => p.segments.join("::"),
        Expr::Field(base, field) => format!("{}_{}", expr_to_name(base), field),
        _ => "_".into(),
    }
}

pub(crate) fn resolve_block(block: &Block, ctx: &mut LowerCtx) -> Block {
    let stmts: Vec<Stmt> = block.stmts.iter().map(|s| resolve_stmt(s, ctx)).collect();
    let expr = block.expr.as_ref().map(|e| Box::new(resolve_expr(e, ctx)));
    Block { stmts, expr }
}

/// Resolve a single statement's expressions (analog path — keeps POM `Stmt`).
pub(crate) fn resolve_stmt(stmt: &Stmt, ctx: &mut LowerCtx) -> Stmt {
    use piperine_lang::parse::ast::{BindOp, Stmt as S};
    match stmt {
        S::Bind { dest, op, src } => {
            let src = resolve_expr(src, ctx);
            if *op == BindOp::Contrib {
                scan_noise(&src, NodeId::GROUND, NodeId::GROUND, ctx);
            }
            S::Bind { dest: dest.clone(), op: op.clone(), src }
        }
        S::VarDecl { name, ty, default } => {
            let default = default.as_ref().map(|e| resolve_expr(e, ctx));
            S::VarDecl { name: name.clone(), ty: ty.clone(), default }
        }
        S::If { cond, then_body, else_body } => {
            S::If {
                cond: resolve_expr(cond, ctx),
                then_body: resolve_block(then_body, ctx),
                else_body: else_body.as_ref().map(|b| resolve_block(b, ctx)),
            }
        }
        S::Match { expr, arms } => {
            let expr = resolve_expr(expr, ctx);
            let arms = arms.iter().map(|arm| {
                piperine_lang::parse::ast::StmtMatchArm {
                    pat: arm.pat.clone(),
                    body: resolve_block(&arm.body, ctx),
                }
            }).collect();
            S::Match { expr, arms }
        }
        S::Event { spec, guard, body } => {
            let guard = guard.as_ref().map(|e| resolve_expr(e, ctx));
            let body = resolve_block(body, ctx);
            let spec = resolve_event_spec(spec, ctx);
            S::Event { spec, guard, body }
        }
        S::Expr(e) => S::Expr(resolve_expr(e, ctx)),
        S::Return(e) => S::Return(resolve_expr(e, ctx)),
        S::Diagnostic { sys, args } => {
            let args = args.iter().map(|a| resolve_expr(a, ctx)).collect();
            S::Diagnostic { sys: sys.clone(), args }
        }
        other => other.clone(),
    }
}

/// Resolve expressions inside an `EventSpec` (hoist analog operators, etc.).
fn resolve_event_spec(spec: &piperine_lang::parse::ast::EventSpec, ctx: &mut LowerCtx) -> piperine_lang::parse::ast::EventSpec {
    use piperine_lang::parse::ast::EventSpec;
    match spec {
        EventSpec::Named { name, args } => EventSpec::Named {
            name: name.clone(),
            args: args.iter().map(|a| resolve_expr(a, ctx)).collect(),
        },
        EventSpec::Initial => EventSpec::Initial,
        EventSpec::Final => EventSpec::Final,
        EventSpec::Or(specs) => EventSpec::Or(
            specs.iter().map(|s| resolve_event_spec(s, ctx)).collect(),
        ),
    }
}

/// Resolve an optional-typed receiver to `(presence param, value expr)`:
/// a plain optional param, a flattened optional bundle field, or a
/// `.map(|v| …)` chain over either (the lambda body substitutes the inner
/// value; presence is untouched — `none.map(f)` is still `none`).
fn optional_receiver(recv: &Expr, ctx: &mut LowerCtx) -> Option<(String, Expr)> {
    match recv {
        Expr::Ident(n) if ctx.params.contains_key(n) => Some((n.clone(), recv.clone())),
        Expr::Field(base, field) => match base.as_ref() {
            Expr::Ident(b) => {
                let flat = format!("{b}_{field}");
                ctx.params
                    .contains_key(&flat)
                    .then(|| (flat.clone(), Expr::Ident(flat)))
            }
            _ => None,
        },
        Expr::Call(f, args) => match f.as_ref() {
            Expr::Field(inner, method) if method == "map" && args.len() == 1 => {
                let Expr::Lambda { params, body } = &args[0] else { return None };
                if params.len() != 1 {
                    return None;
                }
                let (name, value) = optional_receiver(inner, ctx)?;
                let mut subst = std::collections::HashMap::new();
                subst.insert(params[0].clone(), value);
                Some((name, subst_expr(body, &subst)))
            }
            _ => None,
        },
        _ => None,
    }
}

fn resolve_call(func: &Expr, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
    // Method call: recv.method(args)
    if let Expr::Field(recv, method) = func {
        // Optional-param sugar (`T?`, SPEC Part I §6.1). The receiver is a
        // plain param (`rmodel`), a bundle field (`model.rbm` — flattened to
        // the synthetic param `model_rbm`), or a `.map(|v| …)` chain over
        // either. Aliases: `is_some`/`unwrap_or` are the prelude names for
        // the same fold.
        let optional_recv = optional_receiver(recv, ctx);
        // `x.get_or(default)` / `x.map(f).get_or(default)` →
        // `if $param_given("x") { value } else { default }`
        if (method == "get_or" || method == "unwrap_or")
            && args.len() == 1
            && let Some((name, value)) = optional_recv.clone()
        {
            use piperine_lang::parse::ast::{Block, Expr as E, Literal};
            let param_given = E::SysCall(
                "$param_given".into(),
                vec![E::Literal(Literal::String(name))],
            );
            let value = resolve_expr(&value, ctx);
            let default = resolve_expr(&args[0], ctx);
            return E::If {
                cond: Box::new(param_given),
                then_body: Block { stmts: vec![], expr: Some(Box::new(value)) },
                else_body: Block { stmts: vec![], expr: Some(Box::new(default)) },
            };
        }
        // `is_present()` on an optional (mapping never changes presence) →
        // `$param_given("name")`
        if (method == "is_present" || method == "is_some")
            && args.is_empty()
            && let Some((name, _)) = optional_recv
        {
            return Expr::SysCall(
                "$param_given".into(),
                vec![Expr::Literal(Literal::String(name))],
            );
        }

        // Bundle method call: receiver must be a bundle-typed binding.
        let recv_bundle = match recv.as_ref() {
            Expr::Ident(n) => ctx.bundle_bindings.get(n).cloned().map(|b| (n.clone(), b)),
            _ => None,
        };
        let Some((recv_name, (bundle, fields))) = recv_bundle else {
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "method call receiver (not a bundle-typed binding)",
                name: format!("{}.{method}(…)", expr_to_name(recv)),
            });
            return Expr::Literal(Literal::Real(0.0));
        };
        let mangled = format!("{bundle}::{method}");
        let Some(fn_id) = ctx.symbols.fn_by_name(&mangled) else {
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "impl method",
                name: mangled,
            });
            return Expr::Literal(Literal::Real(0.0));
        };
        // Expand receiver into field scalars, then append explicit args.
        let mut full_args: Vec<Expr> = fields
            .iter()
            .map(|f| resolve_expr(&Expr::Field(Box::new(Expr::Ident(recv_name.clone())), f.clone()), ctx))
            .collect();
        let sig = ctx.fn_bundle_sigs.get(&mangled).cloned();
        for (i, a) in args.iter().enumerate() {
            // Skip the `self` position (index 0) which we already filled.
            let sig_i = i + 1;
            match sig.as_ref().and_then(|s| s.get(sig_i)).and_then(|p| p.as_ref()) {
                Some(flds) => {
                    if !lower_bundle_arg(a, &flds.fields, &mut full_args, ctx) {
                        ctx.errors.push(super::LowerError {
                            module: ctx.module_name.clone(),
                            what: "bundle-typed argument",
                            name: format!("{} arg #{}", mangled, sig_i + 1),
                        });
                        return Expr::Literal(Literal::Real(0.0));
                    }
                }
                None => full_args.push(resolve_expr(a, ctx)),
            }
        }
        return inline_user_fn(fn_id, &mangled, &full_args, ctx, /*already_resolved=*/ true);
    }

    let name = match func {
        Expr::Ident(s) => s.as_str(),
        _ => return Expr::Literal(Literal::Real(0.0)),
    };

    if name == "V" || name == "I" {
        let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
        return Expr::Call(Box::new(func.clone()), resolved);
    }

    if let Some(op) = analog_ops().lookup(name) {
        return op.lower(args, ctx);
    }

    if crate::lower::math::math_fn(name).is_some() {
        let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
        return Expr::Call(Box::new(func.clone()), resolved);
    }

    if let Some(fn_id) = ctx.symbols.fn_by_name(name) {
        return inline_user_fn(fn_id, name, args, ctx, false);
    }

    let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
    Expr::Call(Box::new(func.clone()), resolved)
}

/// Expand a bundle-valued call argument into per-field scalars.
fn lower_bundle_arg(
    arg: &Expr,
    fields: &[String],
    out: &mut Vec<Expr>,
    ctx: &mut LowerCtx,
) -> bool {
    match arg {
        Expr::Ident(n) if ctx.bundle_bindings.contains_key(n) => {
            for f in fields {
                let e = Expr::Field(Box::new(Expr::Ident(n.clone())), f.clone());
                out.push(resolve_expr(&e, ctx));
            }
            true
        }
        Expr::BundleLit { fields: lit_fields, .. } => {
            for f in fields {
                match lit_fields.iter().find(|(n, _)| n == f) {
                    Some((_, e)) => out.push(resolve_expr(e, ctx)),
                    None => {
                        ctx.errors.push(super::LowerError {
                            module: ctx.module_name.clone(),
                            what: "bundle literal missing a field",
                            name: f.clone(),
                        });
                        out.push(Expr::Literal(Literal::Real(0.0)));
                    }
                }
            }
            true
        }
        _ => false,
    }
}

/// Inline a user function call by substituting parameters. The function body
/// is POM `Stmt`; we walk it and substitute `Ident(param_name)` with the
/// corresponding argument expression.
fn inline_user_fn(fn_id: FnId, name: &str, args: &[Expr], ctx: &mut LowerCtx, already_resolved: bool) -> Expr {
    let function = match ctx.symbols.try_fn(fn_id) {
        Some(f) => f.clone(),
        None => {
            ctx.errors.push(super::LowerError {
                module: ctx.module_name.clone(),
                what: "function",
                name: name.to_string(),
            });
            return Expr::Literal(Literal::Real(0.0));
        }
    };

    // Expand bundle-typed arguments into per-field scalars (unless already done).
    let mut full_args: Vec<Expr> = if already_resolved {
        args.to_vec()
    } else {
        let sig = ctx.fn_bundle_sigs.get(name).cloned();
        let mut collected: Vec<Expr> = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            match sig.as_ref().and_then(|s| s.get(i)).and_then(|p| p.as_ref()) {
                Some(flds) => {
                    if !lower_bundle_arg(arg, &flds.fields, &mut collected, ctx) {
                        ctx.errors.push(super::LowerError {
                            module: ctx.module_name.clone(),
                            what: "bundle-typed argument",
                            name: format!("{} arg #{}", function.name, i + 1),
                        });
                        return Expr::Literal(Literal::Real(0.0));
                    }
                }
                None => collected.push(resolve_expr(arg, ctx)),
            }
        }
        collected
    };
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
                return Expr::Literal(Literal::Real(0.0));
            }
        }
    }

    let mut subst: std::collections::HashMap<String, Expr> = std::collections::HashMap::new();
    for (&param_id, arg) in function.params.iter().zip(full_args) {
        let pname = ctx.symbols.var(param_id).name.clone();
        subst.insert(pname, arg);
    }

    let body = function.body.clone();
    inline_fn_body(&body, &mut subst, ctx)
}

fn inline_fn_body(stmts: &[Stmt], subst: &mut std::collections::HashMap<String, Expr>, ctx: &mut LowerCtx) -> Expr {
    for s in stmts {
        use piperine_lang::parse::ast::Stmt as S;
        match s {
            S::Return(e) => {
                return subst_expr(e, subst);
            }
            S::Bind { dest, op: _, src } if matches!(dest, Expr::Ident(_)) => {
                if let Expr::Ident(name) = dest {
                    let val = subst_expr(src, subst);
                    subst.insert(name.clone(), val);
                }
            }
            S::VarDecl { name, default: Some(e), .. } => {
                let val = subst_expr(e, subst);
                subst.insert(name.clone(), val);
            }
            S::VarDecl { name, default: None, .. } => {
                subst.insert(name.clone(), Expr::Literal(Literal::Real(0.0)));
            }
            _ => {}
        }
    }
    let _ = ctx;
    Expr::Literal(Literal::Real(0.0))
}

fn subst_expr(expr: &Expr, subst: &std::collections::HashMap<String, Expr>) -> Expr {
    match expr {
        Expr::Ident(name) => {
            subst.get(name).cloned().unwrap_or_else(|| expr.clone())
        }
        Expr::Unary(op, x) => Expr::Unary(op.clone(), Box::new(subst_expr(x, subst))),
        Expr::Binary(l, op, r) => Expr::Binary(
            Box::new(subst_expr(l, subst)),
            op.clone(),
            Box::new(subst_expr(r, subst)),
        ),
        Expr::Call(f, args) => {
            let f = subst_expr(f, subst);
            let args: Vec<Expr> = args.iter().map(|a| subst_expr(a, subst)).collect();
            Expr::Call(Box::new(f), args)
        }
        Expr::SysCall(name, args) => {
            let args: Vec<Expr> = args.iter().map(|a| subst_expr(a, subst)).collect();
            Expr::SysCall(name.clone(), args)
        }
        Expr::If { cond, then_body, else_body } => Expr::If {
            cond: Box::new(subst_expr(cond, subst)),
            then_body: subst_block(then_body, subst),
            else_body: subst_block(else_body, subst),
        },
        Expr::Block(b) => Expr::Block(subst_block(b, subst)),
        Expr::Cast(t, x) => Expr::Cast(t.clone(), Box::new(subst_expr(x, subst))),
        Expr::Field(base, field) => {
            let base = subst_expr(base, subst);
            Expr::Field(Box::new(base), field.clone())
        }
        Expr::Index(base, idx) => Expr::Index(
            Box::new(subst_expr(base, subst)),
            Box::new(subst_expr(idx, subst)),
        ),
        _ => expr.clone(),
    }
}

fn subst_block(block: &Block, subst: &std::collections::HashMap<String, Expr>) -> Block {
    Block {
        stmts: block.stmts.iter().map(|s| subst_stmt(s, subst)).collect(),
        expr: block.expr.as_ref().map(|e| Box::new(subst_expr(e, subst))),
    }
}

fn subst_stmt(stmt: &Stmt, subst: &std::collections::HashMap<String, Expr>) -> Stmt {
    use piperine_lang::parse::ast::Stmt as S;
    match stmt {
        S::Bind { dest, op, src } => S::Bind {
            dest: subst_expr(dest, subst),
            op: op.clone(),
            src: subst_expr(src, subst),
        },
        S::VarDecl { name, ty, default } => S::VarDecl {
            name: name.clone(),
            ty: ty.clone(),
            default: default.as_ref().map(|e| subst_expr(e, subst)),
        },
        S::If { cond, then_body, else_body } => S::If {
            cond: subst_expr(cond, subst),
            then_body: subst_block(then_body, subst),
            else_body: else_body.as_ref().map(|b| subst_block(b, subst)),
        },
        S::Expr(e) => S::Expr(subst_expr(e, subst)),
        S::Return(e) => S::Return(subst_expr(e, subst)),
        other => other.clone(),
    }
}
