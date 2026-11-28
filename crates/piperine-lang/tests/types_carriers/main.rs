//! The type checker over carriers: discrete carriers and definitions, arrays of carriers, bundles and their views, and handles. Each former target is a module of this one.

#[path = "../support/buck.rs"]
#[allow(dead_code)]
mod buck;
#[path = "../support/checked.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod checked;
#[path = "../support/generic_sources.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod generic_sources;
#[path = "../support/pom.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod bundles;
mod carrier_arrays;
mod carriers;
mod handles;
