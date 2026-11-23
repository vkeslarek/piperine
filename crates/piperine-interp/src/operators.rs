//! The forms of `analog-operators` a body holds (R1, AO-11): the interpreter is frozen at the M1
//! analog subset, so [`Interpreter::new`](crate::Interpreter::new) names each of them and
//! refuses the design instead of running it. The compiled backend runs every one.

use piperine_ir::Body;

/// The analog operators one body holds.
pub(crate) struct OperatorForms;

impl OperatorForms {
    /// The refusal name of each operator `body` holds, each once, in a fixed order: a `ddx`
    /// (kept as a partial derivative), a `delay` site, a `noise` source, a `simparam` read and
    /// an `emit!` in `analog` (a trace site).
    pub(crate) fn of(body: Body<'_>) -> Vec<&'static str> {
        [
            (!body.partial_derivatives().is_empty(), "ddx"),
            (!body.delays().is_empty(), "delay"),
            (!body.noise_sources().is_empty(), "noise"),
            (!body.session_parameters().is_empty(), "simparam"),
            (!body.traces().is_empty(), "emit! in analog"),
        ]
        .into_iter()
        .filter_map(|(held, form)| held.then_some(form))
        .collect()
    }
}
