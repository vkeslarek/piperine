//! Carriers run from MSL source on the Rust backend: arrays of carriers, bundles, natures of every kind, and resolved nets with the net modules. Each former target is a module of this one.

#[path = "../source/run.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod run;
#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod bundles;
mod carrier_arrays;
mod natures;
mod nets;
mod resolved;
