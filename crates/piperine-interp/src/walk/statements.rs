//! Statements and calls: executing a statement, calling a function or a closure.

use piperine_abi::device::Fault;
use piperine_ir::{Expr, ExprId, FunctionId, Operator, Stmt, StmtId};

use super::{Sums, Value, Walk};
use crate::dual::Dual;
use piperine_plan::Mode;

impl<'a> Walk<'a> {
    /// Runs one statement: `let`, expression statements, and in a `Terms` walk a contribution,
    /// each addend added with its sign to its branch's direct or `ddt` sum. Elsewhere a
    /// contribution faults.
    pub fn exec(&mut self, id: StmtId) -> Result<(), Fault> {
        let ir = self.ir;
        match ir.stmt(id) {
            Stmt::Let { local, init } => {
                let v = self.eval(*init)?;
                self.locals()[*local as usize] = v;
                Ok(())
            }
            Stmt::Expr(e) => self.eval(*e).map(|_| ()),
            Stmt::Contribute { branch, addends, .. } if self.mode() == Some(Mode::Terms) => {
                for a in addends {
                    let v = self.eval_real(a.value)?;
                    let v = if a.negated { -v } else { v };
                    let Some(analog) = self.analog.as_mut() else { break };
                    let sums = if a.operator == Operator::Direct { &mut analog.sums.direct } else { &mut analog.sums.ddt };
                    match sums.get_mut(*branch as usize) {
                        Some(slot) => Sums::add(slot, v),
                        None => return Err(self.fault(a.value, &format!("a contribution to branch {branch}, out of range"))),
                    }
                }
                Ok(())
            }
            // A site walk evaluates the addends only to reach a site inside them; what they read
            // past the site's own dependencies is `Unread` and decides nothing.
            Stmt::Contribute { addends, .. } if self.analog.is_some() => {
                for a in addends {
                    self.real_or_unread(a.value)?;
                }
                Ok(())
            }
            Stmt::Contribute { addends, .. } => {
                let at = addends.first().map_or_else(|| "<unknown>".to_string(), |a| Self::origin(ir.expr_origin(a.value)));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Contribute` is not executed by the value walk (at {at})")))
            }
            Stmt::Commit { value, .. } => {
                let at = Self::origin(ir.expr_origin(*value));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Commit` is not executed by this walk (at {at})")))
            }
            Stmt::Emit { .. } => Err(Fault::new(Self::UNEXECUTED, "the IR form `Emit` is not executed by this walk")),
            Stmt::Trace { .. } => Err(Fault::new(Self::UNEXECUTED, "the IR form `Trace` is not executed by this walk")),
            Stmt::Assign { value, .. } => {
                let at = Self::origin(ir.expr_origin(*value));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Assign` is not executed by this walk (at {at})")))
            }
            Stmt::BoundStep { value, .. } => {
                let at = Self::origin(ir.expr_origin(*value));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `bound_step` is not executed by this walk (at {at})")))
            }
        }
    }

    /// Calls `function` with `args` bound to its leading locals, in a fresh frame.
    pub fn call(&mut self, function: FunctionId, args: Vec<Value>) -> Result<Value, Fault> {
        let f = self.ir.function(function);
        let mut frame = vec![Value::Unit; f.locals().len().max(args.len())];
        for (slot, arg) in frame.iter_mut().zip(args) {
            *slot = arg;
        }
        self.frames.push(frame);
        let result = self.eval(f.body());
        self.frames.pop();
        result
    }

    /// Calls the closure expression `id` with plain reals, in the current frame: the closure sees
    /// the locals of the block that wrote it.
    pub fn call_closure(&mut self, id: ExprId, args: &[f64]) -> Result<f64, Fault> {
        let ir = self.ir;
        let Expr::Closure { params, body } = ir.expr(id) else {
            return Err(self.fault(id, "not a closure"));
        };
        if params.len() != args.len() {
            return Err(self.fault(id, &format!("a closure of {} params called with {}", params.len(), args.len())));
        }
        for (local, value) in params.iter().zip(args) {
            self.locals()[*local as usize] = Value::Real(Dual::constant(*value));
        }
        Ok(self.eval_real(*body)?.v)
    }
}
