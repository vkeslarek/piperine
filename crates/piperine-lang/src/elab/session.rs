//! `simparam(name, default)` (Part IV §14.4, analog-operators AO-07, design mechanism 5): the
//! session parameter `name`, else `default`, an `f64` constant during an analysis. It lowers to
//! `Expr::SessionParameter` wherever a run-time value is read, definitions and selectors
//! included (§14.4.1); a body holds one `SessionParameter` per name, however often it is read.
//! `name` is a literal found after inlining (anything else is `P0001` naming it); a read with no
//! `default` makes the parameter `required`, and the host refuses a run that gives none (`E0456`).

use piperine_ir::{Expr, ExprId as LoweredExpr, Origin, SessionParameter, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::ExprId;

impl Elaboration<'_, '_> {
    /// A `simparam` call read as a value; `None` after a diagnostic.
    pub(super) fn lower_simparam(&mut self, scope: &mut Scope, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let [name, rest @ ..] = args else { return None };
        let span = self.design.expr_node(*name).span;
        let Some(text) = self.text_of(scope, *name) else {
            self.unsupported(scope.file, span, "a `simparam` name that is not a text literal");
            return None;
        };
        let default = self.optional_constant(scope, rest.first().copied(), "a `simparam` default")?;
        let parameter = match self.structure.session_parameters.iter().position(|p| p.name == text) {
            Some(at) => at,
            None => {
                self.structure.session_parameters.push(SessionParameter { name: text, required: false, origin: origin.clone() });
                self.structure.session_parameters.len() - 1
            }
        };
        // a read with no default makes the parameter required: the host must give it
        if default.is_none() {
            self.structure.session_parameters[parameter].required = true;
        }
        Some(self.emit(Expr::SessionParameter { parameter: parameter as u32, default }, ValueType::F64, origin))
    }
}
