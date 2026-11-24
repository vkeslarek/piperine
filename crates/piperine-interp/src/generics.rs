//! The forms of `generics` a body evaluates (R1): the interpreter is frozen at the M1 analog
//! subset, so [`Interpreter::new`](crate::Interpreter::new) names each generic instantiation and
//! refuses the design instead of running it.

use piperine_ir::{Body, Broken, Ir};

use crate::composite::CompositeForms;

/// The generic instantiations one body evaluates.
pub(crate) struct GenericForms;

impl GenericForms {
    /// The refusal text of each generic instance a function `body` calls or names evaluates, in
    /// the order the walk met them: ``a generic instantiation `swap::<f64>` ``.
    pub(crate) fn of(body: Body<'_>) -> Vec<String> {
        let ir = body.ir();
        let module = body.instance().map(|instance| format!("a generic module instance `{}`", instance.name()));
        let functions = CompositeForms::reached_functions(body)
            .into_iter()
            .filter_map(|function| ir.function(function).instance().map(|instance| format!("a generic instantiation `{}`", instance.name())));
        module.into_iter().chain(functions).collect()
    }
}

impl GenericForms {
    /// One refusal for each type of `ir` whose `Repr` is written by hand (GN-05): the interpreter
    /// lays a value out by `#[derive(Repr)]` only.
    pub(crate) fn hand_written_types(ir: &Ir) -> Vec<Broken> {
        ir.types()
            .iter()
            .filter(|data| !data.repr.is_derived())
            .map(|data| Broken {
                item: format!("type {}", data.name),
                origin: data.origin.clone(),
                what: format!("type {} holds a hand-written Repr: the interpreter runs the M1 analog subset only", data.name),
            })
            .collect()
    }
}
