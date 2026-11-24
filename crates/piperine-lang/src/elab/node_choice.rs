//! A node chosen at elaboration (§1.4.7, user decision of 2026-11-24): an `if`, a `match` or a block whose result is a node and
//! whose choice reads only params, constants and generation indexes names the node it folds to, wherever a node goes (a probe,
//! a contribution target, a connection). The params it reads are structural (`structural.rs`), so each body variant folds it to
//! one node. A choice the run makes is refused by the checker (`E0133`), and one that does not fold here (a function that reads
//! the run) is refused with the same code.

use piperine_ir::Constant;

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{BlockNode, ExprId, ExprKind, StmtKind};

impl Elaboration<'_, '_> {
    /// The node expression the choice `expr` folds to in `scope`: the arm its condition or scrutinee selects, the result of a
    /// block. `None` when `expr` is no choice; `Some(None)` after a diagnostic.
    pub(super) fn chosen_node(&mut self, scope: &Scope, expr: ExprId) -> Option<Option<ExprId>> {
        let node = self.design.expr_node(expr).clone();
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        let chosen = match &node.kind {
            ExprKind::If { cond, then, else_ } => match self.consts.value(*cond, &bindings) {
                Ok(Constant::Bool(true)) => self.block_result(then),
                Ok(Constant::Bool(false)) => *else_,
                _ => None,
            },
            ExprKind::Match { scrutinee, arms } => self.folded_arm(&bindings, &[], *scrutinee, arms).map(|position| arms[position].body),
            ExprKind::Block(block) => self.block_result(block),
            _ => return None,
        };
        if chosen.is_none() {
            // a condition that does not fold reads what only the run knows, through a function the checker cannot see into (§1.4.7)
            let message = "a node chosen by a condition that does not fold at elaboration chooses hardware at run time: a node is hardware, chosen at elaboration or named (§1.4.7)".to_string();
            self.report(scope.file, crate::msl::Diagnostic::new("E0133", message, node.span, "a node chosen at run time"));
        }
        Some(chosen)
    }

    /// The result expression of `block`: its last statement, an expression with no `;`.
    fn block_result(&self, block: &BlockNode) -> Option<ExprId> {
        let last = *block.stmts.last()?;
        match &self.design.stmt_node(last).kind {
            StmtKind::Expr { expr, semi: false } => Some(*expr),
            _ => None,
        }
    }
}
