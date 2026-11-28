//! Which contributions a branch receives and under which conditions, and the kind that decides
//! (Part IV §11 rules 2 and 3).
//!
//! A contribution stays where it was written: inside the arms of an `if` (a `match` lowers to a
//! chain of them) and in nested blocks. Its *selector* is the conjunction of the conditions of
//! the arms that enclose it. `Scope::conditions` is the stack of those conditions while the
//! `analog` block is lowered and every `lower_contribution` records its stack in a
//! `ContributionRecord`; once the whole block is lowered `decide_branch_kinds` reads the records
//! and fixes each branch's kind:
//!
//! | Contributions to the branch | Kind |
//! | --- | --- |
//! | flow only | `Flow` |
//! | potential only, one of them under no condition | `Potential` |
//! | a potential and a flow contribution, both under no condition | `E0415` |
//! | any other mix, or potential only and all of it under a condition | `Switch` |
//!
//! `check_selector_rule` then applies §11 rule 3 to the lowered conditions: a branch that has a
//! potential contribution anywhere in the module has a kind that is constant during a solve, so no
//! condition enclosing a contribution to it, potential or flow, may read a probe (`E0414`).

use super::AnalogLowering;
use std::collections::HashSet;

use piperine_ir::{BranchKind, Condition, Expr, ExprId as LoweredExpr, QuantityRole, Terminal};

use crate::elab::elaborator::Scope;
use crate::refusal::forms::ValueForm;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclKind, ExprId, FileId, PatId};

/// One contribution as lowered: its branch, its quantity, the conditions of the arms that enclose
/// it (outermost first, empty when it is unconditional) and where it was written.
#[derive(Clone)]
pub(in crate::elab) struct ContributionRecord {
    pub(in crate::elab) branch: u32,
    pub(in crate::elab) quantity: QuantityRole,
    pub(in crate::elab) conditions: Vec<Condition>,
    pub(in crate::elab) file: FileId,
    pub(in crate::elab) span: Span,
}

/// What the value of a condition depends on, read off its lowered expression.
#[derive(Default)]
pub(super) struct SelectorReads {
    /// The first probe, branch flow, derivative or `limit` value it reads, as the diagnostic
    /// names it: what no selector of a potential-capable branch may read.
    pub(in crate::elab) probe: Option<String>,
}

/// A condition of an enclosing arm as written, so a guarded site can lower it again with no local
/// in reach (§14 rule 9): what its `enabled` or `when` is made of.
#[derive(Clone)]
pub(in crate::elab) enum ArmCondition {
    /// The arm of `if condition` taken when `holds`, or the right operand of `&&` (`holds`) or `||`.
    If { condition: ExprId, holds: bool },
    /// The arm of a `match` on `scrutinee` whose pattern is `pat` (`None`: the last arm), reached
    /// when none of the patterns `earlier` matched.
    MatchArm { scrutinee: ExprId, earlier: Vec<PatId>, pat: Option<PatId> },
    /// An arm of `if let`, whose test is the pair of an `Option` read once.
    IfLet,
    /// An arm of a `match` with a guard: its test reads the names the pattern binds.
    Guarded,
}

impl ContributionRecord {
    /// Whether no condition encloses the contribution: it is enabled in every solve.
    fn unselected(&self) -> bool {
        self.conditions.is_empty()
    }
}

impl AnalogLowering<'_, '_, '_> {
    /// Enters the arm of an `if` (or one arm of a `match`) whose condition is `condition`,
    /// taken when it `holds` or when it does not; `true` when a condition was pushed (a test
    /// that matches everything, or that failed to lower, has none). Pair every call with
    /// `leave_arm`.
    pub(in crate::elab) fn enter_arm(scope: &mut Scope, condition: Option<LoweredExpr>, holds: bool) -> bool {
        match condition {
            Some(expr) => {
                scope.analog.conditions.push(Condition { expr, holds });
                true
            }
            None => false,
        }
    }

    /// Leaves the arm `enter_arm` entered, when it pushed a condition.
    pub(in crate::elab) fn leave_arm(scope: &mut Scope, entered: bool) {
        if entered {
            scope.analog.conditions.pop();
        }
    }

    /// Fixes the kind of every branch of the body from the contributions it received (the table
    /// of this file): `E0415` for a potential and a flow contribution that nothing separates. A
    /// branch no contribution reached (a flow read numbered it) keeps the kind it was given.
    pub(in crate::elab) fn decide_branch_kinds(&mut self, scope: &mut Scope) {
        for branch in 0..scope.analog.branches.len() {
            let own: Vec<ContributionRecord> = scope.analog.contributions.iter().filter(|r| r.branch == branch as u32).cloned().collect();
            let of = |quantity: QuantityRole| own.iter().filter(move |r| r.quantity == quantity);
            let (potential, flow) = (of(QuantityRole::Potential).next().is_some(), of(QuantityRole::Flow).next().is_some());
            let kind = match (potential, flow) {
                (false, false) => continue,
                (false, true) => BranchKind::Flow,
                (true, false) if of(QuantityRole::Potential).any(ContributionRecord::unselected) => BranchKind::Potential,
                (true, true) if of(QuantityRole::Potential).any(ContributionRecord::unselected) && of(QuantityRole::Flow).any(ContributionRecord::unselected) => {
                    self.report_unseparated(scope, branch, &own);
                    continue;
                }
                _ => BranchKind::Switch,
            };
            scope.analog.branches[branch].kind = kind;
        }
    }

    /// `E0415` for the branch `branch`, at the later of its two unconditional contributions of
    /// different quantities.
    fn report_unseparated(&mut self, scope: &Scope, branch: usize, own: &[ContributionRecord]) {
        let unselected = |quantity: QuantityRole| own.iter().find(|r| r.quantity == quantity && r.unselected());
        let (Some(potential), Some(flow)) = (unselected(QuantityRole::Potential), unselected(QuantityRole::Flow)) else { return };
        let later = if potential.span.start > flow.span.start { potential } else { flow };
        let name = self.branch_name(scope, branch);
        let message = format!("the branch {name} receives a potential and a flow contribution that no condition separates");
        self.report(later.file, Diagnostic::new("E0415", message, later.span, "a contribution of the other kind is enabled with this one"));
    }

    /// `(a, b)`: the nodes of a branch as the module names them, `gnd` for ground.
    pub(super) fn branch_name(&self, scope: &Scope, branch: usize) -> String {
        let ends = &scope.analog.branches[branch];
        format!("({}, {})", self.terminal_name(ends.plus), self.terminal_name(ends.minus))
    }

    /// The name of the node `terminal` of the body being lowered.
    pub(super) fn terminal_name(&self, terminal: Terminal) -> String {
        let Terminal::Node(index) = terminal else { return "gnd".to_string() };
        let decl = self.state.structure.frames[0].nodes.iter().find(|(_, t)| **t == Terminal::Node(index)).map(|(d, _)| *d);
        match decl.map(|d| self.state.design.decl_data(d)) {
            Some(data) if matches!(data.kind, DeclKind::Port(_) | DeclKind::Signal(_)) => data.name.as_ref().map_or_else(|| format!("#{index}"), |n| n.text.clone()),
            _ => format!("#{index}"),
        }
    }
}

impl AnalogLowering<'_, '_, '_> {
    /// `E0414` for every contribution, to a branch that has a potential contribution, whose
    /// enclosing conditions read a probe (§11 rule 3). Run after `decide_branch_kinds`; a branch
    /// of kind `Flow` has no potential contribution, so a probe may select between its flows.
    pub(in crate::elab) fn check_selector_rule(&mut self, scope: &Scope) {
        for record in &scope.analog.contributions {
            if scope.analog.branches[record.branch as usize].kind == BranchKind::Flow {
                continue;
            }
            let read = record.conditions.iter().find_map(|c| self.selector_reads(scope, c.expr).probe);
            let Some(probe) = read else { continue };
            let name = self.branch_name(scope, record.branch as usize);
            let message = format!("a condition enclosing a contribution to the branch {name} reads {probe}: the kind of a branch with a potential contribution is constant during a solve (§11 rule 3)");
            self.report(record.file, Diagnostic::new("E0414", message, record.span, "this contribution is selected by a probe"));
        }
    }

    /// What the lowered condition `condition` reads: a probe, a branch flow, a derivative or a
    /// `limit` value through any operator, through the initializer of a local it names, and
    /// through the arguments of a call.
    fn selector_reads(&self, scope: &Scope, condition: LoweredExpr) -> SelectorReads {
        let mut reads = SelectorReads::default();
        let mut seen: HashSet<LoweredExpr> = HashSet::new();
        let mut open = vec![condition];
        while let Some(id) = open.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(kind) = self.state.builder.expr_kind(id) else { continue };
            let found = match kind {
                Expr::Probe { plus, minus } => Some(format!("the probe of ({}, {})", self.terminal_name(*plus), self.terminal_name(*minus))),
                Expr::Flow { branch } => Some(format!("the flow of the branch {}", self.branch_name(scope, *branch as usize))),
                Expr::Derivative(_) => Some("a `ddt` result".to_string()),
                Expr::Delay(_) => Some("a `delay` result".to_string()),
                Expr::Noise(_) => Some("a `noise` value".to_string()),
                Expr::Limit { .. } => Some("a `limit` value".to_string()),
                _ => None,
            };
            if found.is_some() {
                reads.probe = found;
                return reads;
            }
            open.extend(Self::operands_of(kind));
            if let Expr::Local(local) = kind {
                open.extend(scope.values.local_inits.get(local).copied());
            }
        }
        reads
    }

    /// The operand expressions of the condition forms `selector_reads` follows.
    fn operands_of(kind: &Expr) -> Vec<LoweredExpr> {
        match kind {
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => vec![*expr],
            Expr::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
            Expr::Unwrap { is, value, .. } => vec![*is, *value],
            Expr::If { cond, then, otherwise } => vec![*cond, *then, *otherwise],
            Expr::Call { args, .. } | Expr::Math { args, .. } => args.clone(),
            Expr::Method { receiver, args, .. } => [*receiver].into_iter().chain(args.iter().copied()).collect(),
            _ => Vec::new(),
        }
    }
}

impl AnalogLowering<'_, '_, '_> {
    /// The conjunction of the conditions of the arms `scope` sits in, lowered standalone (no local
    /// in reach, §14 rule 9); `Some(None)` when nothing encloses the code, `None` after a diagnostic
    /// (`P0001` for an `if let` arm).
    pub(super) fn standalone_guard(&mut self, scope: &Scope, span: Span, what: &str) -> Option<Option<LoweredExpr>> {
        if scope.analog.guards.is_empty() {
            return Some(None);
        }
        let mut standalone = Self::standalone_scope(scope);
        let mut tests: Vec<LoweredExpr> = Vec::new();
        for guard in &scope.analog.guards.clone() {
            match guard {
                ArmCondition::If { condition, holds } => {
                    let test = self.values().lower_expr(&mut standalone, *condition)?;
                    let origin = self.expr_origin(*condition);
                    tests.push(if *holds { test } else { self.emit(Expr::Unary { op: piperine_ir::UnaryOp::Not, expr: test }, piperine_ir::ValueType::Bool, origin) });
                }
                ArmCondition::MatchArm { scrutinee, earlier, pat } => tests.extend(self.values().standalone_arm_tests(&mut standalone, *scrutinee, earlier, *pat)?),
                ArmCondition::IfLet => {
                    self.refuse_with(scope.values.file, span, ValueForm::UnderIfLet, [what.to_string()]);
                    return None;
                }
                ArmCondition::Guarded => {
                    self.refuse_with(scope.values.file, span, ValueForm::UnderMatchArmWithGuard, [what.to_string()]);
                    return None;
                }
            }
        }
        if !standalone.values.locals.is_empty() {
            self.refuse_with(scope.values.file, span, ValueForm::UnderConditionThatBindsLocal, [what.to_string()]);
            return None;
        }
        let mut tests = tests.into_iter();
        let Some(first) = tests.next() else { return Some(None) };
        let origin = self.state.values.expr_origins[first.index()].clone();
        Some(Some(tests.fold(first, |lhs, rhs| self.emit(Expr::Binary { op: piperine_ir::BinaryOp::And, lhs, rhs }, piperine_ir::ValueType::Bool, origin.clone()))))
    }
}
