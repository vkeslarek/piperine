//! Symbolic differentiation of analog expressions (POM `Expr`).
//!
//! The Jacobian is built by differentiating each flattened contribution
//! w.r.t. every branch voltage it reads, then emitting the derivative like
//! any other expression. `__state_load` / `__ddt` reads are constants within
//! a Newton iteration (reactive parts are handled by the charge Jacobian).

use piperine_lang::parse::ast::{BinaryOp, Expr, Literal, UnaryOp};

use super::symbols::NodeId;

/// The `(__temp)/(__dtemp)`-tape marker used by every existing (single-order)
/// call site — `__temp(id)` leaves differentiate to `__dtemp(id)`.
const TAPE_D1: &[(&str, &str)] = &[("__temp", "__dtemp")];

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
    }, resolve_node, TAPE_D1)
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
    }, resolve_node, TAPE_D1)
}

/// `∂²expr / ∂V(a,b)∂V(c,d)` — the `.disto` 2nd-derivative kernel's core
/// (DISTO-03), composed from two single-variable passes through a private
/// intermediate tape marker (`__dtemp_inner`) rather than literally calling
/// [`d_dv`] twice:
///
/// 1. First pass (w.r.t. `(a,b)`): `__temp(id)` leaves become
///    `__dtemp_inner(id)` — kept distinct from the caller-visible `__dtemp`
///    marker so it cannot collide with the second pass's own use of that name.
/// 2. Second pass (w.r.t. `(c,d)`) resolves *two* leaf kinds at once:
///    - a `__temp(id)` leaf surviving from the original expression (product/
///      quotient/power rules keep an untouched copy of each operand)
///      differentiates to `__dtemp(id)` — the ordinary first-derivative tape
///      for branch `(c,d)`, exactly what [`d_dv`] would build for that
///      branch. The caller installs this tape (via
///      [`Builder::set_deriv_tape`](crate::codegen::Builder::set_deriv_tape))
///      before evaluating the result.
///    - a `__dtemp_inner(id)` leaf (the first pass's own tape reference)
///      differentiates to `__ddtemp(id)` — the genuine second-order cross
///      term, i.e. `d²(temps[id])/dV(a,b)dV(c,d)`. The caller builds this
///      tape by calling `d_dv_twice` on each `temps[id]` itself and installs
///      it via `Builder::set_ddtemp_tape`.
///
/// A `__dtemp_inner(id)` leaf can also *survive* the second pass unchanged —
/// the product/quotient/power rules clone their undifferentiated operands,
/// and a cloned `__dtemp_inner` still means `d(temps[id])/dV(a,b)`, a *first*
/// derivative w.r.t. the pair's `(a,b)` branch. The caller therefore installs
/// a third tape — the `(a,b)`-branch first-derivative tape, via
/// `Builder::set_deriv_tape2` — so `emit_dtemp2` can evaluate those leaves.
///
/// Every caller (the `disto2`/`disto3` kernel compilers) must build and
/// install all three tapes — the `(c,d)`-branch `__dtemp` tape, the
/// `(a,b)`-branch `__dtemp_inner` tape, and the cross `__ddtemp` tape —
/// before emitting the result, mirroring how
/// [`crate::jit::analog::AnalogCompiler::compile_jacobian`] installs its
/// single derivative tape before emitting a first-order Jacobian row.
pub fn d_dv_twice(
    expr: &Expr,
    a: NodeId,
    b: NodeId,
    c: NodeId,
    d: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    let seed1 = |p: NodeId, m: NodeId| {
        if p == a && m == b { lit(1.0) } else { lit(0.0) }
    };
    let inner = differentiate(expr, &seed1, resolve_node, &[("__temp", "__dtemp_inner")]);
    let seed2 = |p: NodeId, m: NodeId| {
        if p == c && m == d { lit(1.0) } else { lit(0.0) }
    };
    differentiate(&inner, &seed2, resolve_node, &[("__temp", "__dtemp"), ("__dtemp_inner", "__ddtemp")])
}

/// Core chain-rule walk; `seed` gives the derivative of a branch read.
/// `tapes` is an ordered list of `(leaf_marker, derivative_marker)` pairs —
/// a `__leaf_marker(id)` call becomes `Call(derivative_marker, [id])` rather
/// than being treated as an opaque zero-derivative constant. Every existing
/// (single-order) caller passes [`TAPE_D1`]; [`d_dv_twice`] additionally
/// activates a second pair to resolve leaves surviving from its first pass.
fn differentiate(
    expr: &Expr,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    tapes: &[(&str, &str)],
) -> Expr {
    match expr {
        // $limit is transparent to differentiation: d(vnew)/dV.
        Expr::SysCall(name, args) if name.trim_start_matches('$') == "limit" => {
            args.get(1).map(|vnew| differentiate(vnew, seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0))
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
                // A value-tape leaf: its derivative is the matching entry of
                // the derivative tape (`d(temps[id])/dV`), referenced as the
                // pair's derivative-marker leaf and emitted once per branch.
                let mut tape_hit = None;
                for (leaf_marker, deriv_marker) in tapes {
                    if name == leaf_marker {
                        tape_hit = Some(*deriv_marker);
                        break;
                    }
                }
                if let Some(deriv_marker) = tape_hit {
                    if let Some(Expr::Literal(Literal::Int(id))) = args.first() {
                        return Expr::Call(
                            Box::new(Expr::Ident(deriv_marker.into())),
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
                    return d_math(name, args, seed, resolve_node, tapes);
                }
            }
            lit(0.0)
        }

        Expr::Ident(_) => lit(0.0),
        Expr::Path(_) => lit(0.0),

        Expr::Unary(UnaryOp::Neg, x) => neg(differentiate(x, seed, resolve_node, tapes)),
        Expr::Unary(UnaryOp::Not, _) => lit(0.0),

        Expr::Binary(lhs, op, rhs) => d_binary(op.clone(), lhs, rhs, seed, resolve_node, tapes),

        Expr::If { cond, then_body, else_body } => {
            let t = block_deriv(then_body, seed, resolve_node, tapes);
            let e = block_deriv(else_body, seed, resolve_node, tapes);
            Expr::If {
                cond: cond.clone(),
                then_body: piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(t)) },
                else_body: piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(e)) },
            }
        }

        Expr::Block(b) => {
            if let Some(e) = &b.expr {
                return differentiate(e, seed, resolve_node, tapes);
            }
            for s in b.stmts.iter().rev() {
                if let piperine_lang::parse::ast::Stmt::Expr(e) = s {
                    return differentiate(e, seed, resolve_node, tapes);
                }
            }
            lit(0.0)
        }

        Expr::Cast(_, inner) => differentiate(inner, seed, resolve_node, tapes),
        Expr::Field(_, _) => lit(0.0),

        _ => lit(0.0),
    }
}

fn block_deriv(
    block: &piperine_lang::parse::ast::Block,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    tapes: &[(&str, &str)],
) -> Expr {
    if let Some(e) = &block.expr {
        return differentiate(e, seed, resolve_node, tapes);
    }
    for s in block.stmts.iter().rev() {
        if let piperine_lang::parse::ast::Stmt::Expr(e) = s {
            return differentiate(e, seed, resolve_node, tapes);
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
    tapes: &[(&str, &str)],
) -> Expr {
    let da = differentiate(a, seed, resolve_node, tapes);
    let db = differentiate(b, seed, resolve_node, tapes);
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
    tapes: &[(&str, &str)],
) -> Expr {
    let u = args.first().cloned().unwrap_or_else(|| lit(0.0));
    let du = args.first().map(|a| differentiate(a, seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0));
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
            let dv = args.get(1).map(|a| differentiate(a, seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0));
            binary(super::BinOp::Pow, u, v).d_via_pow(du, dv)
        }
        "min" | "max" => {
            let v = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
            let dv = args.get(1).map(|a| differentiate(a, seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0));
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
