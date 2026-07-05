//! Expression lowering: `Expr` → `IrExpr`

use crate::parse::ast::{ArrayBody, BinaryOp, Block, Expr, Literal, Stmt, UnaryOp};
use piperine_ir::*;
use super::analog_ops::analog_ops;
use super::syscalls::syscalls;
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
            // `V(a)` / `I(a)` — an omitted second terminal is ground.
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
        // SPEC §7.3: `name.port` — a named instance's port, resolved by the
        // parent's analog body. We return the qualified string so the caller
        // can look it up in `ctx.instance_ports` or `ctx.lookup_node`.
        Expr::Field(base, field) => {
            match base.as_ref() {
                Expr::Ident(base_name) => Some(format!("{base_name}.{field}")),
                // `name[i].port` after for-unroll becomes `name[0].port` etc.
                Expr::Index(inner, idx) => {
                    if let Expr::Ident(base_name) = inner.as_ref()
                        && let Expr::Literal(Literal::Int(i)) = idx.as_ref() {
                            return Some(format!("{base_name}_{i}.{field}"));
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
    use crate::parse::ast::Walk;
    expr.walk(&mut |e| {
        if let Expr::Call(func, args) = e {
            if let Expr::Ident(name) = func.as_ref() {
                match name.as_str() {
                    "white_noise" => {
                        let psd = args.first()
                            .map(|a| lower_expr(a, ctx))
                            .unwrap_or(IrExpr::Real(0.0));
                        let label = args.get(1).and_then(|a| {
                            if let Expr::Literal(Literal::String(s)) = a { Some(s.clone()) } else { None }
                        });
                        ctx.noise_sources.push(IrNoiseSource {
                            plus,
                            minus,
                            kind: IrNoise::White { psd },
                            label,
                        });
                        return Walk::SkipChildren;
                    }
                    "flicker_noise" => {
                        let psd = args.first()
                            .map(|a| lower_expr(a, ctx))
                            .unwrap_or(IrExpr::Real(0.0));
                        let exponent = args.get(1)
                            .map(|a| lower_expr(a, ctx))
                            .unwrap_or(IrExpr::Real(1.0));
                        let label = args.get(2).and_then(|a| {
                            if let Expr::Literal(Literal::String(s)) = a { Some(s.clone()) } else { None }
                        });
                        ctx.noise_sources.push(IrNoiseSource {
                            plus,
                            minus,
                            kind: IrNoise::Flicker { psd, exponent },
                            label,
                        });
                        return Walk::SkipChildren;
                    }
                    _ => {}
                }
            }
        }
        Walk::Continue
    });
}

pub(crate) fn lower_expr(expr: &Expr, ctx: &mut LowerCtx) -> IrExpr {
    match expr {
        Expr::Literal(Literal::Real(f)) => IrExpr::Real(*f),
        Expr::Literal(Literal::Int(n)) => IrExpr::Int(*n as i64),
        Expr::Literal(Literal::Bool(b)) => IrExpr::Bool(*b),
        Expr::Literal(Literal::String(_s)) => IrExpr::Real(0.0), // No strings in new IR
        // `none` is an elaboration-time absent value: reading an optional in a
        // runtime (IR) context must go through `.get_or(default)` (which folds
        // to the default here) or a `.is_present()` guard. A bare `none`
        // reaching lowering is a real error, not a silent 0.0.
        Expr::Literal(Literal::None) => {
            ctx.errors.push(crate::lowering::LowerError {
                module: ctx.module_name.clone(),
                what: "`none` in a runtime context — read the optional via .get_or(default)",
                name: "none".into(),
            });
            IrExpr::Real(0.0)
        }
        Expr::Literal(Literal::Quad(s)) => {
            let val = match s.trim_start_matches("0q") {
                "0" | "" => 0u8,
                "1" => 1,
                "X" | "x" => 2,
                "Z" | "z" => 3,
                _ => 0,
            };
            IrExpr::Quad(val)
        }

        Expr::Ident(name) => {
            if let Some(val) = ctx.env.get(name) {
                val.clone()
            } else if ctx.module_vars.contains(name) {
                IrExpr::Var(ctx.require_var(name))
            } else if let Some(id) = ctx.lookup_node(name) {
                if ctx.is_digital {
                    IrExpr::Net(id)
                } else if ctx.symbols.node(id).domain == piperine_ir::Domain::Digital {
                    // An analog body reading a digital-domain node by bare
                    // name (not through `V`/`I`) bridges through a shadow
                    // var — the same D2A path a `var` read already uses.
                    // Never a silent 0.0 (GAPS: fail loud, not a stub).
                    IrExpr::Var(ctx.shadow_var_for(id, name))
                } else {
                    // A bare analog-domain node reference outside `V`/`I`
                    // has no defined meaning (SPEC: node access is via
                    // V/I only) — GAPS fallback, unchanged.
                    IrExpr::Real(0.0)
                }
            } else if let Some(value) = ctx.lookup_enum_value(name) {
                IrExpr::Int(value)
            } else if let Some(c) = ctx.consts.get(name) {
                c.clone()
            } else {
                // Last namespace standing: a parameter — or an unresolved
                // name, which is an error, not a silent `ParamId(0)`.
                IrExpr::Param(ctx.require_ident_as_param(name))
            }
        }

        Expr::Path(p) => {
            let name = p.segments.join("::");
            if let Some(val) = ctx.env.get(&name) {
                val.clone()
            } else if let Some(value) = ctx.lookup_enum_value(&name) {
                IrExpr::Int(value)
            } else {
                IrExpr::Param(ctx.require_ident_as_param(&name))
            }
        }

        Expr::Unary(UnaryOp::Neg, inner) => {
            IrExpr::Unary(IrUnOp::Neg, Box::new(lower_expr(inner, ctx)))
        }
        Expr::Unary(UnaryOp::Not, inner) => {
            IrExpr::Unary(IrUnOp::Not, Box::new(lower_expr(inner, ctx)))
        }

        Expr::Binary(lhs, op, rhs) => {
            let l = Box::new(lower_expr(lhs, ctx));
            let r = Box::new(lower_expr(rhs, ctx));
            IrExpr::Binary(lower_binop(op), l, r)
        }

        Expr::Call(func, args) => lower_call(func, args, ctx),

        Expr::SysCall(name, args) => lower_syscall(name, args, ctx),

        Expr::If { cond, then_body, else_body } => {
            let c = Box::new(lower_expr(cond, ctx));
            let t = Box::new(block_value(then_body, ctx));
            let e = Box::new(block_value(else_body, ctx));
            IrExpr::Select(c, t, e)
        }

        Expr::Block(b) => block_value(b, ctx),

        Expr::Index(base, idx) => {
            IrExpr::Index(
                Box::new(lower_expr(base, ctx)),
                Box::new(lower_expr(idx, ctx)),
            )
        }

        Expr::Slice(base, range) => {
            IrExpr::Slice(
                Box::new(lower_expr(base, ctx)),
                Box::new(lower_expr(&range.start, ctx)),
                Box::new(lower_expr(&range.end, ctx)),
                range.inclusive,
            )
        }

        Expr::Field(base, field) => {
            let qualified = match base.as_ref() {
                Expr::Ident(base_name) => format!("{base_name}.{field}"),
                Expr::Index(inner, idx) => {
                    if let (Expr::Ident(base_name), Expr::Literal(Literal::Int(i))) =
                        (inner.as_ref(), idx.as_ref())
                    {
                        format!("{base_name}_{i}.{field}")
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
            // SPEC §7.3: `name.port` resolves to the parent-scope node
            // that the named instance's port is connected to.
            if let Some(id) = ctx.lookup_node(&qualified) {
                if ctx.is_digital { IrExpr::Net(id) } else { IrExpr::Real(0.0) }
            } else if let Some(id) = ctx.lookup_param(&qualified.replace('.', "_")) {
                // A bundle-typed param field (`model.rsh`) was flattened to
                // a scalar param `model_rsh` at elaboration (GAPS §I.14).
                IrExpr::Param(id)
            } else if let Some(id) = ctx.lookup_var(&qualified.replace('.', "_")) {
                // A bundle-typed *fn* param field (`m.rsh` inside
                // `fn f(m: ResModel)`) — flattened to a var by `convert_fn`.
                IrExpr::Var(id)
            } else {
                IrExpr::Param(ctx.require_ident_as_param(&qualified))
            }
        }

        Expr::Array(body) => lower_array(body, ctx),

        Expr::Cast(_target, inner) => lower_expr(inner, ctx),
        // Value-layer-only constructs: no faithful scalar lowering exists,
        // so reaching one in an analog/digital body is a loud error, never
        // a silent 0.0 (SPEC §11).
        Expr::BundleLit { ty, .. } => {
            ctx.errors.push(crate::lowering::LowerError {
                module: ctx.module_name.clone(),
                what: "bundle literal in expression position",
                name: ty.name.clone(),
            });
            IrExpr::Real(0.0)
        }
        Expr::Lambda { .. } | Expr::Tuple(_) | Expr::MapLit(_) => {
            ctx.errors.push(crate::lowering::LowerError {
                module: ctx.module_name.clone(),
                what: "value-layer expression (lambda/tuple/map)",
                name: expr_to_name(expr),
            });
            IrExpr::Real(0.0)
        }
    }
}

/// Expand a bundle-valued call argument into per-field scalars, appended
/// to `out` in field order. Supported shapes: a bundle-typed binding
/// (`model`, expanding through the flattened params/vars) and a bundle
/// literal (`ResModel { .rsh = 2e3 }`, omitted fields taking the bundle's
/// declared defaults through the binding-independent literal fields only —
/// a literal missing a field with no default fails loud at the field
/// lookup). Returns `false` for anything else.
fn lower_bundle_arg(
    arg: &Expr,
    _bundle: &str,
    fields: &[String],
    out: &mut Vec<IrExpr>,
    ctx: &mut LowerCtx,
) -> bool {
    match arg {
        Expr::Ident(n) if ctx.bundle_bindings.contains_key(n) => {
            for f in fields {
                let e = Expr::Field(Box::new(Expr::Ident(n.clone())), f.clone());
                out.push(lower_expr(&e, ctx));
            }
            true
        }
        Expr::BundleLit { fields: lit_fields, .. } => {
            for f in fields {
                match lit_fields.iter().find(|(n, _)| n == f) {
                    Some((_, e)) => out.push(lower_expr(e, ctx)),
                    None => {
                        ctx.errors.push(crate::lowering::LowerError {
                            module: ctx.module_name.clone(),
                            what: "bundle literal missing a field (no default expansion here yet)",
                            name: f.clone(),
                        });
                        out.push(IrExpr::Real(0.0));
                    }
                }
            }
            true
        }
        _ => false,
    }
}

pub(crate) fn lower_array(body: &ArrayBody, ctx: &mut LowerCtx) -> IrExpr {
    match body {
        ArrayBody::List(exprs) => {
            IrExpr::Array(exprs.iter().map(|e| lower_expr(e, ctx)).collect())
        }
        ArrayBody::Repeat(v, _n) => {
            IrExpr::Array(vec![lower_expr(v, ctx)]) // ArrayRepeat removed
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
                    iter_ctx.env.insert(var.clone(), IrExpr::Int(i));
                    elems.push(lower_expr(expr, &mut iter_ctx));
                    ctx.errors.append(&mut iter_ctx.errors);
                    ctx.digital_shadows.append(&mut iter_ctx.digital_shadows);
                }
                IrExpr::Array(elems)
            } else {
                IrExpr::Array(vec![])
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

pub(crate) fn block_value(block: &Block, ctx: &mut LowerCtx) -> IrExpr {
    for s in &block.stmts {
        if let Stmt::VarDecl { name, default: Some(expr), .. } = s {
            let val = lower_expr(expr, ctx);
            ctx.env.insert(name.clone(), val);
        }
    }
    if let Some(e) = &block.expr {
        return lower_expr(e, ctx);
    }
    for s in block.stmts.iter().rev() {
        if let Stmt::Expr(e) = s {
            return lower_expr(e, ctx);
        }
    }
    IrExpr::Real(0.0)
}

pub(crate) fn lower_call(func: &Expr, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
    // Optional-param sugar: `p.is_present()` / `p.get_or(default)` on a scalar
    // parameter `p : T?` map onto the parameter-presence mechanism —
    // `is_present` ≡ `$param_given(p)`, `get_or(d)` ≡ `param_given ? p : d`.
    // This works per-instance without specializing the module (an absent
    // optional's value slot is never read; the select masks it).
    if let Expr::Field(recv, method) = func
        && let Expr::Ident(pname) = recv.as_ref()
        && ctx.lookup_param(pname).is_some()
        && matches!(method.as_str(), "is_present" | "is_some" | "is_none" | "get_or" | "unwrap_or" | "unwrap")
    {
        let given_id = ctx.require_param_given(pname);
        let given = IrExpr::Sim(piperine_ir::SimQuery::ParamGiven(given_id));
        let value = IrExpr::Param(ctx.require_ident_as_param(pname));
        return match method.as_str() {
            "is_present" | "is_some" => given,
            "is_none" => IrExpr::Unary(piperine_ir::IrUnOp::Not, Box::new(given)),
            "get_or" | "unwrap_or" => {
                let default = args.first().map_or(IrExpr::Real(0.0), |a| lower_expr(a, ctx));
                IrExpr::Select(Box::new(given), Box::new(value), Box::new(default))
            }
            // `unwrap` assumes presence (no runtime check available in IR).
            _ => value,
        };
    }
    // `recv.method(args)` — an impl-method call. The receiver must be a
    // bundle-typed binding (module param or fn param); the method was
    // registered as the IR fn `Bundle::method` with `self` flattened
    // per-field, so the call expands `recv` into its field scalars.
    if let Expr::Field(recv, method) = func {
        let recv_bundle = match recv.as_ref() {
            Expr::Ident(n) => ctx.bundle_bindings.get(n).cloned().map(|b| (n.clone(), b)),
            _ => None,
        };
        let Some((recv_name, (bundle, fields))) = recv_bundle else {
            ctx.errors.push(crate::lowering::LowerError {
                module: ctx.module_name.clone(),
                what: "method call receiver (not a bundle-typed binding)",
                name: format!("{}.{method}(…)", expr_to_name(recv)),
            });
            return IrExpr::Real(0.0);
        };
        let mangled = format!("{bundle}::{method}");
        let Some(fn_id) = ctx.symbols.fn_by_name(&mangled) else {
            ctx.errors.push(crate::lowering::LowerError {
                module: ctx.module_name.clone(),
                what: "impl method",
                name: mangled,
            });
            return IrExpr::Real(0.0);
        };
        let mut ir_args: Vec<IrExpr> = fields
            .iter()
            .map(|f| lower_expr(&Expr::Field(Box::new(Expr::Ident(recv_name.clone())), f.clone()), ctx))
            .collect();
        for a in args {
            ir_args.push(lower_expr(a, ctx));
        }
        return IrExpr::Call(fn_id, ir_args);
    }

    let name = match func {
        Expr::Ident(s) => s.as_str(),
        _ => {
            ctx.errors.push(crate::lowering::LowerError {
                module: ctx.module_name.clone(),
                what: "call target (not a plain fn or method name)",
                name: expr_to_name(func),
            });
            return IrExpr::Real(0.0);
        }
    };

    if name == "V" || name == "I" {
        return if args.len() >= 2 {
            let plus_name = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
            let minus_name = ident_from_expr(Some(&args[1])).unwrap_or_else(|| "0".into());
            let plus = ctx.require_node(&plus_name);
            let minus = ctx.require_node(&minus_name);
            let nature = ctx.symbols.add_nature(name, NatureKind::Potential);
            IrExpr::Branch { nature, plus, minus }
        } else if args.len() == 1 {
            let plus_name = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
            let plus = ctx.require_node(&plus_name);
            let nature = ctx.symbols.add_nature(name, NatureKind::Potential);
            IrExpr::Branch { nature, plus, minus: NodeId::GROUND }
        } else {
            let nature = ctx.symbols.add_nature(name, NatureKind::Potential);
            IrExpr::Branch { nature, plus: NodeId::GROUND, minus: NodeId::GROUND }
        };
    }

    if let Some(op) = analog_ops().lookup(name) {
        return op.lower(args, ctx);
    }

    // Built-in math functions (exp, ln, sqrt, pow, sin, …) lower as
    // `MathCall`, resolved by name to a libm intrinsic at JIT time.
    if piperine_ir::math::math_fn(name).is_some() {
        let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
        return IrExpr::MathCall(name.to_string(), ir_args);
    }

    // User function: look up by name in the symbol table. Bundle-typed
    // parameter positions expand their argument into per-field scalars
    // (matching `convert_fn`'s flattened signature).
    if let Some(fn_id) = ctx.symbols.fn_by_name(name) {
        let sig = ctx.fn_bundle_sigs.get(name).cloned();
        let mut ir_args: Vec<IrExpr> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            match sig.as_ref().and_then(|s| s.get(i)).and_then(|p| p.as_ref()) {
                Some((bundle, fields)) => {
                    if !lower_bundle_arg(a, bundle, fields, &mut ir_args, ctx) {
                        ctx.errors.push(crate::lowering::LowerError {
                            module: ctx.module_name.clone(),
                            what: "bundle-typed argument (pass a bundle binding or literal)",
                            name: format!("{name}(… arg #{} …)", i + 1),
                        });
                        return IrExpr::Real(0.0);
                    }
                }
                None => ir_args.push(lower_expr(a, ctx)),
            }
        }
        return IrExpr::Call(fn_id, ir_args);
    }

    // Unknown function: emit as `MathCall` so the JIT produces a
    // descriptive error containing the function name (fail-loud, SPEC §11).
    let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
    IrExpr::MathCall(name.to_string(), ir_args)
}

pub(crate) fn lower_syscall(name: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
    let key = name.trim_start_matches('$').to_lowercase();
    if let Some(f) = syscalls().lookup(&key) {
        return f.lower(&key, args, ctx);
    }
    let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
    IrExpr::MathCall(format!("${key}"), ir_args)
}

pub(crate) fn lower_binop(op: &BinaryOp) -> IrBinOp {
    match op {
        BinaryOp::Add => IrBinOp::Add,
        BinaryOp::Sub => IrBinOp::Sub,
        BinaryOp::Mul => IrBinOp::Mul,
        BinaryOp::Div => IrBinOp::Div,
        BinaryOp::Rem => IrBinOp::Rem,
        BinaryOp::Eq => IrBinOp::Eq,
        BinaryOp::Neq => IrBinOp::Ne,
        BinaryOp::Lt => IrBinOp::Lt,
        BinaryOp::Le => IrBinOp::Le,
        BinaryOp::Gt => IrBinOp::Gt,
        BinaryOp::Ge => IrBinOp::Ge,
        BinaryOp::BitAnd => IrBinOp::BitAnd,
        BinaryOp::BitOr => IrBinOp::BitOr,
        BinaryOp::BitXor => IrBinOp::BitXor,
        BinaryOp::And => IrBinOp::And,
        BinaryOp::Or => IrBinOp::Or,
    }
}
