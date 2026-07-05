//! IR expressions (SPEC §5) plus the traversal and constant-evaluation
//! helpers the codegen needs.

use super::symbols::{FnId, NatureId, NodeId, ParamId, StateId, VarId};

/// A resolved analysis kind returned by `$analysis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Analysis {
    Dc,
    Ac,
    Tran,
    Noise,
}

/// A position axis for `$xposition` / `$yposition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// A simulator query (`$temperature`, `$vt`, `$abstime`, …).
#[derive(Debug, Clone, PartialEq)]
pub enum SimQuery {
    Temperature,
    Vt(Option<Box<IrExpr>>),
    Abstime,
    Mfactor,
    Position(Axis),
    Angle,
    Simparam { key: String, default: Box<IrExpr> },
    Analysis(Analysis),
    ParamGiven(ParamId),
    PortConnected(NodeId),
    Limit { kind: String, args: Vec<IrExpr> },
    Random { kind: String, args: Vec<IrExpr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Logical (short-circuit) and/or.
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    /// Logical shifts.
    Shl,
    Shr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
    /// Bus reductions (digital).
    RedAnd,
    RedOr,
    RedXor,
}

/// An IR expression. References carry resolved ids (SPEC §3); analog
/// operators appear as `State`, never as `Call`.
#[derive(Debug, Clone, PartialEq)]
pub enum IrExpr {
    // Literals.
    Real(f64),
    Int(i64),
    Bool(bool),
    /// 4-state literal: 0=0, 1=1, 2=X, 3=Z.
    Quad(u8),
    // Resolved references.
    Param(ParamId),
    Var(VarId),
    /// Branch access `V(p,n)` / `I(p,n)` / …; `V(a)` resolves `minus` to
    /// [`NodeId::GROUND`].
    Branch { nature: NatureId, plus: NodeId, minus: NodeId },
    /// A digital net read (quad-valued). Digital bodies only.
    Net(NodeId),
    /// An analog-operator result (SPEC §7).
    State(StateId),
    // Queries and stimulus.
    Sim(SimQuery),
    /// `.ac` stimulus; zero outside AC analysis.
    AcStim { mag: Box<IrExpr>, phase: Box<IrExpr> },
    // Computation. `Call` is uniform for built-in math (resolved by name) and
    // user functions (resolved to `FnId`).
    MathCall(String, Vec<IrExpr>),
    Call(FnId, Vec<IrExpr>),
    Binary(BinOp, Box<IrExpr>, Box<IrExpr>),
    Unary(UnOp, Box<IrExpr>),
    /// `cond ? a : b`.
    Select(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),
    // Vectors (buses).
    Array(Vec<IrExpr>),
    Index(Box<IrExpr>, Box<IrExpr>),
    /// `a[lo..hi]`; the flag marks an inclusive upper bound.
    Slice(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>, bool),
}

impl IrExpr {
    /// Shorthand for a binary node.
    pub fn binary(op: BinOp, lhs: IrExpr, rhs: IrExpr) -> IrExpr {
        IrExpr::Binary(op, Box::new(lhs), Box::new(rhs))
    }

    /// Shorthand for a ternary select.
    pub fn select(cond: IrExpr, then_: IrExpr, else_: IrExpr) -> IrExpr {
        IrExpr::Select(Box::new(cond), Box::new(then_), Box::new(else_))
    }

    /// Visit every node of the expression tree (pre-order).
    pub fn visit(&self, f: &mut impl FnMut(&IrExpr)) {
        f(self);
        for child in self.children() {
            child.visit(f);
        }
    }

    /// The direct sub-expressions of this node.
    pub fn children(&self) -> Vec<&IrExpr> {
        match self {
            IrExpr::Real(_)
            | IrExpr::Int(_)
            | IrExpr::Bool(_)
            | IrExpr::Quad(_)
            | IrExpr::Param(_)
            | IrExpr::Var(_)
            | IrExpr::Branch { .. }
            | IrExpr::Net(_)
            | IrExpr::State(_) => Vec::new(),
            IrExpr::Sim(q) => match q {
                SimQuery::Vt(Some(e)) => vec![e.as_ref()],
                SimQuery::Simparam { default, .. } => vec![default.as_ref()],
                SimQuery::Limit { args, .. } | SimQuery::Random { args, .. } => {
                    args.iter().collect()
                }
                _ => Vec::new(),
            },
            IrExpr::AcStim { mag, phase } => vec![mag.as_ref(), phase.as_ref()],
            IrExpr::MathCall(_, args) | IrExpr::Call(_, args) | IrExpr::Array(args) => {
                args.iter().collect()
            }
            IrExpr::Binary(_, a, b) => vec![a.as_ref(), b.as_ref()],
            IrExpr::Unary(_, a) => vec![a.as_ref()],
            IrExpr::Select(c, t, e) => vec![c.as_ref(), t.as_ref(), e.as_ref()],
            IrExpr::Index(a, i) => vec![a.as_ref(), i.as_ref()],
            IrExpr::Slice(a, lo, hi, _) => vec![a.as_ref(), lo.as_ref(), hi.as_ref()],
        }
    }

    /// Rebuild this node with each direct child mapped through `f`.
    pub fn map_children(&self, f: &mut impl FnMut(&IrExpr) -> IrExpr) -> IrExpr {
        let map = |e: &IrExpr, f: &mut dyn FnMut(&IrExpr) -> IrExpr| Box::new(f(e));
        match self {
            IrExpr::Real(_)
            | IrExpr::Int(_)
            | IrExpr::Bool(_)
            | IrExpr::Quad(_)
            | IrExpr::Param(_)
            | IrExpr::Var(_)
            | IrExpr::Branch { .. }
            | IrExpr::Net(_)
            | IrExpr::State(_) => self.clone(),
            IrExpr::Sim(q) => IrExpr::Sim(match q {
                SimQuery::Vt(Some(e)) => SimQuery::Vt(Some(map(e, f))),
                SimQuery::Simparam { key, default } => {
                    SimQuery::Simparam { key: key.clone(), default: map(default, f) }
                }
                SimQuery::Limit { kind, args } => SimQuery::Limit {
                    kind: kind.clone(),
                    args: args.iter().map(|a| f(a)).collect(),
                },
                SimQuery::Random { kind, args } => SimQuery::Random {
                    kind: kind.clone(),
                    args: args.iter().map(|a| f(a)).collect(),
                },
                other => other.clone(),
            }),
            IrExpr::AcStim { mag, phase } => {
                IrExpr::AcStim { mag: map(mag, f), phase: map(phase, f) }
            }
            IrExpr::MathCall(name, args) => {
                IrExpr::MathCall(name.clone(), args.iter().map(|a| f(a)).collect())
            }
            IrExpr::Call(id, args) => IrExpr::Call(*id, args.iter().map(|a| f(a)).collect()),
            IrExpr::Array(items) => IrExpr::Array(items.iter().map(|a| f(a)).collect()),
            IrExpr::Binary(op, a, b) => IrExpr::Binary(*op, map(a, f), map(b, f)),
            IrExpr::Unary(op, a) => IrExpr::Unary(*op, map(a, f)),
            IrExpr::Select(c, t, e) => IrExpr::Select(map(c, f), map(t, f), map(e, f)),
            IrExpr::Index(a, i) => IrExpr::Index(map(a, f), map(i, f)),
            IrExpr::Slice(a, lo, hi, inc) => IrExpr::Slice(map(a, f), map(lo, f), map(hi, f), *inc),
        }
    }

    /// Rewrite the whole tree bottom-up: children first, then `f` on the
    /// rebuilt node.
    pub fn rewrite(&self, f: &mut impl FnMut(IrExpr) -> IrExpr) -> IrExpr {
        let rebuilt = self.map_children(&mut |c| c.rewrite(f));
        f(rebuilt)
    }

    /// The first `State(id)` in the tree whose kind satisfies `pred`, if any.
    pub fn find_state(&self, pred: &impl Fn(StateId) -> bool) -> Option<StateId> {
        let mut found = None;
        self.visit(&mut |e| {
            if found.is_none() {
                if let IrExpr::State(id) = e {
                    if pred(*id) {
                        found = Some(*id);
                    }
                }
            }
        });
        found
    }

    /// Every distinct `Branch` voltage pair in the tree, in first-seen order.
    pub fn collect_branches(&self, out: &mut Vec<(NodeId, NodeId)>) {
        self.visit(&mut |e| {
            if let IrExpr::Branch { plus, minus, .. } = e {
                let pair = (*plus, *minus);
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        });
    }

    /// Evaluate a compile-time-constant expression. `param` resolves
    /// parameter references (e.g. from already-evaluated defaults). Anything
    /// runtime-dependent is an error.
    pub fn eval_const(&self, param: &impl Fn(ParamId) -> Option<f64>) -> Result<f64, String> {
        let eval = |e: &IrExpr| e.eval_const(param);
        match self {
            IrExpr::Real(v) => Ok(*v),
            IrExpr::Int(v) => Ok(*v as f64),
            IrExpr::Bool(b) => Ok(f64::from(*b)),
            IrExpr::Param(id) => {
                param(*id).ok_or_else(|| format!("parameter #{} has no value", id.0))
            }
            IrExpr::Unary(UnOp::Neg, a) => Ok(-eval(a)?),
            IrExpr::Unary(UnOp::Not, a) => Ok(f64::from(eval(a)? == 0.0)),
            IrExpr::Binary(op, a, b) => {
                let (a, b) = (eval(a)?, eval(b)?);
                match op {
                    BinOp::Add => Ok(a + b),
                    BinOp::Sub => Ok(a - b),
                    BinOp::Mul => Ok(a * b),
                    BinOp::Div => Ok(a / b),
                    BinOp::Rem => Ok(a % b),
                    BinOp::Pow => Ok(a.powf(b)),
                    BinOp::Eq => Ok(f64::from(a == b)),
                    BinOp::Ne => Ok(f64::from(a != b)),
                    BinOp::Lt => Ok(f64::from(a < b)),
                    BinOp::Le => Ok(f64::from(a <= b)),
                    BinOp::Gt => Ok(f64::from(a > b)),
                    BinOp::Ge => Ok(f64::from(a >= b)),
                    BinOp::And => Ok(f64::from(a != 0.0 && b != 0.0)),
                    BinOp::Or => Ok(f64::from(a != 0.0 || b != 0.0)),
                    other => Err(format!("operator {other:?} is not const-evaluable")),
                }
            }
            IrExpr::Select(c, t, e) => {
                if eval(c)? != 0.0 { eval(t) } else { eval(e) }
            }
            IrExpr::MathCall(name, args) => {
                let vals: Vec<f64> = args.iter().map(eval).collect::<Result<_, _>>()?;
                piperine_lang::math::eval_const_math(name, &vals)
                    .ok_or_else(|| format!("`{name}` is not a const-evaluable math builtin"))
            }
            other => Err(format!("expression is not compile-time constant: {other:?}")),
        }
    }
}
