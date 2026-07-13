//! Symbolic differentiation of analog expressions (POM `Expr`).
//!
//! The Jacobian is built by differentiating each flattened contribution
//! w.r.t. every branch voltage it reads, then emitting the derivative like
//! any other expression. `__state_load` / `__ddt` reads are constants within
//! a Newton iteration (reactive parts are handled by the charge Jacobian).

use piperine_lang::parse::ast::{BinaryOp, Expr, Literal, UnaryOp};

use super::symbols::NodeId;

/// `∂expr / ∂V(plus, minus)` as a new POM expression. The `resolver` maps
/// node names to `NodeId`s so `V(p,n)` branch accesses can be identified.
pub fn d_dv(
    expr: &Expr,
    plus: NodeId,
    minus: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    differentiate(expr, &|p, m| {
        if p == plus && m == minus {
            lit(1.0)
        } else {
            lit(0.0)
        }
    }, resolve_node)
}

/// `∂expr / ∂V(node)` — the `ddx` derivative w.r.t. a single node potential.
pub fn d_dnode(
    expr: &Expr,
    node: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    differentiate(expr, &|p, m| {
        let sign = f64::from(p == node) - f64::from(m == node);
        lit(sign)
    }, resolve_node)
}

/// Core chain-rule walk; `seed` gives the derivative of a branch read.
fn differentiate(
    expr: &Expr,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    match expr {
        // $limit is transparent to differentiation: d(vnew)/dV.
        Expr::SysCall(name, args) if name.trim_start_matches('$') == "limit" => {
            args.get(1).map(|vnew| differentiate(vnew, seed, resolve_node)).unwrap_or_else(|| lit(0.0))
        }

        Expr::Literal(Literal::Real(_))
        | Expr::Literal(Literal::Int(_))
        | Expr::Literal(Literal::Bool(_))
        | Expr::Literal(Literal::String(_))
        | Expr::Literal(Literal::None)
        | Expr::Literal(Literal::Quad(_))
        | Expr::SysCall(_, _) => lit(0.0),

        // Branch access: V(p,n) — resolve node names to ids and seed.
        Expr::Call(func, args) => {
            if let Expr::Ident(name) = func.as_ref() {
                if name == "V" || name == "I" {
                    let plus_name = ident_str(args.first()).unwrap_or_else(|| "0".into());
                    let minus_name = ident_str(args.get(1)).unwrap_or_else(|| "0".into());
                    let p = resolve_node(&plus_name).unwrap_or(NodeId::GROUND);
                    let m = resolve_node(&minus_name).unwrap_or(NodeId::GROUND);
                    return seed(p, m);
                }
                // A value-tape temp: its derivative is the matching entry of
                // the derivative tape (`d(temps[id])/dV`), referenced as a
                // `__dtemp(id)` leaf and emitted once per branch.
                if name == "__temp" {
                    if let Some(Expr::Literal(Literal::Int(id))) = args.first() {
                        return Expr::Call(
                            Box::new(Expr::Ident("__dtemp".into())),
                            vec![Expr::Literal(Literal::Int(*id))],
                        );
                    }
                    return lit(0.0);
                }
                // __state_load, __ddt, __idt, __delay, __slew, etc. — constants
                // within a Newton iteration.
                if name.starts_with("__") {
                    return lit(0.0);
                }
                // Math builtins: chain rule.
                if super::math::math_fn(name).is_some() {
                    return d_math(name, args, seed, resolve_node);
                }
            }
            lit(0.0)
        }

        Expr::Ident(_) => lit(0.0),
        Expr::Path(_) => lit(0.0),

        Expr::Unary(UnaryOp::Neg, x) => neg(differentiate(x, seed, resolve_node)),
        Expr::Unary(UnaryOp::Not, _) => lit(0.0),

        Expr::Binary(lhs, op, rhs) => d_binary(op.clone(), lhs, rhs, seed, resolve_node),

        Expr::If { cond, then_body, else_body } => {
            let t = block_deriv(then_body, seed, resolve_node);
            let e = block_deriv(else_body, seed, resolve_node);
            Expr::If {
                cond: cond.clone(),
                then_body: piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(t)) },
                else_body: piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(e)) },
            }
        }

        Expr::Block(b) => {
            if let Some(e) = &b.expr {
                return differentiate(e, seed, resolve_node);
            }
            for s in b.stmts.iter().rev() {
                if let piperine_lang::parse::ast::Stmt::Expr(e) = s {
                    return differentiate(e, seed, resolve_node);
                }
            }
            lit(0.0)
        }

        Expr::Cast(_, inner) => differentiate(inner, seed, resolve_node),
        Expr::Field(_, _) => lit(0.0),

        _ => lit(0.0),
    }
}

fn block_deriv(
    block: &piperine_lang::parse::ast::Block,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    if let Some(e) = &block.expr {
        return differentiate(e, seed, resolve_node);
    }
    for s in block.stmts.iter().rev() {
        if let piperine_lang::parse::ast::Stmt::Expr(e) = s {
            return differentiate(e, seed, resolve_node);
        }
    }
    lit(0.0)
}

fn d_binary(
    op: BinaryOp,
    a: &Expr,
    b: &Expr,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    let da = differentiate(a, seed, resolve_node);
    let db = differentiate(b, seed, resolve_node);
    match op {
        BinaryOp::Add => add(da, db),
        BinaryOp::Sub => sub(da, db),
        BinaryOp::Mul => add(mul(da, b.clone()), mul(a.clone(), db)),
        BinaryOp::Div => div(
            sub(mul(da, b.clone()), mul(a.clone(), db)),
            mul(b.clone(), b.clone()),
        ),
        BinaryOp::And | BinaryOp::Or => lit(0.0),
        _ => {
            // For Pow and other ops, use the IR BinOp mapping.
            let ir_op = super::BinOp::from_pom(op);
            match ir_op {
                super::BinOp::Pow => {
                    if is_zero(&db) {
                        mul(
                            mul(b.clone(), binary(super::BinOp::Pow, a.clone(), sub(b.clone(), lit(1.0)))),
                            da,
                        )
                    } else {
                        mul(
                            binary(super::BinOp::Pow, a.clone(), b.clone()),
                            add(mul(db, math1("ln", a.clone())), div(mul(b.clone(), da), a.clone())),
                        )
                    }
                }
                _ => lit(0.0),
            }
        }
    }
}

fn d_math(
    name: &str,
    args: &[Expr],
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    let u = args.first().cloned().unwrap_or_else(|| lit(0.0));
    let du = args.first().map(|a| differentiate(a, seed, resolve_node)).unwrap_or_else(|| lit(0.0));
    match name {
        "exp" | "limexp" => mul(math1(name, u), du),
        "ln" | "log" => div(du, u),
        "log10" => div(du, mul(u, lit(std::f64::consts::LN_10))),
        "sqrt" => div(du, mul(lit(2.0), math1("sqrt", u))),
        "sin" => mul(math1("cos", u), du),
        "cos" => mul(neg(math1("sin", u)), du),
        "tan" => div(du, mul(math1("cos", u.clone()), math1("cos", u))),
        "asin" => div(du, math1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        "acos" => div(neg(du), math1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        "atan" => div(du, add(lit(1.0), mul(u.clone(), u))),
        "sinh" => mul(math1("cosh", u), du),
        "cosh" => mul(math1("sinh", u), du),
        "tanh" => mul(sub(lit(1.0), mul(math1("tanh", u.clone()), math1("tanh", u))), du),
        "abs" => mul(
            Expr::If {
                cond: Box::new(binary(super::BinOp::Ge, u.clone(), lit(0.0))),
                then_body: block(lit(1.0)),
                else_body: block(lit(-1.0)),
            },
            du,
        ),
        "pow" => {
            let v = args.get(1).cloned().unwrap_or_else(|| lit(1.0));
            let dv = args.get(1).map(|a| differentiate(a, seed, resolve_node)).unwrap_or_else(|| lit(0.0));
            binary(super::BinOp::Pow, u, v).d_via_pow(du, dv)
        }
        "min" | "max" => {
            let v = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
            let dv = args.get(1).map(|a| differentiate(a, seed, resolve_node)).unwrap_or_else(|| lit(0.0));
            let pick_first = if name == "min" {
                binary(super::BinOp::Le, u, v)
            } else {
                binary(super::BinOp::Ge, u, v)
            };
            Expr::If {
                cond: Box::new(pick_first),
                then_body: block(du),
                else_body: block(dv),
            }
        }
        _ => lit(0.0),
    }
}

/// Collect every distinct `V(p,n)` branch pair in the tree.
pub fn collect_branches(expr: &Expr, out: &mut Vec<(NodeId, NodeId)>, resolve_node: &impl Fn(&str) -> Option<NodeId>) {
    use piperine_lang::parse::ast::Walk;
    expr.walk(&mut |e| {
        if let Expr::Call(func, args) = e
            && let Expr::Ident(name) = func.as_ref()
                && (name == "V" || name == "I") {
                    let plus_name = ident_str(args.first()).unwrap_or_else(|| "0".into());
                    let minus_name = ident_str(args.get(1)).unwrap_or_else(|| "0".into());
                    let p = resolve_node(&plus_name).unwrap_or(NodeId::GROUND);
                    let m = resolve_node(&minus_name).unwrap_or(NodeId::GROUND);
                    let pair = (p, m);
                    if !out.contains(&pair) {
                        out.push(pair);
                    }
                    return Walk::SkipChildren;
                }
        Walk::Continue
    });
}

/// Walk an expression and call `f` on each node (pre-order).
pub fn visit_expr<F: FnMut(&Expr)>(expr: &Expr, f: &mut F) {
    use piperine_lang::parse::ast::Walk;
    expr.walk(&mut |e| {
        f(e);
        Walk::Continue
    });
}

fn ident_str(e: Option<&Expr>) -> Option<String> {
    match e? {
        Expr::Ident(s) => Some(s.clone()),
        Expr::Field(base, field) => match base.as_ref() {
            Expr::Ident(base_name) => Some(format!("{base_name}.{field}")),
            _ => None,
        },
        _ => None,
    }
}

// ── Constant-folding constructors ──

fn lit(v: f64) -> Expr {
    Expr::Literal(Literal::Real(v))
}

fn is_zero(e: &Expr) -> bool {
    matches!(e, Expr::Literal(Literal::Real(v)) if *v == 0.0)
}

fn is_one(e: &Expr) -> bool {
    matches!(e, Expr::Literal(Literal::Real(v)) if *v == 1.0)
}

fn add(a: Expr, b: Expr) -> Expr {
    if is_zero(&a) { return b; }
    if is_zero(&b) { return a; }
    binary(super::BinOp::Add, a, b)
}

fn sub(a: Expr, b: Expr) -> Expr {
    if is_zero(&b) { return a; }
    binary(super::BinOp::Sub, a, b)
}

fn mul(a: Expr, b: Expr) -> Expr {
    if is_zero(&a) || is_zero(&b) { return lit(0.0); }
    if is_one(&a) { return b; }
    if is_one(&b) { return a; }
    binary(super::BinOp::Mul, a, b)
}

fn div(a: Expr, b: Expr) -> Expr {
    if is_zero(&a) { return lit(0.0); }
    if is_one(&b) { return a; }
    binary(super::BinOp::Div, a, b)
}

fn neg(a: Expr) -> Expr {
    if let Expr::Literal(Literal::Real(v)) = &a {
        return lit(-v);
    }
    Expr::Unary(UnaryOp::Neg, Box::new(a))
}

fn binary(op: super::BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let pom_op = op.to_pom();
    Expr::Binary(Box::new(lhs), pom_op, Box::new(rhs))
}

fn math1(name: &str, a: Expr) -> Expr {
    Expr::Call(Box::new(Expr::Ident(name.to_string())), vec![a])
}

fn block(e: Expr) -> piperine_lang::parse::ast::Block {
    piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(e)) }
}

// Extension trait for Pow differentiation.
trait PowDeriv {
    fn d_via_pow(self, du: Expr, dv: Expr) -> Expr;
}

impl PowDeriv for Expr {
    fn d_via_pow(self, du: Expr, dv: Expr) -> Expr {
        if is_zero(&dv) {
            let (u, v) = unpack_pow(&self);
            mul(mul(v.clone(), binary(super::BinOp::Pow, u.clone(), sub(v, lit(1.0)))), du)
        } else {
            let (u, v) = unpack_pow(&self);
            mul(
                binary(super::BinOp::Pow, u.clone(), v.clone()),
                add(mul(dv, math1("ln", u.clone())), div(mul(v, du), u)),
            )
        }
    }
}

fn unpack_pow(e: &Expr) -> (Expr, Expr) {
    if let Expr::Binary(lhs, BinaryOp::BitXor, rhs) = e {
        // This shouldn't happen — Pow is handled via Call("pow", ...)
        (lhs.as_ref().clone(), rhs.as_ref().clone())
    } else {
        (e.clone(), lit(1.0))
    }
}
