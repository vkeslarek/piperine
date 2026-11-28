//! Value semantics run from MSL source on the Rust backend: structs and tuples, mutation, loops and patterns, the explicit model of unknowns, number widths, places and copies, and the correctness fixes. Each former target is a module of this one.


mod aggregates;
mod control;
mod explicit;
mod fixes;
mod number_widths;
mod value_semantics;
