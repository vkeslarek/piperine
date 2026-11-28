//! `inst.name` through a child, and through children of children at any depth (§8.2.8, §8.4.8): the frames a body reads a placed
//! child through, the params a placement binds in them, and the demands a read through a child of a child makes of the bodies on
//! its path.
//!
//! * `fields.rs`: the reach frame of a placement and a field read through it (a node, a carrier, a param, a `let`).
//! * `placement_params.rs`: the params a placement binds in its module, shared with the hoisted frame (`hoist.rs`).
//! * `demands.rs`: what a read through a child of a child asks of each body on its path.
//! * `seed.rs`: the demands the text names, found before the first run.

pub(in crate::elab) mod demands;
pub(in crate::elab) mod fields;
pub(in crate::elab) mod placement_params;
pub(in crate::elab) mod seed;
