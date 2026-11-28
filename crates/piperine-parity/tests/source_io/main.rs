//! Input and output run from MSL source on the Rust backend: `table_model`, `emit!` in `analog`, noise and `simparam`. Each former target is a module of this one.

#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod noise;
mod session;
mod tables;
mod traces;
