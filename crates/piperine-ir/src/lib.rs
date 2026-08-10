//! `piperine-ir` is the compiler's intermediate representation (MD-40:
//! `piperine-signal` becomes `piperine-ir`). The MD-36 purity invariant
//! survives verbatim and is now load-bearing for a new reason: an IR with
//! no in-tree dependency is the precondition for its reference interpreter
//! to be a trustworthy differential oracle against the JIT backends.
//!
//! This crate never stamps, never sees a matrix, never sees an `Element`,
//! and depends on no other crate in the tree — `tests/purity.rs` gates
//! that mechanically.
//!
//! The existing families — `behavior`/`events`/`limit`/`logic`/`measure`/
//! `ops`/`verify`/`wave` — are runtime-semantic families that will become
//! the first intrinsic-catalog entries, in place. The five-concept IR
//! core (value class, net, value graph, cell, block) is being built in
//! `core/` this phase.

pub mod behavior;
pub mod core;
pub mod events;
pub mod limit;
pub mod logic;
pub mod measure;
pub mod ops;
pub mod verify;
pub mod wave;
