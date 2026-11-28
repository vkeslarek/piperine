//! Structure run from MSL source on the Rust backend: generation, generic items and modules, handles, templates, reach through nested children, modules with values, results and feedback, params and `inst.name`. Each former target is a module of this one.

#[path = "../source/bytes.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod bytes;
#[path = "../source/operators.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod operators;
#[path = "../source/run.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod run;
#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod generation;
mod generic_modules;
mod generics;
mod handles;
mod modules;
mod nested_reach;
mod params;
mod reach;
mod templates;
