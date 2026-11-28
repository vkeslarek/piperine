//! The type checker over compound values and control: unit enums, `Option`, tuples and `match`, aggregates (structs, tuples, arrays, slices), mutation, loops, exits and function values. Each former target is a module of this one.

#[path = "../support/checked.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod checked;
#[path = "../support/pom.rs"]
mod support;

mod aggregates;
mod compound;
mod control;
