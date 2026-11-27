//! The statements of the walk: what each `Stmt` form schedules.

use super::{Task, Walk};
use crate::ids::{ExprId, StmtId};
use crate::model::{EmitField, Stmt};

impl<'a> Walk<'a> {
    /// Schedules the tasks of statement `id`; one arm per `Stmt` form.
    pub(super) fn stmt(&mut self, id: StmtId, next: &mut Vec<Task>) {
        let ir = self.ir;
        match &ir.stmts[id.index()] {
            Stmt::Let { local, init } => next.extend([Task::Expr(*init), Task::Bind(*local, *init)]),
            Stmt::Expr(e) => next.extend([Task::Expr(*e), Task::Discard]),
            Stmt::Assign { place, op, value } => {
                next.push(Task::Expr(*value));
                let indices: Vec<ExprId> = place.index_operands().collect();
                next.extend(indices.iter().map(|&index| Task::Expr(index)));
                next.push(Task::Assign { local: place.local, op: *op, at: *value, indices: indices.len() });
            }
            Stmt::BoundStep { site, value, when } => {
                next.extend([Task::Expr(*value), Task::Cap(*site)]);
                // What decides whether the call runs is read by the same site.
                if let Some(when) = when {
                    next.extend([Task::Expr(*when), Task::Cap(*site)]);
                }
            }
            Stmt::Contribute { branch, addends, .. } => {
                for addend in addends {
                    next.push(Task::Expr(addend.value));
                    next.push(Task::Addend { branch: *branch, operator: addend.operator, at: addend.value });
                }
            }
            Stmt::Commit { value, after, target, .. } => {
                next.extend([Task::Expr(*value), Task::Discard]);
                if let Some(after) = after {
                    next.extend([Task::Expr(*after), Task::Discard]);
                }
                // the element and the value-array indexes of the target are read too (an element commit at a run-time index)
                for index in target.iter().flat_map(crate::model::CommitTarget::index_operands) {
                    next.extend([Task::Expr(index), Task::Discard]);
                }
            }
            Stmt::Emit { fields, .. } => {
                for f in fields {
                    if let EmitField::Value { value, .. } = f {
                        next.extend([Task::Expr(*value), Task::Discard]);
                    }
                }
            }
            Stmt::Trace { site, fields } => {
                for f in fields {
                    if let EmitField::Value { value, .. } = f {
                        next.extend([Task::Expr(*value), Task::Trace(*site)]);
                    }
                }
            }
        }
    }
}
