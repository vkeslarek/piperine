//! Faults of the constant evaluator, turned into the diagnostic of the position they sit in.

use super::consts::ConstFault;
use super::elaborator::Elaboration;
use crate::msl::Diagnostic;

impl Elaboration<'_, '_> {
    /// The name of the `const` item `fault` sits in and what is wrong with it, when the fault is
    /// one §3.9.2 makes `E0188`: a `const` initializer that reads a run value, a `const` that
    /// depends on itself, or a `const` without a value outside a trait.
    fn const_item_fault(&self, fault: &ConstFault) -> Option<(String, String)> {
        let named = |decl: crate::pom::DeclId| self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |name| name.text.clone());
        match fault {
            ConstFault::NoValue { decl } if !self.consts.is_trait_item(*decl) => Some((named(*decl), "it has no value".to_string())),
            ConstFault::Cyclic { constant, .. } => Some((named(*constant), "it depends on itself".to_string())),
            ConstFault::NotConstant { constant, what, .. } => Some((named(*constant), format!("it reads {what}"))),
            _ => None,
        }
    }

    /// Report `fault`, found while folding `position` (`"a param default"`, `"a node's initial
    /// value"`): a read of a param is `P0001` (a value that depends on another param is M2),
    /// anything that needs a run is `E0321`, a form the evaluator does not fold is `P0001`, a panic
    /// (an integer division by zero) is `E0104` (§1 rule 11), and a value that is unknown is
    /// `E0151` (§3.3.8): every position that reaches here is one `Unknown` cannot take, a `where`
    /// condition, a param value or default, a nature's `abstol`. A position that takes an unknown
    /// (a carrier's initial value, a `const` read) folds it before it asks.
    ///
    /// A fault found inside the initializer of a `const` item is `E0188` naming the `const`
    /// (§3.9.2): a read of something only a run gives, a `const` without a value outside a trait,
    /// a `const` that depends on itself. A param default keeps `E0321`.
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
        if let Some((name, reason)) = self.const_item_fault(fault) {
            let message = format!("the `const` `{name}` is not a const expression: {reason}");
            self.report(file, Diagnostic::new("E0188", message, span, "not a const expression"));
            return;
        }
        match fault {
            // Reported above as E0188.
            ConstFault::Cyclic { .. } | ConstFault::NotConstant { .. } => {}
            // §20: the file of an `include_str!` has no text.
            ConstFault::File { fault, .. } => self.report(file, Diagnostic::new(fault.code(), fault.message(), span, "the file named here")),
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
            ConstFault::Panic { what, .. } => {
                let message = format!("{position} panics while it is evaluated at elaboration: {what}");
                self.report(file, Diagnostic::new("E0104", message, span, "panics"));
            }
        }
    }
}
