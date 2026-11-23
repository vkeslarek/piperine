//! Diverging expressions (§1.5.16): `break`, `continue` and `return` have type `!`, which fits
//! wherever a value is expected (`let v = match o { Some(x) => x, None => return 0.0 }`). The IR
//! types every expression, so a diverging one is built again at the type its position needs: the
//! forms that hold one (a block ending in it, an `if` whose arms both end in it) are remembered
//! with the parts they were made of.

use std::collections::BTreeMap;

use piperine_ir::{Expr, ExprId as LoweredExpr, Stmt, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;

/// How a diverging expression was built.
#[derive(Clone)]
enum Divergence {
    /// The exit itself: its `break`, `continue` or `return` node.
    Exit(Expr),
    /// A block whose last expression diverges; `stmts` are those before it.
    Block { stmts: Vec<LoweredStmt>, tail: LoweredExpr },
    /// An `if` whose two arms both diverge.
    Branch { cond: LoweredExpr, then: LoweredExpr, otherwise: LoweredExpr },
}

/// The diverging expressions lowered so far, by id, and the statements that are one.
#[derive(Default)]
pub(super) struct Divergences {
    expressions: BTreeMap<usize, Divergence>,
    /// A statement `exit;` by id: the expression it holds.
    statements: BTreeMap<usize, LoweredExpr>,
}

impl Divergences {
    /// Whether `expr` diverges.
    pub(super) fn diverges(&self, expr: LoweredExpr) -> bool {
        self.expressions.contains_key(&expr.index())
    }

    /// Remembers that statement `stmt` is `Stmt::Expr(expr)` of a diverging expression.
    pub(super) fn note_statement(&mut self, stmt: LoweredStmt, statement: &Stmt) {
        if let Stmt::Expr(expr) = statement
            && self.diverges(*expr)
        {
            self.statements.insert(stmt.index(), *expr);
        }
    }
}

impl Elaboration<'_, '_> {
    /// A `break`, `continue` or `return` node `exit`, typed `()` until a position asks for more.
    pub(super) fn emit_exit(&mut self, exit: Expr, origin: piperine_ir::Origin) -> LoweredExpr {
        let lowered = self.emit(exit.clone(), ValueType::Unit, origin);
        self.divergences.expressions.insert(lowered.index(), Divergence::Exit(exit));
        lowered
    }

    /// Remembers that `expr` diverges and was made of `exit`.
    pub(super) fn note_exit(&mut self, expr: LoweredExpr, exit: Expr) {
        self.divergences.expressions.insert(expr.index(), Divergence::Exit(exit));
    }

    /// Remembers the block `block`, made of `stmts` and `value`, when it diverges: its value does,
    /// or it has none and its last statement is a diverging expression.
    pub(super) fn note_block(&mut self, block: LoweredExpr, stmts: &[LoweredStmt], value: Option<LoweredExpr>) {
        let form = match (value, stmts.split_last()) {
            (Some(tail), _) if self.divergences.diverges(tail) => Divergence::Block { stmts: stmts.to_vec(), tail },
            (None, Some((last, before))) => match self.divergences.statements.get(&last.index()) {
                Some(tail) => Divergence::Block { stmts: before.to_vec(), tail: *tail },
                None => return,
            },
            _ => return,
        };
        self.divergences.expressions.insert(block.index(), form);
    }

    /// Remembers the `if` `whole` when both its arms diverge.
    pub(super) fn note_branch(&mut self, whole: LoweredExpr, cond: LoweredExpr, then: LoweredExpr, otherwise: LoweredExpr) {
        if self.divergences.diverges(then) && self.divergences.diverges(otherwise) {
            self.divergences.expressions.insert(whole.index(), Divergence::Branch { cond, then, otherwise });
        }
    }

    /// `expr` at the type `ty`: itself when it has it or does not diverge, else its form built
    /// again with `ty`.
    pub(super) fn fit_to(&mut self, expr: LoweredExpr, ty: &ValueType) -> LoweredExpr {
        if self.type_of(expr) == *ty {
            return expr;
        }
        let Some(form) = self.divergences.expressions.get(&expr.index()).cloned() else { return expr };
        let origin = self.expr_origins[expr.index()].clone();
        match form {
            Divergence::Exit(exit) => self.emit(exit, ty.clone(), origin),
            Divergence::Block { stmts, tail } => {
                let tail = self.fit_to(tail, ty);
                self.emit(Expr::Block { stmts, value: Some(tail) }, ty.clone(), origin)
            }
            Divergence::Branch { cond, then, otherwise } => {
                let (then, otherwise) = (self.fit_to(then, ty), self.fit_to(otherwise, ty));
                self.emit(Expr::If { cond, then, otherwise }, ty.clone(), origin)
            }
        }
    }
}
