//! Faults of the constant evaluator, turned into the diagnostic of the position they sit in.

use super::consts::ConstFault;
use super::elaborator::Elaboration;
use crate::msl::Diagnostic;

impl Elaboration<'_, '_> {
    /// Report `fault`, found while folding `position` (`"a param default"`, `"a node's initial
    /// value"`): a read of a param is `P0001` (a value that depends on another param is M2),
    /// anything that needs a run is `E0321`, a form the evaluator does not fold is `P0001`, a panic
    /// (an integer division by zero) is `E0104` (§1 rule 11), and a value that is unknown is
    /// `E0151` (§3.3.8): every position that reaches here is one `Unknown` cannot take, a `where`
    /// condition, a param value or default, a nature's `abstol`. A position that takes an unknown
    /// (a carrier's initial value, a `const` read) folds it before it asks.
    pub(super) fn const_fault(&mut self, fault: &ConstFault, position: &str) {
        let (file, span) = match fault {
            ConstFault::NoValue { decl } => {
                let data = self.design.decl_data(*decl);
                (data.file, data.span)
            }
            other => {
                let expr = other.expr().map(|e| self.design.expr_node(e));
                let span = expr.map_or_else(Default::default, |e| e.span);
                let file = expr.map_or(crate::pom::FileId(0), |e| self.design.decl_data(e.owner).file);
                (file, span)
            }
        };
        match fault {
            ConstFault::UnboundParam { .. } => self.unsupported(file, span, &format!("{position} that reads a param")),
            ConstFault::Dynamic { what, .. } => {
                let message = format!("{position} is not an expression over params and constants: it reads {what}");
                self.report(file, Diagnostic::new("E0321", message, span, "not constant"));
            }
            ConstFault::Unsupported { what, .. } => self.unsupported(file, span, &format!("{what} in {position}")),
            ConstFault::NoValue { .. } => self.unsupported(file, span, &format!("a `const` without a value in {position}")),
            ConstFault::Unknown { .. } => {
                let message = format!("{position} is unknown: a value that may be unknown cannot decide structure (§3.3.8)");
                self.report(file, Diagnostic::new("E0151", message, span, "unknown here"));
            }
            ConstFault::Leaves { .. } => self.unsupported(file, span, &format!("a `break`, `continue` or `return` outside the loop or function it leaves in {position}")),
            ConstFault::Panic { what, .. } => {
                let message = format!("{position} panics while it is evaluated at elaboration: {what}");
                self.report(file, Diagnostic::new("E0104", message, span, "panics"));
            }
        }
    }
}
