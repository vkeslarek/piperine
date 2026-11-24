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
        let target = self.place_target(scope, place);
        if let Some(target) = &target {
            self.record_drive_site(target, super::drive_overlaps::DriveWay::Commit, scope.file, self.design.expr_node(place).span);
        }
        let wanted = target.as_ref().map(|t| t.ty.clone());
        let value = match &wanted {
            Some(ty) => self.lower_stored(scope, value, ty),
            None => self.lower_expr(scope, value),
        };
        let after = match after {
            Some(after) => self.lower_expr(scope, after).map(Some),
            None => Some(None),
        };
        match (target, value, after) {
            (Some(target), Some(value), Some(after)) => {
                out.push(self.stmt(Stmt::Commit { carrier: target.carrier, value, after, target: target.commit_target() }));
                true
            }
            _ => false,
        }
    }
}
