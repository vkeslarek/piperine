//! Expression lowering: `Expr` → `IrExpr`, analog-operator calls
//! (`ddt`, `idt`, `V`/`I` access, ...), array literals, and the noise-source
//! scan that walks a contribution's RHS for `$rdisturb`/`white_noise`/etc.

use crate::parse::ast::{ArrayBody, BindOp, BinaryOp, Block, Expr, Literal, Stmt, UnaryOp};

use piperine_codegen::ir::*;

use super::LowerCtx;

// ─── Destination parsing ──────────────────────────────────────────────────────

/// Parse a contribution destination (`V(n1,n2)` or `I(n1,n2)`) into its
/// nature, plus-terminal name, and minus-terminal name.
pub(crate) fn parse_contrib_dest(dest: &Expr) -> (IrNature, String, String) {
    if let Expr::Call(func, args) = dest {
        if let Expr::Ident(name) = func.as_ref() {
            let nature = access_to_nature(name);
            let plus = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
            let minus = ident_from_expr(args.get(1)).unwrap_or_else(|| "?".into());
            return (nature, plus, minus);
        }
    }
    (IrNature::Flow("I".into()), "?".into(), "?".into())
}

/// Map an access function name to its nature (potential or flow).
pub(crate) fn access_to_nature(name: &str) -> IrNature {
    match name {
        "V" => IrNature::Potential("V".into()),
        "I" => IrNature::Flow("I".into()),
        _ => IrNature::Flow(name.into()),
    }
}

/// Extract an identifier string from an expression, returning `None` if it
/// is not a bare `Expr::Ident`.
pub(crate) fn ident_from_expr(e: Option<&Expr>) -> Option<String> {
    match e? {
        Expr::Ident(s) => Some(s.clone()),
        _ => None,
    }
}

// ─── Noise extraction ──────────────────────────────────────────────────────────

/// Walk `expr` recursively and extract any `white_noise` / `flicker_noise`
/// calls into `ctx.noise_sources`.
pub(crate) fn scan_noise(expr: &Expr, plus: &str, minus: &str, ctx: &mut LowerCtx) {
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
                            plus: plus.into(),
                            minus: minus.into(),
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
                            plus: plus.into(),
                            minus: minus.into(),
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

/// Scan every contribution statement in `block` for noise-source calls.
pub(crate) fn scan_noise_block(block: &Block, plus: &str, minus: &str, ctx: &mut LowerCtx) {
    for s in &block.stmts {
        if let Stmt::Bind { op: BindOp::Contrib, src, .. } = s {
            scan_noise(src, plus, minus, ctx);
        }
    }
}

/// Scans a single expression (e.g. an if-condition) for noise-source calls.
pub(crate) fn scan_noise_expr_block(expr: &Expr, plus: &str, minus: &str, ctx: &mut LowerCtx) {
    scan_noise(expr, plus, minus, ctx);
}

// ─── Expression lowering ──────────────────────────────────────────────────────

/// Lower a PPHL expression into an [`IrExpr`], translating literals,
/// operators, calls, and accessor functions.
pub(crate) fn lower_expr(expr: &Expr, ctx: &mut LowerCtx) -> IrExpr {
    match expr {
        Expr::Literal(Literal::Real(f)) => IrExpr::Real(*f),
        Expr::Literal(Literal::Int(n)) => IrExpr::Int(*n as i64),
        Expr::Literal(Literal::Bool(b)) => IrExpr::Bool(*b),
        Expr::Literal(Literal::String(s)) => IrExpr::String(s.clone()),
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
            } else {
                IrExpr::Param(name.clone())
            }
        }

        Expr::Path(p) => {
            let name = p.segments.join("::");
            if let Some(val) = ctx.env.get(&name) {
                val.clone()
            } else {
                IrExpr::Param(name)
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
                Box::new(IrRange {
                    start: lower_expr(&range.start, ctx),
                    end: lower_expr(&range.end, ctx),
                    inclusive: range.inclusive,
                }),
            )
        }

        Expr::Field(base, field) => {
            // Flatten bundle field access: a.field → a_field
            let base_name = expr_to_name(base);
            IrExpr::Param(format!("{base_name}_{field}"))
        }

        Expr::Array(body) => lower_array(body, ctx),

        // Unsupported in analog scalar context
        Expr::BundleLit { .. } | Expr::Lambda { .. } => IrExpr::Real(0.0),
    }
}

/// Lower an array literal (`[...]`, `repeat`, or comprehension) into an
/// [`IrExpr::Array`] or [`IrExpr::ArrayRepeat`].
pub(crate) fn lower_array(body: &ArrayBody, ctx: &mut LowerCtx) -> IrExpr {
    match body {
        ArrayBody::List(exprs) => {
            IrExpr::Array(exprs.iter().map(|e| lower_expr(e, ctx)).collect())
        }
        ArrayBody::Repeat(v, n) => {
            IrExpr::ArrayRepeat(
                Box::new(lower_expr(v, ctx)),
                Box::new(lower_expr(n, ctx)),
            )
        }
        ArrayBody::Comprehension(expr, var, range) => {
            // Try to unroll if bounds are const
            if let (Some(start), Some(end)) = (
                eval_const_int(&range.start),
                eval_const_int(&range.end),
            ) {
                let inclusive = range.inclusive as i64;
                let mut elems = vec![];
                for i in start..(end + inclusive) {
                    let mut iter_ctx = ctx.clone();
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

/// Try to evaluate an expression as a compile-time integer (e.g. for array
/// sizes), returning `None` if it is non-constant.
pub(crate) fn eval_const_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int(n)) => Some(*n as i64),
        Expr::Literal(Literal::Bool(true)) => Some(1),
        Expr::Literal(Literal::Bool(false)) => Some(0),
        Expr::Literal(Literal::Real(f)) => Some(*f as i64),
        _ => None,
    }
}

/// Flatten an expression into a canonical dotted-string name (e.g.
/// `foo.bar.baz` → `"foo_bar_baz"`).
pub(crate) fn expr_to_name(expr: &Expr) -> String {
    match expr {
        Expr::Ident(s) => s.clone(),
        Expr::Path(p) => p.segments.join("::"),
        Expr::Field(base, field) => format!("{}_{}", expr_to_name(base), field),
        _ => "_".into(),
    }
}

/// Evaluate a block expression to an [`IrExpr`]; side-effectful statements
/// (like variable declarations) are processed, then the tail expression or
/// last expression-statement is returned.
pub(crate) fn block_value(block: &Block, ctx: &mut LowerCtx) -> IrExpr {
    // Process statements for side effects (var decls)
    for s in &block.stmts {
        if let Stmt::VarDecl { name, default: Some(expr), .. } = s {
            let val = lower_expr(expr, ctx);
            ctx.env.insert(name.clone(), val);
        }
    }
    if let Some(e) = &block.expr {
        return lower_expr(e, ctx);
    }
    // Last stmt that's an Expr
    for s in block.stmts.iter().rev() {
        if let Stmt::Expr(e) = s {
            return lower_expr(e, ctx);
        }
    }
    IrExpr::Real(0.0)
}

/// Lower a function call: analog accessors (`V`, `I`), system functions
/// (`ddt`, `idt`, `transition`, …), simulation queries, and generic user calls.
pub(crate) fn lower_call(func: &Expr, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
    let name = match func {
        Expr::Ident(s) => s.as_str(),
        _ => return IrExpr::Real(0.0),
    };

    match name {
        "V" | "I" => {
            if args.len() >= 2 {
                let plus = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
                let minus = ident_from_expr(Some(&args[1])).unwrap_or_else(|| "?".into());
                IrExpr::BranchAccess { access: name.to_string(), plus, minus }
            } else if args.len() == 1 {
                let node = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
                IrExpr::BranchAccess { access: name.to_string(), plus: node, minus: "0".into() }
            } else {
                IrExpr::BranchAccess { access: name.to_string(), plus: "?".into(), minus: "0".into() }
            }
        }
        "ddt" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let id = ctx.alloc_state(IrStateKind::Ddt, arg);
            IrExpr::StateRef(id)
        }
        "idt" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let ic = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Idt { ic }, arg);
            IrExpr::StateRef(id)
        }
        "idtmod" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let ic = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let modulus = args.get(2).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(1.0));
            let id = ctx.alloc_state(IrStateKind::IdtMod { ic, modulus }, arg);
            IrExpr::StateRef(id)
        }
        "ddx" if args.len() >= 2 => {
            let arg = lower_expr(&args[0], ctx);
            let node = ident_from_expr(Some(&args[1])).unwrap_or_else(|| "?".into());
            let id = ctx.alloc_state(IrStateKind::Ddx { node }, arg);
            IrExpr::StateRef(id)
        }
        "delay" | "absdelay" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let delay = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Delay { delay }, arg);
            IrExpr::StateRef(id)
        }
        "transition" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let delay = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let rise = args.get(2).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let fall = args.get(3).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let tol = args.get(4).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Transition { delay, rise, fall, tol }, arg);
            IrExpr::StateRef(id)
        }
        "slew" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let rise = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let fall = args.get(2).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Slew { rise, fall }, arg);
            IrExpr::StateRef(id)
        }
        "laplace_np" | "laplace_zp" | "laplace_pm" | "laplace_nm" | "laplace_npm"
            if args.len() >= 3 =>
        {
            let arg = lower_expr(&args[0], ctx);
            let num = lower_expr(&args[1], ctx);
            let den = lower_expr(&args[2], ctx);
            let id = ctx.alloc_state(
                IrStateKind::Laplace {
                    variant: name.trim_start_matches("laplace_").to_string(),
                    num,
                    den,
                },
                arg,
            );
            IrExpr::StateRef(id)
        }
        "zi_zd" | "zi_zp" | "zi_nd" | "zi_np" if args.len() >= 4 => {
            let arg = lower_expr(&args[0], ctx);
            let num = lower_expr(&args[1], ctx);
            let den = lower_expr(&args[2], ctx);
            let sample_dt = lower_expr(&args[3], ctx);
            let id = ctx.alloc_state(
                IrStateKind::ZTransform {
                    variant: name.trim_start_matches("zi_").to_string(),
                    num,
                    den,
                    sample_dt,
                },
                arg,
            );
            IrExpr::StateRef(id)
        }
        "ac_stim" => {
            let mag = args.first().map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(1.0));
            let phase = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            IrExpr::AcStim { mag: Box::new(mag), phase: Box::new(phase) }
        }
        "white_noise" | "flicker_noise" => {
            // Noise sources tracked separately via scan_noise; return 0 in expr position.
            IrExpr::Real(0.0)
        }
        "analysis" => {
            let kind = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "dc".into()
            };
            IrExpr::Sim(SimQuery::Analysis(kind))
        }
        _ => {
            let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
            IrExpr::Call(name.to_string(), ir_args)
        }
    }
}

/// Lower a `$system_call` into the corresponding simulator query
/// (`$temperature`, `$vt`, `$simparam`, …).
pub(crate) fn lower_syscall(name: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
    match name.trim_start_matches('$').to_lowercase().as_str() {
        "temperature" => IrExpr::Sim(SimQuery::Temperature),
        "vt" => {
            if args.is_empty() {
                IrExpr::Sim(SimQuery::Vt(None))
            } else {
                IrExpr::Sim(SimQuery::Vt(Some(Box::new(lower_expr(&args[0], ctx)))))
            }
        }
        "abstime" => IrExpr::Sim(SimQuery::Abstime),
        "mfactor" => IrExpr::Sim(SimQuery::Mfactor),
        "xposition" => IrExpr::Sim(SimQuery::XPosition),
        "yposition" => IrExpr::Sim(SimQuery::YPosition),
        "angle" => IrExpr::Sim(SimQuery::Angle),
        "simparam" => {
            let key = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            let default = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            IrExpr::Sim(SimQuery::Simparam { key, default: Box::new(default) })
        }
        "param_given" => {
            let name = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            IrExpr::Sim(SimQuery::ParamGiven(name))
        }
        "port_connected" => {
            let name = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            IrExpr::Sim(SimQuery::PortConnected(name))
        }
        "limit" => {
            let kind = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            let limit_args = args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect();
            IrExpr::Sim(SimQuery::Limit { kind, args: limit_args })
        }
        "random" => {
            IrExpr::Sim(SimQuery::Random { kind: "random".into(), args: vec![] })
        }
        n if n.starts_with("dist_") => {
            let dist_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
            IrExpr::Sim(SimQuery::Random { kind: n.to_string(), args: dist_args })
        }
        "analysis" => {
            let kind = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "dc".into()
            };
            IrExpr::Sim(SimQuery::Analysis(kind))
        }
        _ => IrExpr::Real(0.0),
    }
}

/// Convert a PHDL binary operator to the corresponding IR [`IrBinOp`].
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
    }
}
