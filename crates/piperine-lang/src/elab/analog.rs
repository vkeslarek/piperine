//! The `analog` block of a body: its statements lowered in source order. Probes, environment
//! reads and sites are lowered here; the contribution to a branch is `contribute.rs`.

use piperine_ir::{ExprId as LoweredExpr, Origin, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Span;
use crate::pom::{BehaviorKind, BuiltinId, DeclId, DeclKind, ExprId};

impl Elaboration<'_, '_> {
    /// The statements of the `analog` block bound to `module`, in source order; `None` when it
    /// has none. A second `analog` block, or a `digital` one, is `P0001`.
    pub(super) fn lower_analog(&mut self, scope: &mut Scope, module: DeclId) -> Option<Vec<LoweredStmt>> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        let blocks = data.behaviors.clone();
        let mut stmts = None;
        for block in blocks {
            let decl = self.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            if behavior.kind != BehaviorKind::Analog {
                self.unsupported(decl.file, decl.span, "a `digital` block");
                continue;
            }
            if stmts.is_some() {
                self.unsupported(decl.file, decl.span, "a second `analog` block for one module");
                continue;
            }
            // After a diagnostic the block is empty: no `Ir` is built anyway.
            let (mut lowered, value) = self.lower_statements(scope, &behavior.body).unwrap_or_default();
            // At the top of the block a trailing expression without `;` is a statement like any other.
            if let Some(value) = value {
                lowered.push(self.ir.stmt(piperine_ir::Stmt::Expr(value)));
            }
            stmts = Some(lowered);
        }
        stmts
    }

    /// A probe `v(a)` or `v(a, b)`: lowered with the terminals of its nodes.
    pub(super) fn lower_probe(&mut self, scope: &mut Scope, expr: ExprId, _ty: ValueType, _origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        self.unsupported(scope.file, span, "a probe");
        None
    }

    /// A call of a core primitive of the subset.
    pub(super) fn lower_core(&mut self, scope: &mut Scope, expr: ExprId, _id: BuiltinId, _args: &[ExprId], _ty: ValueType, _origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        self.unsupported(scope.file, span, "a core primitive");
        None
    }

    /// A contribution statement `lhs +> rhs`; `false` after a diagnostic.
    pub(super) fn lower_contribution(&mut self, scope: &mut Scope, _out: &mut Vec<LoweredStmt>, _lhs: ExprId, _rhs: ExprId, span: Span) -> bool {
        self.unsupported(scope.file, span, "a contribution");
        false
    }
}
