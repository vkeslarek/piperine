//! The elaborator over digital structure: carriers, drives, events and reactions, modules with a result, arrays of carriers and bundles. Each former target is a module of this one.

#[path = "../support/buck.rs"]
#[allow(dead_code)]
mod buck;
#[path = "../support/elab.rs"]
#[allow(dead_code)]
mod support;

mod bundles;
mod carrier_arrays;
mod digital;
mod results;
