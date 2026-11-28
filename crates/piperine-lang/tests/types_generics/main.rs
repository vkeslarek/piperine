//! The type checker over generics: generic items and their bounds, generic modules, generation, templates as values, and traits with their `impl`s. Each former target is a module of this one.

#[path = "../support/generic_sources.rs"]
mod generic_sources;
#[path = "../support/pom.rs"]
mod support;

mod generation;
mod generic_modules;
mod generics;
mod templates;
mod traits;
