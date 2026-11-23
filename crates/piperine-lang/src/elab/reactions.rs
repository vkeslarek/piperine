//! Reaction bodies (part IV §12.1, DIG-09 AC 10): a `digital` block's reaction body is lowered
//! statement by statement in source order, `let`, `if`, calls and probes as value code, and
//! each commit `place <- value [after d];` as a `Commit` of its carrier with its `after`. A
//! commit inside an `if` is a statement of the `if`'s block.

use piperine_ir::{Stmt, StmtId as LoweredStmt};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::ExprId;

impl Elaboration<'_, '_> {
    /// `place <- value [after delay];`, appended to `out`; `false` after a diagnostic. The
    /// checker allows a commit in a reaction body only.
    pub(super) fn lower_commit(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, place: ExprId, value: ExprId, after: Option<ExprId>) -> bool {
        let carrier = self.drive_target(scope, place);
        let wanted = carrier.and_then(|c| self.structure.carriers.get(c as usize)).map(|c| c.ty.clone());
        let value = match &wanted {
            Some(ty) => self.lower_stored(scope, value, ty),
            None => self.lower_expr(scope, value),
        };
        let after = match after {
            Some(after) => self.lower_expr(scope, after).map(Some),
            None => Some(None),
        };
        match (carrier, value, after) {
            (Some(carrier), Some(value), Some(after)) => {
                out.push(self.stmt(Stmt::Commit { carrier, value, after }));
                true
            }
            _ => false,
        }
    }
}
