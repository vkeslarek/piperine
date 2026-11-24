//! `Body::contributions`: every contribution of a body's `analog` block with the conditions that
//! enclose it, in source order (analog-structure, mechanism 1).
//!
//! A contribution may sit in the arms of an `if` (a `match` lowers to a chain of them) and in a
//! nested block, anywhere inside the `analog` block but in a closure; the *selector* of a
//! contribution is the conjunction of the conditions of the arms that enclose it (§11). The IR
//! does not store a selector: this one walk reads it off the structure, and the plan, the
//! printer and the interpreter share it. The walk is iterative, so nesting depth costs no native
//! stack, and skips an id out of range: `Ir::check` names it. An expression or a statement
//! shared by two parents is walked once, at its first place (a shared sum can have exponentially
//! many paths, as `deps` knows).

use std::collections::HashSet;

use crate::ids::{ExprId, StmtId};
use crate::model::{Addend, Expr, Ir, QuantityRole, Stmt};

/// One enclosing condition of a contribution: the `if` condition `expr`, and whether the
/// contribution sits in the arm taken when it holds (`then`) or when it does not (`else`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    pub expr: ExprId,
    pub holds: bool,
}

/// A contribution statement of the `analog` block and the conditions that enclose it, outermost
/// first. An empty list is an unconditional contribution.
#[derive(Debug, Clone, PartialEq)]
pub struct Contribution<'a> {
    pub stmt: StmtId,
    pub branch: u32,
    pub quantity: QuantityRole,
    pub addends: &'a [Addend],
    pub conditions: Vec<Condition>,
}

/// One step of the walk.
enum Step {
    Stmt(StmtId),
    Expr(ExprId),
    /// A contribution whose operands are walked: it is listed now.
    List(StmtId),
    /// The walk enters the arm of an `if` under `condition`.
    Enter(Condition),
    /// The walk leaves the arm it entered last.
    Leave,
}

/// The walk over the statements of one `analog` block.
pub(crate) struct Contributions<'a> {
    ir: &'a Ir,
}

impl<'a> Contributions<'a> {
    /// A walk over the statements of `ir`.
    pub(crate) fn new(ir: &'a Ir) -> Self {
        Contributions { ir }
    }

    /// The contributions among `stmts` and among what they hold, in source order.
    pub(crate) fn of(&self, stmts: &[StmtId]) -> Vec<Contribution<'a>> {
        let mut out = Vec::new();
        let mut conditions: Vec<Condition> = Vec::new();
        let mut seen_stmts: HashSet<StmtId> = HashSet::new();
        let mut seen_exprs: HashSet<ExprId> = HashSet::new();
        let mut stack: Vec<Step> = stmts.iter().rev().map(|s| Step::Stmt(*s)).collect();
        while let Some(step) = stack.pop() {
            let mut next: Vec<Step> = Vec::new();
            match step {
                Step::Enter(condition) => conditions.push(condition),
                Step::Leave => {
                    conditions.pop();
                }
                Step::List(id) => {
                    if let Some(Stmt::Contribute { branch, quantity, addends }) = self.ir.stmts.get(id.index()) {
                        out.push(Contribution { stmt: id, branch: *branch, quantity: *quantity, addends, conditions: conditions.clone() });
                    }
                }
                Step::Stmt(id) => {
                    let Some(stmt) = self.ir.stmts.get(id.index()) else { continue };
                    if !seen_stmts.insert(id) {
                        continue;
                    }
                    next.extend(stmt.operands().into_iter().map(Step::Expr));
                    if matches!(stmt, Stmt::Contribute { .. }) {
                        next.push(Step::List(id));
                    }
                }
                Step::Expr(id) => {
                    let Some(data) = self.ir.exprs.get(id.index()) else { continue };
                    if !seen_exprs.insert(id) {
                        continue;
                    }
                    match &data.kind {
                        Expr::If { cond, then, otherwise } => {
                            next.push(Step::Expr(*cond));
                            for (arm, holds) in [(*then, true), (*otherwise, false)] {
                                next.extend([Step::Enter(Condition { expr: *cond, holds }), Step::Expr(arm), Step::Leave]);
                            }
                        }
                        Expr::Block { stmts, value } => {
                            next.extend(stmts.iter().map(|s| Step::Stmt(*s)));
                            next.extend(value.iter().map(|v| Step::Expr(*v)));
                        }
                        // A loop's statements are its own (`Ir::check` refuses a contribution
                        // in one); only its operands are walked here.
                        other => next.extend(Contributions::operands(other).into_iter().map(Step::Expr)),
                    }
                }
            }
            // `next` holds the follow-up steps in evaluation order.
            stack.extend(next.into_iter().rev());
        }
        out
    }

    /// The operand expressions of a form other than an `if` or a block. A closure's body and a
    /// callee's body are not the analog block's own statements: a contribution there is refused
    /// by `Ir::check`, so the walk does not enter them.
    fn operands(kind: &Expr) -> Vec<ExprId> {
        match kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Probe { .. } | Expr::Env(_) => Vec::new(),
            Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::ParamPrevious(_) | Expr::Others(_) | Expr::Resolve { .. } => Vec::new(),
            Expr::Closure { .. } | Expr::If { .. } | Expr::Block { .. } => Vec::new(),
            Expr::Unwrap { is, value, .. } => vec![*is, *value],
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => vec![*expr],
            Expr::LevelCast { expr, .. } => vec![*expr],
            Expr::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
            Expr::Same { a, b } => vec![*a, *b],
            Expr::Call { args, .. } | Expr::Math { args, .. } => args.clone(),
            Expr::Limit { x, f, seed, .. } => [*x, *f].into_iter().chain(*seed).collect(),
            Expr::Stim { mag, phase, .. } => [*mag].into_iter().chain(*phase).collect(),
            Expr::SessionParameter { .. } => Vec::new(),
            Expr::Method { receiver, args, .. } => [*receiver].into_iter().chain(args.iter().copied()).collect(),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => kind.operands(),
        }
    }
}
