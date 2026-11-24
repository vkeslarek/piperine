//! The statements of a reaction that write the outside, in the order the ABI numbers them: a
//! depth-first walk in source order, each expression's operands in order, a block's statements
//! before its value, `then` before `else`, and a commit or an emit after the operands it
//! evaluates. A commit and an emit count together (abi.md: one statement per write).
//!
//! The walk is iterative, so nesting depth costs no native stack, and skips an id out of range:
//! `Ir::check` names it.

use crate::ids::{ExprId, StmtId};
use crate::model::{CommitTarget, EmitField, Expr, Ir, Stmt};

/// One step of the walk.
enum Step {
    Stmt(StmtId),
    Expr(ExprId),
    /// A commit or an emit whose operands are walked: it counts now.
    Count(StmtId),
}

pub(crate) struct Statements;

impl Statements {
    /// The commits and emits among `stmts`, and among what they hold, in ABI order.
    pub(crate) fn of(ir: &Ir, stmts: &[StmtId]) -> Vec<StmtId> {
        let mut out = Vec::new();
        let mut stack: Vec<Step> = stmts.iter().rev().map(|s| Step::Stmt(*s)).collect();
        while let Some(step) = stack.pop() {
            let mut next: Vec<Step> = Vec::new();
            match step {
                Step::Count(id) => out.push(id),
                Step::Stmt(id) => match ir.stmts.get(id.index()) {
                    Some(Stmt::Let { init, .. } | Stmt::Assign { value: init, .. }) => next.push(Step::Expr(*init)),
                    Some(Stmt::Expr(e) | Stmt::BoundStep { value: e, .. }) => next.push(Step::Expr(*e)),
                    Some(Stmt::Contribute { addends, .. }) => next.extend(addends.iter().map(|a| Step::Expr(a.value))),
                    Some(Stmt::Commit { value, after, target, .. }) => {
                        next.extend([*value].into_iter().chain(*after).chain(target.iter().flat_map(CommitTarget::index_operands)).map(Step::Expr));
                        next.push(Step::Count(id));
                    }
                    Some(Stmt::Emit { fields, .. }) => {
                        next.extend(fields.iter().filter_map(|f| match f {
                            EmitField::Value { value, .. } => Some(Step::Expr(*value)),
                            EmitField::Text { .. } => None,
                        }));
                        next.push(Step::Count(id));
                    }
                    // a trace is counted too: the `analog` block holds no reaction statement, so the count of
                    // its statements finds each site's text (`Body::trace_texts`)
                    Some(Stmt::Trace { fields, .. }) => {
                        next.extend(fields.iter().filter_map(|f| match f {
                            EmitField::Value { value, .. } => Some(Step::Expr(*value)),
                            EmitField::Text { .. } => None,
                        }));
                        next.push(Step::Count(id));
                    }
                    None => {}
                },
                Step::Expr(id) => {
                    let Some(data) = ir.exprs.get(id.index()) else { continue };
                    match &data.kind {
                        Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Probe { .. } | Expr::Env(_) => {}
                        Expr::Int(_) | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::ParamPrevious(_) | Expr::Others(_) | Expr::Resolve { .. } => {}
                        Expr::Unwrap { is, value, .. } => next.extend([Step::Expr(*is), Step::Expr(*value)]),
                        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => next.push(Step::Expr(*expr)),
                        Expr::Binary { lhs, rhs, .. } => next.extend([Step::Expr(*lhs), Step::Expr(*rhs)]),
                        Expr::Same { a, b } => next.extend([Step::Expr(*a), Step::Expr(*b)]),
                        Expr::LevelCast { expr, .. } => next.push(Step::Expr(*expr)),
                        Expr::If { cond, then, otherwise } => next.extend([Step::Expr(*cond), Step::Expr(*then), Step::Expr(*otherwise)]),
                        Expr::Call { args, .. } | Expr::Math { args, .. } => next.extend(args.iter().map(|a| Step::Expr(*a))),
                        Expr::Closure { body, .. } => next.push(Step::Expr(*body)),
                        // A loop body is walked once: a commit in a loop is one statement (§12.1.14).
                        Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => {
                            next.extend(data.kind.operands().into_iter().map(Step::Expr));
                            next.extend(data.kind.statements().iter().map(|s| Step::Stmt(*s)));
                        }
                        Expr::Limit { x, f, seed, .. } => next.extend([*x, *f].into_iter().chain(*seed).map(Step::Expr)),
                        Expr::Stim { mag, phase, .. } => next.extend([*mag].into_iter().chain(*phase).map(Step::Expr)),
                        Expr::SessionParameter { .. } => {}
                        Expr::Method { receiver, args, .. } => next.extend([*receiver].into_iter().chain(args.iter().copied()).map(Step::Expr)),
                        Expr::Block { stmts, value } => {
                            next.extend(stmts.iter().map(|s| Step::Stmt(*s)));
                            next.extend(value.iter().map(|v| Step::Expr(*v)));
                        }
                    }
                }
            }
            // `next` holds the follow-up steps in evaluation order.
            stack.extend(next.into_iter().rev());
        }
        out
    }
}
