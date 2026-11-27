//! Expressions: evaluation of an expression to a value, a real or a bool, over dual numbers.

use piperine_abi::device::Fault;
use piperine_ir::{BinaryOp, Expr, ExprId, MathFn, UnaryOp};

use super::{Frame, Value, Walk};
use crate::dual::Dual;
use piperine_plan::Mode;

impl<'a> Walk<'a> {
    /// The locals of the innermost frame.
    pub(super) fn locals(&mut self) -> &mut Vec<Value> {
        self.frames.last_mut().expect("a walk always has a frame")
    }

    /// Evaluates `id` and requires a real; `Unread` faults (`UNDECLARED`).
    pub fn eval_real(&mut self, id: ExprId) -> Result<Dual, Fault> {
        match self.eval(id)? {
            Value::Real(d) => Ok(d),
            Value::Unread => Err(self.undeclared(id)),
            other => Err(self.fault(id, &format!("expected a real, found {}", Self::kind(&other)))),
        }
    }

    /// Evaluates `id` and requires a bool; `Unread` faults (`UNDECLARED`).
    pub fn eval_bool(&mut self, id: ExprId) -> Result<bool, Fault> {
        match self.eval(id)? {
            Value::Bool(b) => Ok(b),
            Value::Unread => Err(self.undeclared(id)),
            other => Err(self.fault(id, &format!("expected a bool, found {}", Self::kind(&other)))),
        }
    }

    /// A real or `Unread`; any other value faults.
    pub(super) fn real_or_unread(&mut self, id: ExprId) -> Result<Option<Dual>, Fault> {
        match self.eval(id)? {
            Value::Real(d) => Ok(Some(d)),
            Value::Unread => Ok(None),
            other => Err(self.fault(id, &format!("expected a real, found {}", Self::kind(&other)))),
        }
    }

    /// A bool or `Unread`; any other value faults.
    pub(super) fn bool_or_unread(&mut self, id: ExprId) -> Result<Option<bool>, Fault> {
        match self.eval(id)? {
            Value::Bool(b) => Ok(Some(b)),
            Value::Unread => Ok(None),
            other => Err(self.fault(id, &format!("expected a bool, found {}", Self::kind(&other)))),
        }
    }

    /// Evaluates one expression.
    pub fn eval(&mut self, id: ExprId) -> Result<Value, Fault> {
        if let Some(v) = self.read_form(id) {
            return Ok(v);
        }
        let ir = self.ir;
        match ir.expr(id) {
            Expr::Real(r) => Ok(Value::Real(Dual::constant(r.get()))),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Unit => Ok(Value::Unit),
            Expr::Local(i) => Ok(self.locals()[*i as usize].clone()),
            Expr::Param(i) => match self.params.get(*i as usize) {
                Some(v) => Ok(v.clone()),
                None => Err(self.fault(id, &format!("param {i} is not bound"))),
            },
            Expr::Unary { op, expr } => match op {
                UnaryOp::Neg => Ok(self.real_or_unread(*expr)?.map_or(Value::Unread, |x| Value::Real(-x))),
                UnaryOp::Not => Ok(self.bool_or_unread(*expr)?.map_or(Value::Unread, |b| Value::Bool(!b))),
            },
            Expr::Binary { op, lhs, rhs } => self.binary(id, *op, *lhs, *rhs),
            Expr::If { cond, then, otherwise } => match self.bool_or_unread(*cond)? {
                Some(true) => self.eval(*then),
                Some(false) => self.eval(*otherwise),
                None => Ok(Value::Unread),
            },
            Expr::Block { stmts, value } => {
                for s in stmts {
                    self.exec(*s)?;
                }
                match value {
                    Some(v) => self.eval(*v),
                    None => Ok(Value::Unit),
                }
            }
            Expr::Call { function, args } => {
                let mut values = Vec::with_capacity(args.len());
                for a in args {
                    values.push(self.eval(*a)?);
                }
                self.call(*function, values)
            }
            Expr::Math { f, args } => {
                let Some(first) = args.first() else {
                    return Err(self.fault(id, "a math function with no argument"));
                };
                let Some(x) = self.real_or_unread(*first)? else { return Ok(Value::Unread) };
                Ok(Value::Real(match f {
                    MathFn::Exp => x.exp(),
                    MathFn::Ln => x.ln(),
                    MathFn::Sqrt => x.sqrt(),
                    MathFn::Abs => x.abs(),
                }))
            }
            Expr::Closure { .. } => Ok(Value::Closure(id)),
            Expr::Stim { site, mag, phase } if self.mode() == Some(Mode::Stim(*site)) => self.stim_site(*mag, *phase),
            Expr::Limit { site, x, f, seed } if self.mode() == Some(Mode::Limit(*site)) => {
                self.limit_site(id, *x, *f, *seed)
            }
            other => Err(self.fault(id, &format!("the IR {} is not executed by this walk", Self::form(other)))),
        }
    }

    /// The mode of the analog walk, or `None` for a value walk.
    pub(super) fn mode(&self) -> Option<Mode> {
        self.analog.as_ref().map(|a| a.mode)
    }

    /// The value of an analog read form (probe, environment, a site read in `Terms`), `Unread`
    /// when the function does not read its cell, or `None` when `id` is not one this walk
    /// answers from its frame.
    pub(super) fn read_form(&self, id: ExprId) -> Option<Value> {
        let analog = self.analog.as_ref()?;
        let f = &analog.frame;
        if let Expr::Env(piperine_ir::Env::TemperatureGiven) = self.ir.expr(id) {
            return Some(f.temperature_given.map_or(Value::Unread, Value::Bool));
        }
        let read = match self.ir.expr(id) {
            Expr::Probe { plus, minus } => f.probe(*plus, *minus),
            Expr::Env(env) => Frame::read(Some(match env {
                piperine_ir::Env::Temperature => &f.temperature,
                piperine_ir::Env::Gmin => &f.gmin,
                piperine_ir::Env::SourceScale => &f.source_scale,
                // Not a frame cell: `eval` faults on it (`UNEXECUTED`), and `Interpreter::new`
                // rejects a body that holds one.
                piperine_ir::Env::Time | piperine_ir::Env::Static | piperine_ir::Env::Freq | piperine_ir::Env::Analysis(_) | piperine_ir::Env::TemperatureGiven => return None,
            })),
            // §18 rule 3, abi.md A2: the held value stands in for `x`; `x`, `f` and the seed are
            // the site function's, not evaluated here.
            // Any other site's function reads the site's value too, never its arguments.
            Expr::Limit { site, .. } if analog.mode != Mode::Limit(*site) => Frame::read(f.limits.get(*site as usize)),
            Expr::Stim { site, .. } if analog.mode != Mode::Stim(*site) => Frame::read(f.stims.get(*site as usize)),
            _ => return None,
        };
        Some(read.map_or(Value::Unread, Value::Real))
    }

    /// A binary operator over dual numbers; `&&` and `||` short-circuit.
    pub(super) fn binary(&mut self, id: ExprId, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Value, Fault> {
        match op {
            // An `Unread` left side decides nothing: the right side is not run.
            BinaryOp::And | BinaryOp::Or => {
                let Some(left) = self.bool_or_unread(lhs)? else { return Ok(Value::Unread) };
                if left == (op == BinaryOp::Or) {
                    return Ok(Value::Bool(left));
                }
                Ok(self.bool_or_unread(rhs)?.map_or(Value::Unread, Value::Bool))
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let (l, r) = (self.eval(lhs)?, self.eval(rhs)?);
                let same = match (&l, &r) {
                    (Value::Unread, Value::Real(_) | Value::Bool(_) | Value::Unread)
                    | (Value::Real(_) | Value::Bool(_), Value::Unread) => return Ok(Value::Unread),
                    (Value::Real(a), Value::Real(b)) => a.v == b.v,
                    (Value::Bool(a), Value::Bool(b)) => a == b,
                    _ => return Err(self.fault(id, "equality needs two reals or two bools")),
                };
                Ok(Value::Bool(same == (op == BinaryOp::Eq)))
            }
            _ => {
                let (a, b) = (self.real_or_unread(lhs)?, self.real_or_unread(rhs)?);
                let (Some(a), Some(b)) = (a, b) else { return Ok(Value::Unread) };
                Ok(match op {
                    BinaryOp::Add => Value::Real(a + b),
                    BinaryOp::Sub => Value::Real(a - b),
                    BinaryOp::Mul => Value::Real(a * b),
                    BinaryOp::Div => Value::Real(a / b),
                    BinaryOp::Lt => Value::Bool(a.v < b.v),
                    BinaryOp::Le => Value::Bool(a.v <= b.v),
                    BinaryOp::Gt => Value::Bool(a.v > b.v),
                    BinaryOp::Ge => Value::Bool(a.v >= b.v),
                    BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Ne => {
                        return Err(self.fault(id, "unreachable operator"));
                    }
                    BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
                        return Err(self.fault(id, &format!("the IR operator `{op:?}` is not executed by this walk")));
                    }
                })
            }
        }
    }
}
