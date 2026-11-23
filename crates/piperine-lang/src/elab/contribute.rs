//! A contribution `lhs +> rhs` lowered to `Contribute`: its branch (one per unordered terminal
//! pair, oriented by its first contribution) and the right side split into addends (abi.md A1).
//!
//! The split flattens the top-level chain of `+`, `-` and unary `-` of a contribution into
//! signed leaves, parentheses transparent. Only a chain that holds a `ddt` leaf is flattened: a
//! sub-sum with no `ddt` in it is one leaf, kept as written, so no floating-point sum is
//! re-associated. Each `ddt(x)` leaf is its own addend (value `x`); each maximal run of other leaves
//! is one direct addend, re-joined in source order. A `ddt` that is not a whole addend (off the
//! top of the chain, under a condition, with an `abstol`) reaches value lowering, which makes it a
//! derivative site (`derivatives.rs`).

use piperine_ir::{Addend, BinaryOp, Branch, BranchKind, Expr, ExprId as LoweredExpr, Operator, QuantityRole, Stmt, StmtId as LoweredStmt, Terminal, ValueType};

use super::elaborator::Elaboration;
use super::derivatives::LeafRules;
use super::lower::Scope;
use super::selectors::ContributionRecord;
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
        let Some((role, plus, minus)) = self.contribution_target(scope, lhs) else { return false };
        if plus == minus {
            let at = self.design.expr_node(lhs).span;
            self.report(scope.file, Diagnostic::new("E0241", "a contribution to a branch from a node to itself", at, "both nodes are the same node"));
            return false;
        }
        // A merged branch is no branch: its two nodes are one (`structural.rs`).
        if scope.merged.iter().any(|(a, b)| (*a, *b) == (plus, minus) || (*a, *b) == (minus, plus)) {
            return true;
        }
        // The branch's kind is decided once every contribution is lowered (`selectors.rs`); until
        // then it is the kind of the first contribution.
        let kind = match role {
            QuantityRole::Flow => BranchKind::Flow,
            QuantityRole::Potential => BranchKind::Potential,
        };
        let existing = scope.branches.iter().position(|b| (b.plus, b.minus) == (plus, minus) || (b.plus, b.minus) == (minus, plus));
        let (branch, reversed) = match existing {
            Some(index) => (index as u32, scope.branches[index].plus != plus),
            None => {
                scope.branches.push(Branch { plus, minus, kind });
                (scope.branches.len() as u32 - 1, false)
            }
        };
        scope.contributions.push(ContributionRecord { branch, quantity: role, conditions: scope.conditions.clone(), file: scope.file, span });
        // A potential contribution is split like a flow one: its `ddt` leaves are `Ddt` addends
        // (amendment A5; abi.md A1 hoists them onto the branch's equation row).
        let mut leaves = Vec::new();
        let rules = LeafRules { whole_addends: scope.conditions.is_empty(), lets: &scope.addend_lets.clone() };
        self.flatten(&rules, rhs, false, &mut leaves);
        let Some(addends) = self.addends(scope, &leaves, reversed) else { return false };
        out.push(self.stmt(Stmt::Contribute { branch, quantity: role, addends }));
        true
    }

    /// The quantity and the terminals the left side names: `i(a, b)`, `v(a)`, `flow(a, b)`.
    fn contribution_target(&mut self, scope: &Scope, lhs: ExprId) -> Option<(QuantityRole, Terminal, Terminal)> {
        let node = self.design.expr_node(lhs).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else {
            self.unsupported(scope.file, node.span, "a contribution target that is not a probe");
            return None;
        };
        let Some(role) = self.quantity_role(*callee) else {
            self.unsupported(scope.file, node.span, "a contribution target that is not a probe");
            return None;
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

    /// The quantity the call of `callee` reads or contributes: a nature's access alias (`i`, `v`)
    /// or the built-in `flow` and `potential`; `None` for anything else.
    pub(super) fn quantity_role(&self, callee: ExprId) -> Option<pom::QuantityRole> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        match self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            Resolution::Decl(q) => match &self.design.decl_data(q).kind {
                DeclKind::Quantity(data) => Some(data.role),
                _ => None,
            },
            Resolution::Builtin(id) if id.info().name == "flow" => Some(pom::QuantityRole::Flow),
            Resolution::Builtin(id) if id.info().name == "potential" => Some(pom::QuantityRole::Potential),
            _ => None,
        }
    }

    /// The signed leaves of `expr`, negated when `negated`, appended in source order.
    fn flatten(&self, rules: &LeafRules, expr: ExprId, negated: bool, out: &mut Vec<Leaf>) {
        let inner = self.peel_parens(expr);
        if let Some(call) = self.whole_addend(rules, inner) {
            out.push(Leaf { negated, expr: call, ddt: true });
            return;
        }
        if !self.holds_ddt_leaf(rules, inner) {
            out.push(Leaf { negated, expr: inner, ddt: false });
            return;
        }
        match &self.design.expr_node(inner).kind {
            ExprKind::Binary { op: BinOp::Add, lhs, rhs } => {
                self.flatten(rules, *lhs, negated, out);
                self.flatten(rules, *rhs, negated, out);
            }
            ExprKind::Binary { op: BinOp::Sub, lhs, rhs } => {
                self.flatten(rules, *lhs, negated, out);
                self.flatten(rules, *rhs, !negated, out);
            }
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => self.flatten(rules, *operand, !negated, out),
            // `holds_ddt_leaf` is true only through the three forms above.
            _ => out.push(Leaf { negated, expr: inner, ddt: false }),
        }
    }

    /// Whether `expr` has a `ddt` call among the leaves of its top-level chain of `+`, `-` and
    /// unary `-`.
    fn holds_ddt_leaf(&self, rules: &LeafRules, expr: ExprId) -> bool {
        let inner = self.peel_parens(expr);
        if self.whole_addend(rules, inner).is_some() {
            return true;
        }
        match &self.design.expr_node(inner).kind {
            ExprKind::Binary { op: BinOp::Add | BinOp::Sub, lhs, rhs } => self.holds_ddt_leaf(rules, *lhs) || self.holds_ddt_leaf(rules, *rhs),
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => self.holds_ddt_leaf(rules, *operand),
            _ => false,
        }
    }

    /// Whether `expr` is a call of the core module `ddt`.
    pub(super) fn is_ddt(&self, expr: ExprId) -> bool {
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

    /// The argument of a whole-addend `ddt` leaf: a call with one argument (`whole_addend`).
    fn ddt_value(&mut self, scope: &mut Scope, ddt: ExprId) -> Option<LoweredExpr> {
        let node = self.design.expr_node(ddt).clone();
        let ExprKind::Call { args, .. } = &node.kind else { return None };
        let [x] = &args[..] else { return None };
        self.lower_expr(scope, x.value)
    }

    /// One run of non-`ddt` leaves re-joined in order, signed relative to its first leaf: the
    /// addend carries the first leaf's sign, a leaf of the same sign is added, of the other
    /// subtracted.
    fn direct_run(&mut self, scope: &mut Scope, run: &[Leaf]) -> Option<LoweredExpr> {
        let first = run[0];
        let origin = self.expr_origin(first.expr);
        let mut value = self.lower_expr(scope, first.expr);
        for leaf in &run[1..] {
            let rhs = self.lower_expr(scope, leaf.expr);
            let op = if leaf.negated == first.negated { BinaryOp::Add } else { BinaryOp::Sub };
            value = match (value, rhs) {
                (Some(lhs), Some(rhs)) => Some(self.emit(Expr::Binary { op, lhs, rhs }, ValueType::F64, origin.clone())),
                _ => None,
            };
        }
        value
    }
}
