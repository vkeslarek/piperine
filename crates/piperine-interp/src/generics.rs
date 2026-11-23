//! The forms of `generics` a body evaluates (R1): the interpreter is frozen at the M1 analog
//! subset, so [`Interpreter::new`](crate::Interpreter::new) names each generic instantiation and
//! refuses the design instead of running it.

use piperine_ir::Body;

use crate::composite::CompositeForms;

/// The generic instantiations one body evaluates.
pub(crate) struct GenericForms;

impl GenericForms {
    /// The refusal text of each generic instance a function `body` calls or names evaluates, in
    /// the order the walk met them: ``a generic instantiation `swap::<f64>` ``.
    pub(crate) fn of(body: Body<'_>) -> Vec<String> {
        let ir = body.ir();
        CompositeForms::reached_functions(body)
            .into_iter()
            .filter_map(|function| ir.function(function).instance().map(|instance| format!("a generic instantiation `{}`", instance.name())))
            .collect()
    }
}
