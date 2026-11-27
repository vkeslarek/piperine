//! The interpreter of `piperine-interp` over hand-built M1 circuits: `Interpreter::new` and its circuits, the freeze at the M1 analog subset, the function of a `limit` site, every function of every body against the partials verifier, the shape of a body, the function of an `ac_stim` site and the `terms` function of a body. Each former target is a module of this one.

mod circuit;
mod frozen;
mod limit;
mod models;
mod partials;
mod shape;
mod stimulus;
mod terms;
