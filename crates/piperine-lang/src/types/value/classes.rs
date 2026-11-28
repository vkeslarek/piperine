//! Function classes at a call (generics GN-12, part I §3, design D13): a function is solve-reading when its body
//! probes, reads `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(..)` (`temperature()` is constant during an analysis, so it is pure), or calls a
//! solve-reading function (a fixed point over recursion, `closures.rs`). Such a call is accepted in `analog`, an event
//! expression, a reaction body, a body `let` and a value input, and is `E0122` in a const context (a `const`, a param
//! default, a `where`, a const argument). A definition that reads the solve is `E0336`, the rule that names it (§8.2.5).

use crate::pom::{DeclId, ExprId};

use crate::types::check::Context;
use super::ExpressionTyping;

impl ExpressionTyping<'_, '_> {
    /// `E0122` for a call of `function` written where its class is not allowed: a solve-reading function in a const
    /// context, naming the function and the position.
    pub(super) fn check_call_class(&mut self, expr: ExprId, function: DeclId) {
        // a definition that reads the solve is `E0336` (§8.2.5), the rule that names it: it is not asked twice
        if self.state.values.context != Context::Const || self.state.carriers.in_definition > 0 {
            return;
        }
        let position = "a const context";
        if !self.function_class(function, &mut std::collections::BTreeSet::new()).solve_reading {
            return;
        }
        let name = self.state.design.decl_data(function).name.as_ref().map_or_else(String::new, |name| name.text.clone());
        let (file, span) = (self.file_of_expr(expr), self.state.design.expr_node(expr).span);
        let message = format!("the function `{name}` is solve-reading (it probes or reads the environment of the solve) and is called in {position}, where that class is not allowed (§3)");
        self.coded(file, "E0122", span, message, "a call out of its class");
    }
}
