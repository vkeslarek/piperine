//! The standard library run from MSL source on the Rust backend: the items built on generics, the Appendix H rows and the four-state logic. Each former target is a module of this one.

#[path = "../source/bytes.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod bytes;
#[path = "../source/run.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod run;
#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod catalogue;
mod logic;
mod stdlib_generic;
