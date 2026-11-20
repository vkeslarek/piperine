//! The dependency walk over mutation (composite-values CV-01): the methods of [`Walk`] that give
//! an assignment its abstract value.
//!
//! The lattice stays finite, so the fixed point here ends: an **assignment** joins the assigned
//! value (and the conditions it sits under) into the local's, so a local only grows.

use super::{Class, Value, Walk};
use crate::ids::ExprId;
use crate::model::{BinaryOp, Expr};

impl Value {
    /// The least value above `self` and `other`: the sources of both, the larger class.
    fn join(&mut self, other: &Value) {
        self.sources.join(&other.sources);
        self.class = self.class.max(other.class);
    }

    /// `self`, read under `conditions`: it also reads what they read, and is no longer affine
    /// when one is not `Constant`, as a contribution under an `if` is not (a switch).
    fn under(&mut self, conditions: &[Value]) {
        for condition in conditions {
            self.sources.join(&condition.sources);
            if condition.class != Class::Constant {
                self.class = Class::Nonlinear;
            }
        }
    }
}

impl<'a> Walk<'a> {
    /// Whether a form's walk assigns a local, directly or in a part: its value is not memoized,
    /// since a memoized value would hide the assignment on the next visit.
    pub(super) fn assigns(kind: &Expr) -> bool {
        match kind {
            Expr::If { .. } => true,
            Expr::Block { stmts, .. } => !stmts.is_empty(),
            _ => false,
        }
    }

    /// A block or a closure ends: the locals it bound (`outside` holds none of them)
    /// go out of scope, while a local bound before it keeps what the code assigned to it.
    pub(super) fn restore(&mut self, outside: Vec<Option<Value>>) {
        let scope = self.scope();
        let kept: Vec<Option<Value>> = scope.locals.iter().zip(outside).map(|(now, before)| before.and(now.clone())).collect();
        if scope.locals != kept {
            scope.memo.clear();
            scope.locals = kept;
        }
    }

    /// `local = value` or `local op= value` at `at`, the value on the stack: the local's value
    /// joins the assigned one, and the conditions the assignment sits under inside this owner.
    pub(super) fn assign(&mut self, local: u32, op: Option<BinaryOp>, at: ExprId) {
        let assigned = self.pop();
        let old = match self.scope().locals.get(local as usize) {
            Some(Some(value)) => value.clone(),
            Some(None) => self.refuse(at, &format!("local {local} assigned before its `let`")),
            None => self.refuse(at, &format!("local {local} out of range")),
        };
        let mut value = match op {
            None => assigned,
            Some(op) => Walk::binary(op, old.clone(), assigned),
        };
        value.join(&old);
        let from = self.scope().enclosed_from;
        let conditions = self.enclosing.get(from..).unwrap_or_default().to_vec();
        value.under(&conditions);
        self.bind(local, value, at);
    }
}
