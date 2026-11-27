//! The walks over the bodies of a design: the expression tree below a node, and the
//! statements nested in a block.

use super::body::{BlockNode, ExprKind, StmtKind};
use super::design::Design;
use super::ids::{ExprId, StmtId};

/// The walks over the expressions and statements of one design.
pub(crate) struct BodyWalk<'design> {
    design: &'design Design,
}

impl<'design> BodyWalk<'design> {
    /// The walks over `design`.
    pub(crate) fn new(design: &'design Design) -> Self {
        BodyWalk { design }
    }

    /// The direct sub-expressions of an expression, in source order: the one
    /// enumeration of an `ExprKind`'s children that [`Expr::children`], the
    /// parent table and `check()` all read. A form that holds a block lists the
    /// expressions of the block's statements (see [`BodyWalk::statement_children`]) where the
    /// block was written. Types, patterns and generic arguments are not
    /// expressions and are not entered.
    pub(crate) fn expression_children(&self, kind: &ExprKind) -> Vec<ExprId> {
        let mut out = Vec::new();
        match kind {
            ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Continue | ExprKind::Error | ExprKind::IncludeStr(_) => {}
            ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) | ExprKind::Try(expr) | ExprKind::Cast { expr, .. } | ExprKind::Field { expr, .. } => out.push(*expr),
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Assign { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
            ExprKind::Call { callee, args, params, .. } => {
                out.push(*callee);
                out.extend(args.iter().map(|a| a.value));
                out.extend(params.iter().flatten().map(|f| f.value));
            }
            ExprKind::MethodCall { recv, args, .. } => {
                out.push(*recv);
                out.extend(args.iter().map(|a| a.value));
            }
            ExprKind::Index { expr, index } => out.extend([*expr, *index]),
            ExprKind::Range { lo, hi, .. } => out.extend(lo.iter().chain(hi)),
            ExprKind::Tuple(items) | ExprKind::Array(items) => out.extend(items),
            ExprKind::ArrayRepeat { elem, len } => out.extend([*elem, *len]),
            ExprKind::Struct { fields, .. } | ExprKind::Emit { fields, .. } => out.extend(fields.iter().map(|f| f.value)),
            ExprKind::Block(b) | ExprKind::Loop { body: b } => out.extend(self.block_expressions(b)),
            ExprKind::If { cond, then, else_ } => {
                out.push(*cond);
                out.extend(self.block_expressions(then));
                out.extend(else_);
            }
            ExprKind::Let { expr, .. } => out.push(*expr),
            ExprKind::Match { scrutinee, arms } => {
                out.push(*scrutinee);
                for a in arms {
                    out.extend(a.guard);
                    out.push(a.body);
                }
            }
            ExprKind::While { cond, body } => {
                out.push(*cond);
                out.extend(self.block_expressions(body));
            }
            ExprKind::For { iter, body, .. } => {
                out.push(*iter);
                out.extend(self.block_expressions(body));
            }
            ExprKind::Break(v) | ExprKind::Return(v) => out.extend(v),
            ExprKind::Closure { body, .. } => out.push(*body),
        }
        out
    }

    /// The expressions a statement holds without any expression above them: its
    /// own, and those of the statements of a `let .. else` block.
    pub(crate) fn statement_children(&self, id: StmtId) -> Vec<ExprId> {
        let mut out = self.statement_own_expressions(id);
        if let StmtKind::Let { else_: Some(b), .. } = &self.design.stmt_node(id).kind {
            out.extend(self.block_expressions(b));
        }
        out
    }

    /// The expressions a statement itself holds, in source order; the statements
    /// of a `let .. else` block hold their own.
    pub(crate) fn statement_own_expressions(&self, id: StmtId) -> Vec<ExprId> {
        match &self.design.stmt_node(id).kind {
            StmtKind::Let { init, .. } => init.iter().copied().collect(),
            StmtKind::Expr { expr, .. } => vec![*expr],
            StmtKind::Contribution { lhs, rhs } => vec![*lhs, *rhs],
            StmtKind::Commit { place, value, after } => [*place, *value].into_iter().chain(*after).collect(),
            StmtKind::Item(_) | StmtKind::Empty | StmtKind::Signal(_) | StmtKind::Reaction(_) => Vec::new(),
        }
    }

    /// The children of every statement of a block, in order.
    pub(crate) fn block_expressions(&self, block: &BlockNode) -> Vec<ExprId> {
        block.stmts.iter().flat_map(|s| self.statement_children(*s)).collect()
    }

    /// `id` and everything below it, pre-order.
    pub(crate) fn descendants_depth_first(&self, id: ExprId, out: &mut Vec<ExprId>) {
        crate::stack::Stack::deep(|| {
            out.push(id);
            for c in self.expression_children(&self.design.expr_node(id).kind) {
                self.descendants_depth_first(c, out);
            }
        })
    }

    /// The statements under `stmts`, each followed by those nested in it
    /// (blocks, `if`, `match`, loops), in source order. Items inside a block are
    /// declarations of their own and are not entered.
    pub(crate) fn nested_statements(&self, stmts: &[StmtId]) -> Vec<StmtId> {
        let mut out = Vec::new();
        for s in stmts {
            self.walk_statement(*s, &mut out);
        }
        out
    }

    /// Push `id` and every statement nested in it.
    fn walk_statement(&self, id: StmtId, out: &mut Vec<StmtId>) {
        out.push(id);
        match &self.design.stmt_node(id).kind {
            StmtKind::Expr { expr, .. } => self.walk_expression(*expr, out),
            StmtKind::Let { init, else_, .. } => {
                if let Some(e) = init {
                    self.walk_expression(*e, out);
                }
                if let Some(b) = else_ {
                    self.walk_block(b, out);
                }
            }
            StmtKind::Item(_)
            | StmtKind::Contribution { .. }
            | StmtKind::Commit { .. }
            | StmtKind::Empty
            | StmtKind::Signal(_)
            | StmtKind::Reaction(_) => {}
        }
    }

    /// Push every statement of `block`, each followed by those nested in it.
    fn walk_block(&self, block: &BlockNode, out: &mut Vec<StmtId>) {
        for s in &block.stmts {
            self.walk_statement(*s, out);
        }
    }

    /// Push the statements nested in the expression `id`, on a deep stack.
    fn walk_expression(&self, id: ExprId, out: &mut Vec<StmtId>) {
        crate::stack::Stack::deep(|| self.walk_expression_inner(id, out))
    }

    /// The statements nested in the expression `id`: those of its blocks, arms and parenthesised body.
    fn walk_expression_inner(&self, id: ExprId, out: &mut Vec<StmtId>) {
        match &self.design.expr_node(id).kind {
            ExprKind::Block(b) | ExprKind::Loop { body: b } | ExprKind::While { body: b, .. } | ExprKind::For { body: b, .. } => {
                self.walk_block(b, out)
            }
            ExprKind::If { then, else_, .. } => {
                self.walk_block(then, out);
                if let Some(e) = else_ {
                    self.walk_expression(*e, out);
                }
            }
            ExprKind::Match { arms, .. } => {
                for a in arms {
                    self.walk_expression(a.body, out);
                }
            }
            ExprKind::Paren(inner) => self.walk_expression(*inner, out),
            _ => {}
        }
    }
}
