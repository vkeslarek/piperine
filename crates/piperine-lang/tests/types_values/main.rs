//! The type checker (`Types::check`) over value expressions and their numbers: literals, operators and the method catalogue, the integer and float widths, the operator traits, places and copies, and the explicit model of unknowns. Each former target is a module of this one.

#[path = "../support/checked.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod checked;
#[path = "../support/pom.rs"]
mod support;

mod explicit;
mod integers;
mod number_widths;
mod operators;
mod value_semantics;
mod values;
