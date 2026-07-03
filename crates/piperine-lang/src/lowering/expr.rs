//! Expression lowering: `Expr` → `IrExpr`

use crate::parse::ast::{ArrayBody, BindOp, BinaryOp, Block, Expr, Literal, Stmt, UnaryOp};
use piperine_codegen::ir::*;
use super::analog_ops::analog_ops;
use super::syscalls::syscalls;
use super::LowerCtx;

pub(crate) fn parse_contrib_dest(dest: &Expr, ctx: &mut LowerCtx) -> (NatureId, NodeId, NodeId) {
    if let Expr::Call(func, args) = dest {
        if let Expr::Ident(name) = func.as_ref() {
            let nature_kind = match name.as_str() {
                "V" => NatureKind::Potential,
                "I" => NatureKind::Flow,
                _ => NatureKind::Flow,
            };
            let nature = ctx.symbols.add_nature(name.as_str(), nature_kind);
            
            let plus_name = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
            let minus_name = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());
            
            let plus = ctx.lookup_node(&plus_name).unwrap_or(NodeId::GROUND);
            let minus = ctx.lookup_node(&minus_name).unwrap_or(NodeId::GROUND);
            
            return (nature, plus, minus);
        }
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
                    if let Expr::Ident(base_name) = inner.as_ref() {
                        if let Expr::Literal(Literal::Int(i)) = idx.as_ref() {
                            return Some(format!("{base_name}_{i}.{field}"));
                        }
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
    match expr {
        Expr::Call(func, args) => {
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
                        return;
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
                        return;
                    }
                    _ => {}
                }
            }
            for arg in args {
                scan_noise(arg, plus, minus, ctx);
            }
        }
        Expr::Binary(l, _, r) => {
            scan_noise(l, plus, minus, ctx);
            scan_noise(r, plus, minus, ctx);
        }
        Expr::Unary(_, inner) => scan_noise(inner, plus, minus, ctx),
        Expr::If { cond, then_body, else_body } => {
            scan_noise_expr_block(cond, plus, minus, ctx);
            scan_noise_block(then_body, plus, minus, ctx);
            scan_noise_block(else_body, plus, minus, ctx);
        }
        _ => {}
    }
}

pub(crate) fn scan_noise_block(block: &Block, plus: NodeId, minus: NodeId, ctx: &mut LowerCtx) {
    for s in &block.stmts {
        if let Stmt::Bind { op: BindOp::Contrib, src, .. } = s {
            scan_noise(src, plus, minus, ctx);
        }
    }
}

pub(crate) fn scan_noise_expr_block(expr: &Expr, plus: NodeId, minus: NodeId, ctx: &mut LowerCtx) {
    scan_noise(expr, plus, minus, ctx);
}

pub(crate) fn lower_expr(expr: &Expr, ctx: &mut LowerCtx) -> IrExpr {
    match expr {
        Expr::Literal(Literal::Real(f)) => IrExpr::Real(*f),
        Expr::Literal(Literal::Int(n)) => IrExpr::Int(*n as i64),
        Expr::Literal(Literal::Bool(b)) => IrExpr::Bool(*b),
        Expr::Literal(Literal::String(_s)) => IrExpr::Real(0.0), // No strings in new IR
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
                let id = ctx.lookup_var(name).unwrap_or(VarId(0));
                IrExpr::Var(id)
            } else if let Some(id) = ctx.lookup_node(name) {
                if ctx.is_digital { IrExpr::Net(id) } else { IrExpr::Real(0.0) } // Just a fallback for non-digital context
            } else if let Some(value) = ctx.lookup_enum_value(name) {
                IrExpr::Int(value)
            } else {
                let id = ctx.lookup_param(name).unwrap_or(ParamId(0));
                IrExpr::Param(id)
            }
        }

        Expr::Path(p) => {
            let name = p.segments.join("::");
            if let Some(val) = ctx.env.get(&name) {
                val.clone()
            } else if let Some(value) = ctx.lookup_enum_value(&name) {
                IrExpr::Int(value)
            } else {
                let id = ctx.lookup_param(&name).unwrap_or(ParamId(0));
                IrExpr::Param(id)
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
            } else {
                let id = ctx.lookup_param(&qualified).unwrap_or(ParamId(0));
                IrExpr::Param(id)
            }
        }

        Expr::Array(body) => lower_array(body, ctx),

        Expr::Cast(_target, inner) => lower_expr(inner, ctx),
        Expr::BundleLit { .. } | Expr::Lambda { .. } => IrExpr::Real(0.0),
    }
}

pub(crate) fn lower_array(body: &ArrayBody, ctx: &mut LowerCtx) -> IrExpr {
    match body {
        ArrayBody::List(exprs) => {
            IrExpr::Array(exprs.iter().map(|e| lower_expr(e, ctx)).collect())
        }
        ArrayBody::Repeat(v, n) => {
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
                    let mut iter_ctx = LowerCtx::new(ctx.symbols, ctx.is_digital, ctx.module_vars.clone());
                    iter_ctx.env = ctx.env.clone();
                    iter_ctx.enum_values = ctx.enum_values.clone();
                    iter_ctx.env.insert(var.clone(), IrExpr::Int(i));
                    elems.push(lower_expr(expr, &mut iter_ctx));
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
    let name = match func {
        Expr::Ident(s) => s.as_str(),
        _ => return IrExpr::Real(0.0),
    };

    if name == "V" || name == "I" {
        return if args.len() >= 2 {
            let plus_name = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
            let minus_name = ident_from_expr(Some(&args[1])).unwrap_or_else(|| "0".into());
            let plus = ctx.lookup_node(&plus_name).unwrap_or(NodeId::GROUND);
            let minus = ctx.lookup_node(&minus_name).unwrap_or(NodeId::GROUND);
            let nature = ctx.symbols.add_nature(name, NatureKind::Potential);
            IrExpr::Branch { nature, plus, minus }
        } else if args.len() == 1 {
            let plus_name = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
            let plus = ctx.lookup_node(&plus_name).unwrap_or(NodeId::GROUND);
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
    if piperine_codegen::jit::math::math_fn(name).is_some() {
        let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
        return IrExpr::MathCall(name.to_string(), ir_args);
    }

    // User function: look up by name in the symbol table.
    if let Some(fn_id) = ctx.symbols.fn_by_name(name) {
        let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
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
