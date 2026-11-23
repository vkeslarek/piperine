//! `ddt` wherever a core module goes (Part IV §14.5, abi.md A1; analog-structure mechanism 3).
//!
//! A `ddt` is a **whole addend** when it is written as a leaf of the top-level `+`/`-` chain of an
//! unconditional contribution with one argument, directly or through a body `let` that binds the
//! bare call and is read only so (§8.2.25: the `let` is its expression at each read, so the rows
//! are the inline form's). `contribute.rs` lowers those as `Ddt` addends, as before this feature.
//!
//! Every other `ddt` is a **derivative site**: its rate is the private pin `d<k>.rate` that every
//! read lowers to (`Expr::Derivative(k)`) in the `analog` block, a body `let`, an event argument or
//! a reaction. One `ddt` call of a frame is one site (`Frame::derivatives`), so a body `let` read in
//! `analog` and in a reaction is one instance (§8.3.3). The state (and the `abstol`) of a site is
//! lowered *standalone*, because a core module's inputs are evaluated at every accepted solution
//! whatever control flow surrounds its placement (§14 rule 9): in a scope with no locals, a
//! block-local `let` it reads inlined from its source expression (`Scope::let_sources`), a `match`
//! scrutinee read again at each test.

use std::collections::BTreeSet;

use piperine_ir::{Derivative, Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, PatKind, Resolution};

/// What makes a `ddt` leaf of a contribution a whole addend: the contribution is unconditional,
/// and the body `let`s that bind a bare `ddt` call and are read only as whole addends.
pub(super) struct LeafRules<'a> {
    pub(super) whole_addends: bool,
    pub(super) lets: &'a BTreeSet<DeclId>,
}

impl Elaboration<'_, '_> {
    /// The `ddt` call that `expr`, a leaf of a contribution, is as a whole addend: itself when it is
    /// a call with one argument, or the call of the body `let` it names when that `let` is read only
    /// as a whole addend. `None` when the leaf is no whole addend (it is lowered as a value).
    pub(super) fn whole_addend(&self, rules: &LeafRules, expr: ExprId) -> Option<ExprId> {
        if !rules.whole_addends {
            return None;
        }
        if self.is_ddt(expr) {
            return self.has_one_argument(expr).then_some(expr);
        }
        let binding = self.read_binding(expr)?;
        rules.lets.contains(&binding).then(|| self.body_let_value(binding).map(|v| self.peel_parens(v))).flatten()
    }

    /// Whether the call `expr` takes exactly one argument (no `abstol`).
    fn has_one_argument(&self, expr: ExprId) -> bool {
        matches!(&self.design.expr_node(expr).kind, ExprKind::Call { args, .. } if args.len() == 1)
    }

    /// The binding the path `expr` reads, if it is one.
    fn read_binding(&self, expr: ExprId) -> Option<DeclId> {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res)? {
            Resolution::Decl(decl) if matches!(self.design.decl_data(decl).kind, DeclKind::Binding(_)) => Some(decl),
            _ => None,
        }
    }

    /// The body `let`s of `module` that bind a bare `ddt(x)` call and whose every read, in `module`,
    /// is a whole addend of an unconditional contribution (keys folded as `scope.keys`).
    pub(super) fn addend_only_ddt_lets(&mut self, scope: &Scope, module: DeclId) -> BTreeSet<DeclId> {
        let candidates: Vec<DeclId> = self.design.decl_data(module).children.iter().copied().filter_map(|child| self.ddt_let_binding(child)).collect();
        if candidates.is_empty() {
            return BTreeSet::new();
        }
        let all: BTreeSet<DeclId> = candidates.iter().copied().collect();
        let rules = LeafRules { whole_addends: true, lets: &all };
        let mut leaves: BTreeSet<ExprId> = BTreeSet::new();
        for contribution in self.written_contributions(module, &scope.keys).into_iter().filter(|c| c.conditions.is_empty()) {
            let mut found = Vec::new();
            self.leaf_exprs(&rules, contribution.rhs, &mut found);
            leaves.extend(found);
        }
        candidates.into_iter().filter(|binding| self.module_exprs(module).into_iter().filter(|e| self.read_binding(*e) == Some(*binding)).all(|e| leaves.contains(&e))).collect()
    }

    /// The binding a `let` child of a module declares, when its value is a bare call of `ddt`.
    fn ddt_let_binding(&self, child: DeclId) -> Option<DeclId> {
        let DeclKind::Let(l) = &self.design.decl_data(child).kind else { return None };
        let PatKind::Ident { binding, sub: None, .. } = &self.design.pat_node(l.pat).kind else { return None };
        let value = self.peel_parens(l.value);
        (self.is_ddt(value) && self.has_one_argument(value)).then_some(*binding)
    }

    /// The leaves of the top-level `+`/`-` chain of `expr` that are whole addends or names.
    fn leaf_exprs(&self, rules: &LeafRules, expr: ExprId, out: &mut Vec<ExprId>) {
        let inner = self.peel_parens(expr);
        match &self.design.expr_node(inner).kind {
            ExprKind::Binary { op: crate::pom::BinOp::Add | crate::pom::BinOp::Sub, lhs, rhs } => {
                self.leaf_exprs(rules, *lhs, out);
                self.leaf_exprs(rules, *rhs, out);
            }
            ExprKind::Unary { op: crate::pom::UnOp::Neg, expr: operand } => self.leaf_exprs(rules, *operand, out),
            _ => out.push(inner),
        }
    }

    /// Every expression owned by `module` or by one of its behaviors.
    pub(super) fn module_exprs(&self, module: DeclId) -> Vec<ExprId> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let behaviors = &data.behaviors;
        let inside = |mut owner: DeclId| loop {
            if owner == module || behaviors.contains(&owner) {
                return true;
            }
            match self.design.decl_data(owner).parent {
                Some(parent) => owner = parent,
                None => return false,
            }
        };
        (0..self.design.expr_count() as u32).map(ExprId).filter(|e| inside(self.design.expr_node(*e).owner)).collect()
    }

    /// A `ddt` call that is no whole addend, as the site it is: `Expr::Derivative(k)`. The site is
    /// made the first time the call is read in the frame and its state and `abstol` are lowered
    /// standalone; `None` after a diagnostic.
    pub(super) fn lower_derivative(&mut self, scope: &mut Scope, call: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        if let Some(site) = self.frames[scope.frame].derivatives.get(&call).copied() {
            return Some(self.emit(Expr::Derivative(site), ValueType::F64, origin));
        }
        let span = self.design.expr_node(call).span;
        let [state, rest @ ..] = args else { return None };
        let mut standalone = Self::standalone_scope(scope);
        standalone.value_input = true;
        let state = self.lower_expr(&mut standalone, *state)?;
        let abstol = match rest {
            [tolerance] => Some(self.lower_expr(&mut standalone, *tolerance)?),
            _ => None,
        };
        scope.adopt_standalone(&mut standalone);
        if !standalone.locals.is_empty() {
            self.unsupported(scope.file, span, "a closure or an `Option` binding inside a `ddt` state");
            return None;
        }
        let site = self.structure.derivatives.len() as u32;
        self.structure.derivatives.push(Derivative { name: format!("d{site}"), state, abstol, origin: origin.clone() });
        self.frames[scope.frame].derivatives.insert(call, site);
        Some(self.emit(Expr::Derivative(site), ValueType::F64, origin))
    }

    /// A scope that reads what `scope` names but holds no local (§14 rule 9): a block-local `let`
    /// is read as its source expression, a `match` scrutinee is read at each test.
    pub(super) fn standalone_scope(scope: &Scope) -> Scope {
        let mut standalone = Self::sub_scope(scope);
        standalone.no_locals = true;
        standalone.analog = scope.analog;
        standalone.at_solution = scope.at_solution;
        standalone.value_input = scope.value_input;
        standalone.differentiated = scope.differentiated;
        standalone.keys = scope.keys.clone();
        standalone.let_sources = scope.let_sources.clone();
        // the names of a `fn` being expanded are read in a value nested in it
        standalone.values = scope.values.clone();
        standalone.option_values = scope.option_values.clone();
        standalone.texts = scope.texts.clone();
        standalone.handles = scope.handles.clone();
        standalone.unrolled = scope.unrolled.clone();
        // the stimulus sites and the branches of the enclosing scope stay numbered as they are: one the code adds is the next
        standalone.stims = scope.stims.clone();
        standalone.branches = scope.branches.clone();
        standalone
    }
}
