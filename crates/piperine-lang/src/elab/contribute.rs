//! A contribution `lhs +> rhs` lowered to `Contribute`: its branch (one per unordered terminal
//! pair, oriented by its first contribution) and the right side split into addends (abi.md A1).
//!
//! The split flattens the top-level chain of `+`, `-` and unary `-` of a flow contribution into
//! signed leaves, parentheses transparent. Only a chain that holds a `ddt` leaf is flattened: a
//! sub-sum with no `ddt` in it is one leaf, kept as written, so no floating-point sum is
//! re-associated. Each `ddt(x)` leaf is its own addend (value `x`); each maximal run of other leaves
//! is one direct addend, re-joined in source order. A `ddt` anywhere else reaches value lowering,
//! which refuses it.

use piperine_ir::{Addend, BinaryOp, Branch, BranchKind, Expr, ExprId as LoweredExpr, Operator, QuantityRole, Stmt, StmtId as LoweredStmt, Terminal, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use super::unlowered::Unlowered;
use crate::msl::{Diagnostic, Span};
use crate::pom::{self, BinOp, DeclKind, ExprId, ExprKind, Resolution, UnOp};
use crate::types::Callee;

/// A signed leaf of the top-level chain of a contribution's right side.
#[derive(Clone, Copy)]
struct Leaf {
    negated: bool,
    expr: ExprId,
    ddt: bool,
}

impl Elaboration<'_, '_> {
    /// A contribution statement `lhs +> rhs`, appended to `out`; `false` after a diagnostic.
    pub(super) fn lower_contribution(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, lhs: ExprId, rhs: ExprId, span: Span) -> bool {
        if scope.nesting > 0 {
            self.refuse(scope.file, span, Unlowered::ConditionalContribution);
            return false;
        }
        if scope.block_depth > 0 {
            self.refuse(scope.file, span, Unlowered::NestedContribution);
            return false;
        }
        let Some((role, plus, minus)) = self.contribution_target(scope, lhs) else { return false };
        if plus == minus {
            let at = self.design.expr_node(lhs).span;
            self.report(scope.file, Diagnostic::new("E0241", "a contribution to a branch from a node to itself", at, "both nodes are the same node"));
            return false;
        }
        let kind = match role {
            QuantityRole::Flow => BranchKind::Flow,
            QuantityRole::Potential => BranchKind::Potential,
        };
        let existing = scope.branches.iter().position(|b| (b.plus, b.minus) == (plus, minus) || (b.plus, b.minus) == (minus, plus));
        let (branch, reversed) = match existing {
            Some(index) if scope.branches[index].kind != kind => {
                let message = "a branch receives both potential and flow contributions";
                self.report(scope.file, Diagnostic::new("E0415", message, span, "a contribution of the other kind to this branch"));
                return false;
            }
            Some(index) => (index as u32, scope.branches[index].plus != plus),
            None => {
                scope.branches.push(Branch { plus, minus, kind });
                (scope.branches.len() as u32 - 1, false)
            }
        };
        let mut leaves = Vec::new();
        match role {
            QuantityRole::Flow => self.flatten(rhs, false, &mut leaves),
            // `ddt` in a potential contribution needs the hoisting of abi.md A1 (M2): the whole
            // right side is one leaf, and value lowering refuses a `ddt` in it.
            QuantityRole::Potential => leaves.push(Leaf { negated: false, expr: rhs, ddt: false }),
        }
        let Some(addends) = self.addends(scope, &leaves, reversed) else { return false };
        out.push(self.ir.stmt(Stmt::Contribute { branch, quantity: role, addends }));
        true
    }

    /// The quantity and the terminals the left side names: `i(a, b)`, `v(a)`, `flow(a, b)`.
    fn contribution_target(&mut self, scope: &Scope, lhs: ExprId) -> Option<(QuantityRole, Terminal, Terminal)> {
        let node = self.design.expr_node(lhs).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else {
            self.unsupported(scope.file, node.span, "a contribution target that is not a probe");
            return None;
        };
        let ExprKind::Path(path) = &self.design.expr_node(*callee).kind else {
            self.unsupported(scope.file, node.span, "a contribution target that is not a probe");
            return None;
        };
        let role = match self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            Resolution::Decl(q) => match &self.design.decl_data(q).kind {
                DeclKind::Quantity(data) => data.role,
                _ => {
                    self.unsupported(scope.file, node.span, "a contribution target that is not a probe");
                    return None;
                }
            },
            Resolution::Builtin(id) if id.info().name == "flow" => pom::QuantityRole::Flow,
            Resolution::Builtin(id) if id.info().name == "potential" => pom::QuantityRole::Potential,
            _ => {
                self.unsupported(scope.file, node.span, "a contribution target that is not a probe");
                return None;
            }
        };
        let role = match role {
            pom::QuantityRole::Flow => QuantityRole::Flow,
            pom::QuantityRole::Potential => QuantityRole::Potential,
        };
        let mut terminals = Vec::new();
        for arg in args {
            terminals.push(self.lower_terminal(scope, arg.value)?);
        }
        match terminals[..] {
            [plus] => Some((role, plus, Terminal::Ground)),
            [plus, minus] => Some((role, plus, minus)),
            _ => {
                self.unsupported(scope.file, node.span, "a contribution to a probe of more than two nodes");
                None
            }
        }
    }

    /// The signed leaves of `expr`, negated when `negated`, appended in source order.
    fn flatten(&self, expr: ExprId, negated: bool, out: &mut Vec<Leaf>) {
        let inner = self.peel_parens(expr);
        if self.is_ddt(inner) {
            out.push(Leaf { negated, expr: inner, ddt: true });
            return;
        }
        if !self.holds_ddt_leaf(inner) {
            out.push(Leaf { negated, expr: inner, ddt: false });
            return;
        }
        match &self.design.expr_node(inner).kind {
            ExprKind::Binary { op: BinOp::Add, lhs, rhs } => {
                self.flatten(*lhs, negated, out);
                self.flatten(*rhs, negated, out);
            }
            ExprKind::Binary { op: BinOp::Sub, lhs, rhs } => {
                self.flatten(*lhs, negated, out);
                self.flatten(*rhs, !negated, out);
            }
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => self.flatten(*operand, !negated, out),
            // `holds_ddt_leaf` is true only through the three forms above.
            _ => out.push(Leaf { negated, expr: inner, ddt: false }),
        }
    }

    /// Whether `expr` has a `ddt` call among the leaves of its top-level chain of `+`, `-` and
    /// unary `-`.
    fn holds_ddt_leaf(&self, expr: ExprId) -> bool {
        let inner = self.peel_parens(expr);
        if self.is_ddt(inner) {
            return true;
        }
        match &self.design.expr_node(inner).kind {
            ExprKind::Binary { op: BinOp::Add | BinOp::Sub, lhs, rhs } => self.holds_ddt_leaf(*lhs) || self.holds_ddt_leaf(*rhs),
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => self.holds_ddt_leaf(*operand),
            _ => false,
        }
    }

    /// Whether `expr` is a call of the core module `ddt`.
    fn is_ddt(&self, expr: ExprId) -> bool {
        matches!(self.design.expr_node(expr).kind, ExprKind::Call { .. }) && matches!(self.types.callee(expr), Some(Callee::Core(id)) if id.info().name == "ddt")
    }

    /// The addends of `leaves`: one per `ddt` leaf, one per maximal run of the others. Every
    /// addend is negated once more when the contribution names its branch reversed.
    fn addends(&mut self, scope: &mut Scope, leaves: &[Leaf], reversed: bool) -> Option<Vec<Addend>> {
        let mut addends = Vec::new();
        let mut ok = true;
        let mut start = 0;
        while start < leaves.len() {
            let leaf = leaves[start];
            if leaf.ddt {
                match self.ddt_value(scope, leaf.expr) {
                    Some(value) => addends.push(Addend { operator: Operator::Ddt, negated: leaf.negated ^ reversed, value }),
                    None => ok = false,
                }
                start += 1;
                continue;
            }
            let end = leaves[start..].iter().position(|l| l.ddt).map_or(leaves.len(), |k| start + k);
            match self.direct_run(scope, &leaves[start..end]) {
                Some(value) => addends.push(Addend { operator: Operator::Direct, negated: leaf.negated ^ reversed, value }),
                None => ok = false,
            }
            start = end;
        }
        ok.then_some(addends)
    }

    /// The argument of a `ddt` leaf; a second argument (a tolerance) has no place in the IR.
    fn ddt_value(&mut self, scope: &mut Scope, ddt: ExprId) -> Option<LoweredExpr> {
        let node = self.design.expr_node(ddt).clone();
        let ExprKind::Call { args, .. } = &node.kind else { return None };
        match &args[..] {
            [x] => {
                let value = self.lower_expr(scope, x.value)?;
                self.known_addend(scope, x.value, value)
            }
            _ => {
                self.refuse(scope.file, node.span, Unlowered::DdtTolerance);
                None
            }
        }
    }

    /// One run of non-`ddt` leaves re-joined in order, signed relative to its first leaf: the
    /// addend carries the first leaf's sign, a leaf of the same sign is added, of the other
    /// subtracted.
    fn direct_run(&mut self, scope: &mut Scope, run: &[Leaf]) -> Option<LoweredExpr> {
        let first = run[0];
        let origin = self.expr_origin(first.expr);
        let mut value = self.lower_expr(scope, first.expr).and_then(|v| self.known_addend(scope, first.expr, v));
        for leaf in &run[1..] {
            let rhs = self.lower_expr(scope, leaf.expr).and_then(|v| self.known_addend(scope, leaf.expr, v));
            let op = if leaf.negated == first.negated { BinaryOp::Add } else { BinaryOp::Sub };
            value = match (value, rhs) {
                (Some(lhs), Some(rhs)) => Some(self.emit(Expr::Binary { op, lhs, rhs }, ValueType::F64, origin.clone())),
                _ => None,
            };
        }
        value
    }

    /// `value`, the lowering of the leaf `leaf`, when it is known: the solver computes with known
    /// reals (§3.3.5), so a value that may be unknown is `P0001` at its leaf.
    fn known_addend(&mut self, scope: &Scope, leaf: ExprId, value: LoweredExpr) -> Option<LoweredExpr> {
        if self.flag(value) == Flag::Flagged {
            let span = self.design.expr_node(leaf).span;
            self.unsupported(scope.file, span, "a value that may be unknown in a contribution");
            return None;
        }
        Some(value)
    }
}
