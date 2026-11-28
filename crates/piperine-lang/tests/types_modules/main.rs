//! The type checker over modules: the entry point and its determinism, params, nodes and placements, modules with a result, the analog structure forms, the run settings, reach through nested children, the subset boundary, the whole-model check and the buck example. Each former target is a module of this one.

#[path = "../support/generic_sources.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod generic_sources;
#[path = "../support/nested_reach.rs"]
mod nested;
#[path = "../support/pom.rs"]
mod support;

mod buck;
mod entry;
mod implicit;
mod model;
mod modules;
mod nested_reach;
mod results;
mod structure;
mod subset;
