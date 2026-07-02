//! Symbolic differentiation of analog expressions.
//!
//! The Jacobian is built by differentiating each flattened contribution
//! w.r.t. every branch voltage it reads, then emitting the derivative like
//! any other expression. `State` reads are constants within a Newton
//! iteration (reactive parts are handled by the charge Jacobian), so their
//! derivative is zero.

use crate::ir::{IrBinOp, IrExpr, IrUnOp, NodeId};

impl IrExpr {
    /// `∂self / ∂V(plus, minus)` as a new expression.
    pub fn d_dv(&self, plus: NodeId, minus: NodeId) -> IrExpr {
        self.differentiate(&|p, m| {
            if p == plus && m == minus {
                lit(1.0)
            } else {
                lit(0.0)
            }
        })
    }

    /// `∂self / ∂V(node)` — the `ddx` derivative w.r.t. a single node
    /// potential: a branch `V(a,b)` contributes `+1` if `a == node`,
    /// `−1` if `b == node`.
    pub fn d_dnode(&self, node: NodeId) -> IrExpr {
        self.differentiate(&|p, m| {
            let sign = f64::from(p == node) - f64::from(m == node);
            lit(sign)
        })
    }

    /// Core chain-rule walk; `seed` gives the derivative of a branch read.
    fn differentiate(&self, seed: &impl Fn(NodeId, NodeId) -> IrExpr) -> IrExpr {
        match self {
            IrExpr::Real(_)
            | IrExpr::Int(_)
            | IrExpr::Bool(_)
            | IrExpr::Quad(_)
            | IrExpr::Param(_)
            | IrExpr::Var(_)
            | IrExpr::Sim(_)
            | IrExpr::Net(_)
            | IrExpr::State(_)
            | IrExpr::AcStim { .. } => lit(0.0),

            IrExpr::Branch { plus, minus, .. } => seed(*plus, *minus),

            IrExpr::Unary(IrUnOp::Neg, x) => neg(x.differentiate(seed)),
            // Boolean / bitwise results are piecewise constant.
            IrExpr::Unary(_, _) => lit(0.0),

            IrExpr::Binary(op, a, b) => Self::d_binary(*op, a, b, seed),

            IrExpr::Select(c, t, e) => IrExpr::Select(
                c.clone(),
                Box::new(t.differentiate(seed)),
                Box::new(e.differentiate(seed)),
            ),

            IrExpr::MathCall(name, args) => Self::d_math(name, args, seed),

            // User calls are inlined before differentiation; vectors have no
            // scalar derivative. Emission of these in an analog contribution
            // is rejected upstream, so a zero here is unreachable rather
            // than a silent fallback.
            IrExpr::Call(..) | IrExpr::Array(_) | IrExpr::Index(..) | IrExpr::Slice(..) => lit(0.0),
        }
    }

    fn d_binary(op: IrBinOp, a: &IrExpr, b: &IrExpr, seed: &impl Fn(NodeId, NodeId) -> IrExpr) -> IrExpr {
        let da = a.differentiate(seed);
        let db = b.differentiate(seed);
        match op {
            IrBinOp::Add => add(da, db),
            IrBinOp::Sub => sub(da, db),
            IrBinOp::Mul => add(mul(da, b.clone()), mul(a.clone(), db)),
            IrBinOp::Div => div(
                sub(mul(da, b.clone()), mul(a.clone(), db)),
                mul(b.clone(), b.clone()),
            ),
            // General power rule: (u^v)' = u^v · (v'·ln u + v·u'/u).
            // With a constant exponent (the common case) v' = 0 and it
            // folds to v·u^(v−1)·u'.
            IrBinOp::Pow => {
                if is_zero(&db) {
                    mul(
                        mul(b.clone(), IrExpr::binary(IrBinOp::Pow, a.clone(), sub(b.clone(), lit(1.0)))),
                        da,
                    )
                } else {
                    mul(
                        IrExpr::binary(IrBinOp::Pow, a.clone(), b.clone()),
                        add(
                            mul(db, math1("ln", a.clone())),
                            div(mul(b.clone(), da), a.clone()),
                        ),
                    )
                }
            }
            // Comparisons, logic, bit ops, shifts, remainder: piecewise
            // constant almost everywhere.
            _ => lit(0.0),
        }
    }

    fn d_math(name: &str, args: &[IrExpr], seed: &impl Fn(NodeId, NodeId) -> IrExpr) -> IrExpr {
        let u = args.first().cloned().unwrap_or_else(|| lit(0.0));
        let du = args
            .first()
            .map(|a| a.differentiate(seed))
            .unwrap_or_else(|| lit(0.0));
        match name {
            "exp" | "limexp" => mul(math1("exp", u), du),
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
                IrExpr::select(
                    IrExpr::binary(IrBinOp::Ge, u, lit(0.0)),
                    lit(1.0),
                    lit(-1.0),
                ),
                du,
            ),
            "pow" => {
                let v = args.get(1).cloned().unwrap_or_else(|| lit(1.0));
                let dv = args
                    .get(1)
                    .map(|a| a.differentiate(seed))
                    .unwrap_or_else(|| lit(0.0));
                IrExpr::binary(IrBinOp::Pow, u, v).d_via_pow(du, dv)
            }
            "min" | "max" => {
                let v = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
                let dv = args
                    .get(1)
                    .map(|a| a.differentiate(seed))
                    .unwrap_or_else(|| lit(0.0));
                let pick_first = if name == "min" {
                    IrExpr::binary(IrBinOp::Le, u, v)
                } else {
                    IrExpr::binary(IrBinOp::Ge, u, v)
                };
                IrExpr::select(pick_first, du, dv)
            }
            // floor/ceil and friends are piecewise constant.
            _ => lit(0.0),
        }
    }

    /// Rewrites `self` (which must be `Pow(u, v)`) into its derivative given
    /// `du`/`dv`; shared by the operator and the `pow` call.
    fn d_via_pow(self, du: IrExpr, dv: IrExpr) -> IrExpr {
        let IrExpr::Binary(IrBinOp::Pow, u, v) = self else {
            return lit(0.0);
        };
        if is_zero(&dv) {
            mul(
                mul((*v).clone(), IrExpr::binary(IrBinOp::Pow, (*u).clone(), sub((*v).clone(), lit(1.0)))),
                du,
            )
        } else {
            mul(
                IrExpr::Binary(IrBinOp::Pow, u.clone(), v.clone()),
                add(mul(dv, math1("ln", (*u).clone())), div(mul((*v).clone(), du), (*u).clone())),
            )
        }
    }
}

// ── Constant-folding constructors (keep derivative trees small) ──────────────

fn lit(v: f64) -> IrExpr {
    IrExpr::Real(v)
}

fn is_zero(e: &IrExpr) -> bool {
    matches!(e, IrExpr::Real(v) if *v == 0.0)
}

fn is_one(e: &IrExpr) -> bool {
    matches!(e, IrExpr::Real(v) if *v == 1.0)
}

fn add(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_zero(&a) {
        return b;
    }
    if is_zero(&b) {
        return a;
    }
    IrExpr::binary(IrBinOp::Add, a, b)
}

fn sub(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_zero(&b) {
        return a;
    }
    IrExpr::binary(IrBinOp::Sub, a, b)
}

fn mul(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_zero(&a) || is_zero(&b) {
        return lit(0.0);
    }
    if is_one(&a) {
        return b;
    }
    if is_one(&b) {
        return a;
    }
    IrExpr::binary(IrBinOp::Mul, a, b)
}

fn div(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_zero(&a) {
        return lit(0.0);
    }
    if is_one(&b) {
        return a;
    }
    IrExpr::binary(IrBinOp::Div, a, b)
}

fn neg(a: IrExpr) -> IrExpr {
    if let IrExpr::Real(v) = &a {
        return lit(-v);
    }
    IrExpr::Unary(IrUnOp::Neg, Box::new(a))
}

fn math1(name: &str, a: IrExpr) -> IrExpr {
    IrExpr::MathCall(name.to_string(), vec![a])
}
