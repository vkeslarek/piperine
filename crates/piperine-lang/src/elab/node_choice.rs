//! A node chosen at elaboration (§1.4.7, user decision of 2026-11-24): an `if`, a `match` or a block whose result is a node
//! and whose value does not depend on the run names the node it folds to, wherever a node goes (a probe, a contribution
//! target, a connection). Its conditions read params, constants, generation indexes and the `let`s of its blocks, which fold
//! here; the params it reads are structural (`structural.rs`), so each body variant folds it to one node. A choice the run
//! makes is refused by the checker (`E0133`), and one that does not fold here (a function that reads the run) is refused with
//! the same code.

use std::collections::BTreeMap;

use piperine_ir::Constant;

use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{Arm, BlockNode, DeclId, ExprId, ExprKind, PatKind, Resolution, StmtKind};

/// What folding a choice has bound so far: the constants of the `let`s of its blocks, and the `let`s that name a node.
struct ChoiceScope {
    /// The params, generation indexes and folded `let`s the conditions read.
    bindings: Bindings,
    /// Each `let` of a block bound to a node, to its initializer.
    nodes: BTreeMap<DeclId, ExprId>,
    /// The `let`s whose value binding was tried already, each once: a `let` that reads itself (`s = !s`), directly or through
    /// others, is not followed again, so the fold ends.
    followed: Vec<DeclId>,
}

impl Elaboration<'_, '_> {
    /// The node expression the choice `expr` folds to in `scope`: the arm its condition or scrutinee selects, the result of a
    /// block, through every choice nested in them. `None` when `expr` is no choice; `Some(None)` after a diagnostic.
    pub(super) fn chosen_node(&mut self, scope: &Scope, expr: ExprId) -> Option<Option<ExprId>> {
        if !matches!(self.design.expr_node(expr).kind, ExprKind::If { .. } | ExprKind::Match { .. } | ExprKind::Block(_)) {
            return None;
        }
        let mut within = ChoiceScope { bindings: self.frame_bindings(scope.frame).overlaid(&scope.unrolled), nodes: BTreeMap::new(), followed: Vec::new() };
        let chosen = self.fold_choice(&mut within, expr);
        if chosen.is_none() {
            // a condition that does not fold reads what only the run knows, through a function the checker cannot see into
            let message = "a node chosen by a condition that does not fold at elaboration chooses hardware at run time: a node is hardware, chosen at elaboration or named (§1.4.7)".to_string();
            let span = self.design.expr_node(expr).span;
            self.report(scope.file, crate::msl::Diagnostic::new("E0133", message, span, "a node chosen at run time"));
        }
        Some(chosen)
    }

    /// The node expression `expr` names once every choice in it is folded: itself when it is no choice.
    fn fold_choice(&mut self, within: &mut ChoiceScope, expr: ExprId) -> Option<ExprId> {
        let expr = self.peel_parens(expr);
        match self.design.expr_node(expr).kind.clone() {
            ExprKind::If { cond, then, else_ } => match self.choice_value(within, cond) {
                Ok(Constant::Bool(true)) => self.fold_block(within, &then),
                Ok(Constant::Bool(false)) => self.fold_choice(within, else_?),
                _ => None,
            },
            ExprKind::Match { scrutinee, arms } => {
                let body = self.selected_arm(within, scrutinee, &arms)?;
                self.fold_choice(within, body)
            }
            ExprKind::Block(block) => self.fold_block(within, &block),
            ExprKind::Path(path) => match self.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(local)) if let Some(init) = within.nodes.get(&local).copied() => self.fold_choice(within, init),
                _ => Some(expr),
            },
            _ => Some(expr),
        }
    }

    /// The node the block `block` results in: its `let`s bound first (a node to its initializer, a value to its constant).
    fn fold_block(&mut self, within: &mut ChoiceScope, block: &BlockNode) -> Option<ExprId> {
        let (last, statements) = block.stmts.split_last()?;
        for statement in statements {
            let StmtKind::Let { pat, init: Some(init), .. } = &self.design.stmt_node(*statement).kind else { continue };
            let PatKind::Ident { binding, sub: None, .. } = &self.design.pat_node(*pat).kind else { continue };
            let (binding, init) = (*binding, *init);
            if matches!(self.typed_local(binding), Some(crate::types::Ty::Node(_) | crate::types::Ty::AnyNode)) {
                within.nodes.insert(binding, init);
            } else if let Ok(value) = self.consts.value(init, &within.bindings) {
                within.bindings.bind(binding, value);
            }
        }
        match &self.design.stmt_node(*last).kind {
            StmtKind::Expr { expr, semi: false } => self.fold_choice(within, *expr),
            _ => None,
        }
    }

    /// The constant `expr` folds to, the `let`s it reads bound first to the constants of their initializers (§1.4.7: a `let` of
    /// params and constants is one).
    fn choice_value(&mut self, within: &mut ChoiceScope, expr: ExprId) -> Result<Constant, super::consts::ConstFault> {
        self.bind_read_lets(within, expr);
        self.consts.value_in_a_scope_of_its_own(expr, &within.bindings)
    }

    /// Binds every `let` `expr` reads that is not bound yet to the constant of its initializer, the `let`s that initializer reads
    /// first, or, for one a body assigns, to the value its statements leave it where `expr` reads it (`value_where_read`); one
    /// that does not fold stays unbound, so what reads it does not fold either.
    fn bind_read_lets(&mut self, within: &mut ChoiceScope, expr: ExprId) {
        for local in self.read_bindings(expr) {
            if within.bindings.get(local).is_some() || within.followed.contains(&local) {
                continue;
            }
            within.followed.push(local);
            if self.types.is_assigned(local) {
                if let Some(value) = self.value_where_read(within, local, expr) {
                    within.bindings.bind(local, value);
                }
                continue;
            }
            let Some(init) = self.let_initializer(local) else { continue };
            self.bind_read_lets(within, init);
            if let Ok(value) = self.consts.value(init, &within.bindings) {
                within.bindings.bind(local, value);
            }
        }
    }

    /// The value the `let` `local`, which its body assigns, holds where `at` reads it (§1.4.7; the checker judged its initializer,
    /// every value assigned to it and the conditions around those constant): the statements of its block from its `let` to the
    /// one that holds `at` run at elaboration, the `let`s they read from outside bound first. `None` when they do not fold, or
    /// when the statement that holds `at` assigns `local` before it (a choice nested in a block with an assignment).
    fn value_where_read(&mut self, within: &mut ChoiceScope, local: DeclId, at: ExprId) -> Option<Constant> {
        let (stmts, from) = self.block_declaring(local)?;
        let read = self.design.expr_node(at).span;
        let holds = |span: crate::msl::Span| span.start <= read.start && read.end <= span.end;
        let to = stmts.iter().position(|stmt| holds(self.design.stmt_node(*stmt).span))?;
        if to <= from {
            return None;
        }
        let assigned_before = |this: &Self, stmt: crate::pom::StmtId| {
            crate::pom::BodyWalk::new(this.design).statement_children(stmt).into_iter().any(|child| {
                let mut inside = Vec::new();
                crate::pom::BodyWalk::new(this.design).descendants_depth_first(child, &mut inside);
                inside.into_iter().any(|expr| matches!(&this.design.expr_node(expr).kind, ExprKind::Assign { lhs, .. } if this.read_bindings(*lhs).contains(&local) && this.design.expr_node(expr).span.end <= read.start))
            })
        };
        if assigned_before(self, stmts[to]) {
            return None;
        }
        for stmt in &stmts[from..to] {
            for child in crate::pom::BodyWalk::new(self.design).statement_children(*stmt) {
                self.bind_read_lets(within, child);
            }
        }
        self.consts.local_after(at, &stmts[from..to], local, &within.bindings)
    }

    /// What the assignments of the `let` `local` read, in the block that declares it: each value assigned and each condition around
    /// an assignment (an `if`, a `match`, a loop's condition or range, and every condition inside a loop around it), whose params
    /// a choice by `local` makes structural as it does those of the initializer (`structural.rs`).
    pub(super) fn assignment_reads(&self, local: DeclId) -> Vec<ExprId> {
        let Some((stmts, from)) = self.block_declaring(local) else { return Vec::new() };
        let mut inside = Vec::new();
        for stmt in &stmts[from..] {
            for child in crate::pom::BodyWalk::new(self.design).statement_children(*stmt) {
                crate::pom::BodyWalk::new(self.design).descendants_depth_first(child, &mut inside);
            }
        }
        let within = |outer: crate::msl::Span, inner: crate::msl::Span| outer.start <= inner.start && inner.end <= outer.end;
        let mut reads = Vec::new();
        for assignment in inside.iter().copied() {
            let ExprKind::Assign { lhs, rhs, .. } = &self.design.expr_node(assignment).kind else { continue };
            if !self.read_bindings(*lhs).contains(&local) {
                continue;
            }
            reads.push(*rhs);
            let at = self.design.expr_node(assignment).span;
            for around in inside.iter().copied().filter(|expr| within(self.design.expr_node(*expr).span, at)) {
                match &self.design.expr_node(around).kind {
                    ExprKind::While { .. } | ExprKind::Loop { .. } | ExprKind::For { .. } => {
                        let span = self.design.expr_node(around).span;
                        reads.extend(inside.iter().filter(|expr| within(span, self.design.expr_node(**expr).span)).flat_map(|expr| Self::conditions_of(&self.design.expr_node(*expr).kind)));
                    }
                    kind => reads.extend(Self::conditions_of(kind)),
                }
            }
        }
        reads
    }

    /// The conditions an expression of kind `kind` decides by: an `if`'s, a `match`'s scrutinee and guards, a `while`'s, a `for`'s
    /// range.
    fn conditions_of(kind: &ExprKind) -> Vec<ExprId> {
        match kind {
            ExprKind::If { cond, .. } | ExprKind::While { cond, .. } => vec![*cond],
            ExprKind::For { iter, .. } => vec![*iter],
            ExprKind::Match { scrutinee, arms } => std::iter::once(*scrutinee).chain(arms.iter().filter_map(|arm| arm.guard)).collect(),
            _ => Vec::new(),
        }
    }

    /// The statements of the block that declares the `let` `local` with a name, and the index of its `let` among them: the body
    /// of a function, an `analog` block or a reaction, or a block of an expression of the same owner.
    fn block_declaring(&self, local: DeclId) -> Option<(Vec<crate::pom::StmtId>, usize)> {
        let declares = |stmt: &crate::pom::StmtId| matches!(&self.design.stmt_node(*stmt).kind, StmtKind::Let { pat, .. } if matches!(&self.design.pat_node(*pat).kind, PatKind::Ident { binding, sub: None, .. } if *binding == local));
        let statement = (0..self.design.stmt_count() as u32).map(crate::pom::StmtId).find(|stmt| declares(stmt))?;
        let owner = self.design.stmt_node(statement).owner;
        let mut blocks: Vec<Vec<crate::pom::StmtId>> = match &self.design.decl_data(owner).kind {
            crate::pom::DeclKind::Fn(data) => data.body.iter().map(|body| body.stmts.clone()).collect(),
            crate::pom::DeclKind::Behavior(data) => vec![data.body.clone()],
            crate::pom::DeclKind::Reaction(data) => vec![data.body.stmts.clone()],
            _ => Vec::new(),
        };
        for index in 0..self.design.expr_count() as u32 {
            let node = self.design.expr_node(ExprId(index));
            if node.owner != owner {
                continue;
            }
            match &node.kind {
                ExprKind::Block(block) | ExprKind::If { then: block, .. } | ExprKind::While { body: block, .. } | ExprKind::Loop { body: block } | ExprKind::For { body: block, .. } => blocks.push(block.stmts.clone()),
                _ => {}
            }
        }
        blocks.into_iter().find_map(|stmts| stmts.iter().position(declares).map(|from| (stmts, from)))
    }

    /// The bindings `expr` names, anywhere in it.
    pub(super) fn read_bindings(&self, expr: ExprId) -> Vec<DeclId> {
        let node = self.design.expr_node(expr);
        match &node.kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, crate::pom::DeclKind::Binding(_)) => vec![decl],
                _ => Vec::new(),
            },
            kind => crate::pom::BodyWalk::new(self.design).expression_children(kind).into_iter().flat_map(|child| self.read_bindings(child)).collect(),
        }
    }

    /// The initializer of the `let` that binds `local` with a name, in a statement or a module's body.
    pub(super) fn let_initializer(&self, local: DeclId) -> Option<ExprId> {
        if let Some(value) = self.body_let_value(local) {
            return Some(value);
        }
        (0..self.design.stmt_count()).find_map(|index| match &self.design.stmt_node(crate::pom::StmtId(index as u32)).kind {
            StmtKind::Let { pat, init: Some(init), .. } if matches!(&self.design.pat_node(*pat).kind, PatKind::Ident { binding, sub: None, .. } if *binding == local) => Some(*init),
            _ => None,
        })
    }

    /// The body of the arm of a `match` over `scrutinee` that its constant selects: the first whose pattern holds and whose guard
    /// is true. `None` when the scrutinee or a guard does not fold, or no arm matches.
    fn selected_arm(&mut self, within: &mut ChoiceScope, scrutinee: ExprId, arms: &[Arm]) -> Option<ExprId> {
        let subject = self.choice_value(within, scrutinee).ok()?;
        for arm in arms {
            if !self.constant_pattern_holds(&within.bindings, arm.pat, subject)? {
                continue;
            }
            let guarded = match arm.guard {
                Some(guard) => match self.choice_value(within, guard) {
                    Ok(Constant::Bool(holds)) => holds,
                    _ => return None,
                },
                None => true,
            };
            if guarded {
                return Some(arm.body);
            }
        }
        None
    }
}
