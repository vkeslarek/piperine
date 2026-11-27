//! Calls: entering a function, walking its body in a scope of its own, and the value a call reads.

use super::{Scope, Task, Walk};
use super::super::Value;
use crate::ids::{ExprId, FunctionId};

impl<'a> Walk<'a> {
    /// Enters `function`, called at `at` with `args`: its body is walked in a scope of its own,
    /// its parameters (its first locals) bound to the arguments; `Return` leaves it.
    pub(super) fn enter(&mut self, at: ExprId, function: FunctionId, args: Vec<Value>, next: &mut Vec<Task>) {
        if let Some(callee) = self.returns.get(&(function, args.clone())).cloned() {
            self.called(at, &args, &callee);
            return;
        }
        if self.calls.contains(&function) {
            // A recursive call reads the result known so far (§3 rule 12): the walk of the
            // function that began it grows that result until it holds still (`leave_call`).
            self.recursing.insert(function);
            let known = self.approximate.get(&function).cloned().unwrap_or_else(Value::constant);
            self.called(at, &args, &known);
            return;
        }
        self.start_call(at, function, args, next);
    }

    /// Walks the body of `function`, called at `at` with `args`, in a scope of its own.
    pub(super) fn start_call(&mut self, at: ExprId, function: FunctionId, args: Vec<Value>, next: &mut Vec<Task>) {
        let ir = self.ir;
        let data = &ir.functions[function.index()];
        let mut callee = Scope::new(format!("function {}", data.name), false, data.locals.len());
        if args.len() != data.params.len() || args.len() > callee.locals.len() {
            self.refuse(at, &format!("call of {} with {} args, it takes {}", data.name, args.len(), data.params.len()));
        }
        for (slot, arg) in callee.locals.iter_mut().zip(&args) {
            *slot = Some(arg.clone());
        }
        callee.enclosed_from = self.enclosing.len();
        self.calls.push(function);
        self.scopes.push(callee);
        next.extend([Task::Expr(data.body), Task::Return { at, function, args }]);
    }

    /// A call at `at` returned `callee`: the call reads its arguments and what the callee reads.
    pub(super) fn called(&mut self, at: ExprId, args: &[Value], callee: &Value) {
        let mut out = Value::nonlinear(args);
        out.sources.join(&callee.sources);
        self.done(at, out);
    }
}
