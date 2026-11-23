//! Analog loops and recursions that cannot stay what they are are expanded at elaboration
//! (part IV §11 rule 12, part III §8.3.4, part I §3 rule 12).
//!
//! * A loop in an `analog` block that holds a contribution or a placement is its unrolling: the
//!   trip count is folded by `ConstEval` over the params and constants of the frame (and the loop
//!   variables of the loops around it), and each iteration is the body lowered again with the loop
//!   variable bound to its constant (`Scope::unrolled`). A placement in iteration `k` is a distinct
//!   instance, hoisted by the existing rule and numbered in unrolled order (D16). A trip count that
//!   does not fold is `E0416` for a contribution and `E0351` for a placement. A loop that holds
//!   neither stays a loop in the IR.
//! * A call from an `analog` block of a function that reaches a call cycle is expanded where it
//!   stands, its parameters bound to the constants its arguments fold to, so each decision the
//!   recursion hangs on folds and only the arm it selects is lowered. A decision that does not fold,
//!   or a depth past 1 024, is `E0126` naming the function: the IR never holds a recursion reached
//!   from `analog`.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{Constant, Expr, ExprId as LoweredExpr, Local, Origin, Stmt, StmtId as LoweredStmt};

use super::consts::{Bindings, ConstFault};
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::{Diagnostic, Span};
use crate::pom::{Arm, BlockNode, DeclId, DeclKind, ExprId, ExprKind, PatKind, StmtKind};
use crate::types::{Callee, Ty};

/// The most iterations one loop is unrolled into, and the deepest call an expansion follows
/// (Q5, §1 rule 11).
const TRIPS: i128 = 1 << 24;
const DEPTH: usize = 1024;

/// What an analog loop holds that cannot stay a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Held {
    /// A contribution to a branch (§11 rule 12).
    Contribution,
    /// A placement of a module with a result (§8.3.4).
    Placement,
}

impl Held {
    /// The code of a trip count that does not fold.
    fn code(self) -> &'static str {
        match self {
            Held::Contribution => "E0416",
            Held::Placement => "E0351",
        }
    }

    /// What the loop holds, as the diagnostic names it.
    fn noun(self) -> &'static str {
        match self {
            Held::Contribution => "a contribution",
            Held::Placement => "a placement",
        }
    }
}

/// One recursive call being expanded: the function, and how many decisions that did not fold the
/// code sat under when its expansion began.
pub(super) struct Expansion {
    /// The function whose body is being lowered in place.
    function: DeclId,
    /// `Scope::decisions` at the call.
    decisions: u32,
}

/// The functions on a call cycle, and the functions that reach one: asked often, answered once.
#[derive(Default)]
pub(super) struct CallCycles {
    /// Whether a function can call itself, directly or through others.
    cyclic: BTreeMap<DeclId, bool>,
    /// Whether a function is, or calls, one on a cycle.
    reaching: BTreeMap<DeclId, bool>,
}

impl Elaboration<'_, '_> {
    // ---- loops -------------------------------------------------------------------------------

    /// The loop statement `expr` of an `analog` block as its unrolling, its statements appended to
    /// `out`; `None` when it is no loop of an `analog` block or holds nothing that unrolls it,
    /// `Some(false)` after a diagnostic.
    pub(super) fn lower_analog_loop(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, expr: ExprId) -> Option<bool> {
        if !scope.in_analog_block {
            return None;
        }
        let node = self.design.expr_node(self.peel_parens(expr)).clone();
        let body = match &node.kind {
            ExprKind::For { body, .. } | ExprKind::While { body, .. } | ExprKind::Loop { body } => body.clone(),
            _ => return None,
        };
        let held = self.held_by_loop(&body)?;
        let ExprKind::For { pat, iter, .. } = &node.kind else {
            self.not_constant(scope, node.span, held, "a `while` or a `loop` has no trip count");
            return Some(false);
        };
        let iter = self.peel_parens(*iter);
        let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.design.expr_node(iter).kind else {
            self.not_constant(scope, node.span, held, "a `for` that is not over a range with both bounds has no constant trip count");
            return Some(false);
        };
        let variable = match self.design.pat_node(*pat).kind {
            PatKind::Ident { binding, sub: None, .. } => Some(binding),
            PatKind::Wild => None,
            _ => {
                self.unsupported(scope.file, node.span, "an analog loop whose variable is not a name");
                return Some(false);
            }
        };
        let Some((first, end)) = self.trip_range(scope, (lo, hi), inclusive, held, node.span) else { return Some(false) };
        Some(self.unroll(scope, out, variable, (first, end), &body))
    }

    /// What `body` holds that unrolls the loop around it: a contribution first, else a placement.
    fn held_by_loop(&self, body: &BlockNode) -> Option<Held> {
        if crate::pom::nested_stmts(self.design, &body.stmts).iter().any(|s| matches!(self.design.stmt_node(*s).kind, StmtKind::Contribution { .. })) {
            return Some(Held::Contribution);
        }
        (!self.placements_in(body).is_empty()).then_some(Held::Placement)
    }

    /// Every expression of `body`, nested ones included.
    fn body_exprs(&self, body: &BlockNode) -> Vec<ExprId> {
        let mut inside = Vec::new();
        for expr in crate::pom::block_exprs(self.design, body) {
            crate::pom::preorder(self.design, expr, &mut inside);
        }
        inside
    }

    /// The placements of hoisted modules inside `body`.
    fn placements_in(&self, body: &BlockNode) -> Vec<ExprId> {
        self.body_exprs(body).into_iter().filter(|e| self.hoisted_callee(*e).is_some()).collect()
    }

    /// The placements of hoisted modules that `module` writes inside a loop: hoisted by the
    /// unrolling of the loop, never up front.
    pub(super) fn looped_placements(&mut self, module: DeclId) -> BTreeSet<ExprId> {
        if let Some(known) = self.looped.get(&module) {
            return known.clone();
        }
        let mut found = BTreeSet::new();
        if let DeclKind::Module(data) = &self.design.decl_data(module).kind {
            for block in data.behaviors.clone() {
                let DeclKind::Behavior(behavior) = &self.design.decl_data(block).kind else { continue };
                for stmt in crate::pom::nested_stmts(self.design, &behavior.body) {
                    for expr in crate::pom::stmt_own_exprs(self.design, stmt) {
                        if let ExprKind::For { body, .. } | ExprKind::While { body, .. } | ExprKind::Loop { body } = &self.design.expr_node(expr).kind {
                            found.extend(self.placements_in(body));
                        }
                    }
                }
            }
        }
        self.looped.insert(module, found.clone());
        found
    }

    /// The first value and the end (exclusive) a loop over `(lo, hi)` counts, folded over the
    /// frame's params and the loop variables around it; `None` after a diagnostic: `E0416` or
    /// `E0351` when a bound does not fold, `E0104` when one panics or the trips pass 2^24.
    fn trip_range(&mut self, scope: &Scope, (lo, hi): (ExprId, ExprId), inclusive: bool, held: Held, span: Span) -> Option<(i128, i128)> {
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        let (low, high) = (self.consts.value(lo, &bindings), self.consts.value(hi, &bindings));
        let (first, last) = match (low, high) {
            (Ok(Constant::Int(first)), Ok(Constant::Int(last))) => (first, last),
            (Err(fault @ ConstFault::Panic { .. }), _) | (_, Err(fault @ ConstFault::Panic { .. })) => {
                self.const_fault(&fault, "the bound of a loop");
                return None;
            }
            _ => {
                self.not_constant(scope, span, held, "its bounds are not expressions over params and constants");
                return None;
            }
        };
        let end = if inclusive { last + 1 } else { last };
        if end - first > TRIPS {
            let message = format!("this loop runs more than 2^24 iterations: {} (§1 rule 11)", end - first);
            self.report(scope.file, Diagnostic::new("E0104", message, span, "too many iterations"));
            return None;
        }
        Some((first, end.max(first)))
    }

    /// `E0416` or `E0351`: the loop at `span` holds `held` and `why` its trip count is not constant.
    fn not_constant(&mut self, scope: &Scope, span: Span, held: Held, why: &str) {
        let message = format!("a loop that holds {} must have a constant trip count: {why} (§11 rule 12, §8.3.4)", held.noun());
        self.report(scope.file, Diagnostic::new(held.code(), message, span, "not a constant trip count"));
    }

    /// The body of a loop lowered once per value of its variable, the statements of every
    /// iteration appended to `out`; `false` after a diagnostic. Each iteration hoists its placements
    /// and derivative sites afresh: the body is a different expression there.
    fn unroll(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, variable: Option<DeclId>, (first, end): (i128, i128), body: &BlockNode) -> bool {
        let inside = self.body_exprs(body);
        let placements: Vec<ExprId> = inside.iter().copied().filter(|e| self.hoisted_callee(*e).is_some()).collect();
        let mut ok = true;
        for value in first..end {
            if let Some(variable) = variable {
                scope.unrolled.bind(variable, Constant::Int(value));
            }
            let frame = &mut self.frames[scope.frame];
            for call in &placements {
                frame.placements.remove(call);
                frame.failed.remove(call);
            }
            for expr in &inside {
                frame.derivatives.remove(expr);
            }
            let Some((stmts, tail)) = self.lower_statements(scope, &body.stmts) else {
                ok = false;
                break;
            };
            out.extend(stmts);
            if let Some(tail) = tail {
                out.push(self.stmt(Stmt::Expr(tail)));
            }
        }
        if let Some(variable) = variable {
            scope.unrolled.unbind(variable);
        }
        ok
    }

    // ---- folds over the bindings of an unrolling ---------------------------------------------

    /// Whether the condition `cond` is decided at elaboration: it reads only the key params and
    /// constants (§11 rule 3), or the loop variables and parameters an unrolling has bound.
    pub(super) fn folded_decision(&mut self, scope: &Scope, cond: ExprId) -> Option<bool> {
        let chain = self.placement_chain(scope.frame);
        if let Some(holds) = self.folded_condition(&scope.keys.clone(), &chain, cond) {
            return Some(holds);
        }
        let bindings = self.unrolled_bindings(scope)?;
        match self.consts.value(cond, &bindings) {
            Ok(Constant::Bool(holds)) => Some(holds),
            _ => None,
        }
    }

    /// The arm a `match` on `scrutinee` selects at elaboration, by the same two rules.
    pub(super) fn folded_selection(&mut self, scope: &Scope, scrutinee: ExprId, arms: &[Arm]) -> Option<usize> {
        let chain = self.placement_chain(scope.frame);
        if let Some(selected) = self.folded_arm(&scope.keys.clone(), &chain, scrutinee, arms) {
            return Some(selected);
        }
        let bindings = self.unrolled_bindings(scope)?;
        let subject = self.consts.value(scrutinee, &bindings).ok()?;
        for (position, arm) in arms.iter().enumerate() {
            if arm.guard.is_some() {
                return None;
            }
            if self.constant_pattern_holds(&bindings, arm.pat, subject)? {
                return Some(position);
            }
        }
        None
    }

    /// What a fold reads in `scope` when an unrolling has bound something: the frame's params and
    /// the bindings of the unrolling; `None` when nothing is bound.
    fn unrolled_bindings(&self, scope: &Scope) -> Option<Bindings> {
        (!scope.unrolled.is_empty()).then(|| self.frame_bindings(scope.frame).overlaid(&scope.unrolled))
    }

    // ---- recursion ---------------------------------------------------------------------------

    /// The functions `function`'s body calls by name.
    fn callees(&self, function: DeclId) -> Vec<DeclId> {
        let DeclKind::Fn(f) = &self.design.decl_data(function).kind else { return Vec::new() };
        let Some(body) = &f.body else { return Vec::new() };
        let mut found = Vec::new();
        for expr in self.body_exprs(body) {
            if matches!(self.design.expr_node(expr).kind, ExprKind::Call { .. })
                && let Some(Callee::Function(callee)) = self.types.callee(expr)
                && !found.contains(&callee)
            {
                found.push(callee);
            }
        }
        found
    }

    /// Whether `function` can call itself, directly or through others.
    fn on_cycle(&mut self, function: DeclId) -> bool {
        if let Some(known) = self.cycles.cyclic.get(&function) {
            return *known;
        }
        let mut seen = BTreeSet::new();
        let mut open = self.callees(function);
        let mut cyclic = false;
        while let Some(next) = open.pop() {
            if next == function {
                cyclic = true;
                break;
            }
            if seen.insert(next) {
                open.extend(self.callees(next));
            }
        }
        self.cycles.cyclic.insert(function, cyclic);
        cyclic
    }

    /// Whether `function` is on a call cycle or calls a function that is: the functions an `analog`
    /// block must expand rather than call (§3 rule 12).
    pub(super) fn reaches_recursion(&mut self, function: DeclId) -> bool {
        if let Some(known) = self.cycles.reaching.get(&function) {
            return *known;
        }
        let mut seen = BTreeSet::from([function]);
        let mut open = vec![function];
        let mut reaches = false;
        while let Some(next) = open.pop() {
            if self.on_cycle(next) {
                reaches = true;
                break;
            }
            for callee in self.callees(next) {
                if seen.insert(callee) {
                    open.push(callee);
                }
            }
        }
        self.cycles.reaching.insert(function, reaches);
        reaches
    }

    /// `E0126`: the recursion of `function` cannot be expanded; `why` says what stops it.
    pub(super) fn unfoldable_recursion(&mut self, scope: &Scope, function: DeclId, span: Span, why: &str) {
        let name = self.design.decl_data(function).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("the recursion of `{name}` reached from `analog` cannot be unrolled: {why} (§3 rule 12)");
        self.report(scope.file, Diagnostic::new("E0126", message, span, "recursion does not fold"));
    }

    /// A call of `function`, which is or reaches a recursion, from an `analog` block: its body lowered
    /// in place as a block, each parameter a local holding its argument and, when the argument
    /// folds, bound to that constant too, so the decisions of the body fold. `None` after a
    /// diagnostic.
    pub(super) fn lower_expanded_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(body) = f.body.clone() else {
            self.unsupported(scope.file, span, "a recursion through a function without a body");
            return None;
        };
        if self.calls_option_function(expr).is_some() || self.expands_forms_it_cannot(&body) {
            self.unsupported(scope.file, span, "a recursion reached from `analog` whose function returns early or returns an `Option`");
            return None;
        }
        if scope.expanding.len() >= DEPTH {
            self.unfoldable_recursion(scope, function, span, "it is deeper than 1 024 calls");
            return None;
        }
        if scope.expanding.iter().any(|open| open.function == function && scope.decisions > open.decisions) {
            self.unfoldable_recursion(scope, function, span, "a decision it hangs on does not fold");
            return None;
        }
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        if params.iter().any(|p| matches!(self.typed_local(*p), Some(Ty::Option(_)))) {
            self.unsupported(scope.file, span, "a recursion reached from `analog` with an `Option` parameter");
            return None;
        }
        // The arguments are read where the call stands, before any parameter is rebound.
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        let slots = self.types.binding(expr).map(<[_]>::to_vec).unwrap_or_default();
        let folded: Vec<Option<Constant>> = slots.iter().map(|slot| slot.and_then(|arg| self.consts.value(arg, &bindings).ok())).collect();
        let arguments = self.lower_arguments(scope, expr, function)?;
        let (saved_locals, saved_unrolled) = (scope.local_index.clone(), scope.unrolled.clone());
        let mut stmts = Vec::new();
        for (k, param) in params.iter().enumerate() {
            let parameter = self.design.decl_data(*param).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let init = *arguments.get(k)?;
            let local = scope.locals.len() as u32;
            let ty = self.type_of(init);
            scope.locals.push(Local { name: format!("{name}.{parameter}"), ty, mutable: false });
            scope.local_index.insert(*param, local);
            scope.unrolled.unbind(*param);
            if let Some(Some(value)) = folded.get(k) {
                scope.unrolled.bind(*param, *value);
            }
            stmts.push(self.stmt(Stmt::Let { local, init }));
        }
        scope.expanding.push(Expansion { function, decisions: scope.decisions });
        let lowered = crate::stack::Stack::deep(|| self.lower_statements(scope, &body.stmts));
        scope.expanding.pop();
        scope.local_index = saved_locals;
        scope.unrolled = saved_unrolled;
        let (inner, value) = lowered?;
        stmts.extend(inner);
        let ty = value.map_or(piperine_ir::ValueType::Unit, |v| self.type_of(v));
        Some(self.emit(Expr::Block { stmts, value }, ty, origin))
    }

    /// Whether `body` holds an exit an expansion cannot place: `return` or `?`.
    fn expands_forms_it_cannot(&self, body: &BlockNode) -> bool {
        self.body_exprs(body).iter().any(|e| matches!(self.design.expr_node(*e).kind, ExprKind::Return(_) | ExprKind::Try(_)))
    }
}
