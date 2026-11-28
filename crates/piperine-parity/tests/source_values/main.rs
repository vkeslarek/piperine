//! Value semantics run from MSL source on the Rust backend: structs and tuples, mutation, loops and patterns, the explicit model of unknowns, number widths, places and copies, and the correctness fixes. Each former target is a module of this one.

#[path = "../source/bytes.rs"]
mod bytes;
#[path = "../../../piperine-numbers/tests/floats/exact.rs"]
mod exact;
#[path = "../../../piperine-numbers/tests/floats/exact_algebra.rs"]
mod exact_algebra;
#[path = "../../../piperine-numbers/tests/floats/exact_integers.rs"]
mod exact_integers;
#[path = "../../../piperine-numbers/tests/floats/exact_nearest.rs"]
mod exact_nearest;
#[path = "../../../piperine-numbers/tests/floats/exact_parts.rs"]
mod exact_parts;
#[path = "../../../piperine-numbers/tests/floats/exact_root.rs"]
mod exact_root;
#[path = "../../../piperine-numbers/tests/floats/exact_values.rs"]
mod exact_values;
#[path = "../source/operators.rs"]
mod operators;
#[path = "../../../piperine-numbers/tests/floats/oracle.rs"]
mod oracle;
#[path = "../source/run.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod run;
#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;
#[path = "../source/widths.rs"]
mod widths;

mod aggregates;
mod control;
mod explicit;
mod fixes;
mod number_widths;
mod value_semantics;
