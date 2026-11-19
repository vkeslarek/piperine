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

use piperine_ir::{BranchKind, Condition, ExprId as LoweredExpr, QuantityRole, Terminal};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclKind, FileId};

/// One contribution as lowered: its branch, its quantity, the conditions of the arms that enclose
/// it (outermost first, empty when it is unconditional) and where it was written.
#[derive(Clone)]
pub(super) struct ContributionRecord {
    pub(super) branch: u32,
    pub(super) quantity: QuantityRole,
    pub(super) conditions: Vec<Condition>,
    pub(super) file: FileId,
    pub(super) span: Span,
}

impl ContributionRecord {
    /// Whether no condition encloses the contribution: it is enabled in every solve.
    fn unselected(&self) -> bool {
        self.conditions.is_empty()
    }
}

impl Elaboration<'_, '_> {
    /// Enters the arm of an `if` (or one arm of a `match`) whose condition is `condition`,
    /// taken when it `holds` or when it does not; `true` when a condition was pushed (a test
    /// that matches everything, or that failed to lower, has none). Pair every call with
    /// `leave_arm`.
    pub(super) fn enter_arm(scope: &mut Scope, condition: Option<LoweredExpr>, holds: bool) -> bool {
        match condition {
            Some(expr) => {
                scope.conditions.push(Condition { expr, holds });
                true
            }
            None => false,
        }
    }

    /// Leaves the arm `enter_arm` entered, when it pushed a condition.
    pub(super) fn leave_arm(scope: &mut Scope, entered: bool) {
        if entered {
            scope.conditions.pop();
        }
    }

    /// Fixes the kind of every branch of the body from the contributions it received (the table
    /// of this file): `E0415` for a potential and a flow contribution that nothing separates. A
    /// branch no contribution reached (a flow read numbered it) keeps the kind it was given.
    pub(super) fn decide_branch_kinds(&mut self, scope: &mut Scope) {
        for branch in 0..scope.branches.len() {
            let own: Vec<ContributionRecord> = scope.contributions.iter().filter(|r| r.branch == branch as u32).cloned().collect();
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
            scope.branches[branch].kind = kind;
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
        let ends = &scope.branches[branch];
        format!("({}, {})", self.terminal_name(scope, ends.plus), self.terminal_name(scope, ends.minus))
    }

    /// The name of the node `terminal` of the body being lowered.
    fn terminal_name(&self, scope: &Scope, terminal: Terminal) -> String {
        let Terminal::Node(index) = terminal else { return "gnd".to_string() };
        let decl = scope.nodes.iter().find(|(_, i)| **i == index).map(|(d, _)| *d);
        match decl.map(|d| self.design.decl_data(d)) {
            Some(data) if matches!(data.kind, DeclKind::Port(_) | DeclKind::Signal(_)) => data.name.as_ref().map_or_else(|| format!("#{index}"), |n| n.text.clone()),
            _ => format!("#{index}"),
        }
    }
}
