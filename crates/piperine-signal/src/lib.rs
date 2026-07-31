//! `piperine-signal` computes values, derivatives, event predicates and
//! running reductions. It never stamps, never sees a matrix, never sees an
//! `Element`, and depends on no other crate in the tree.

pub mod behavior;
pub mod events;
pub mod limit;
pub mod ops;
