//! The elaborator over structure: the analog-structure forms, modules and placements, reach through nested children, generation, generic modules and instances, handles, templates, files read at elaboration, the forms that are not lowered yet, and the buck example. Each former target is a module of this one.

#[path = "../support/buck.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod buck;
#[path = "../support/elab_forms.rs"]
#[allow(dead_code)]
mod elab_forms;
#[path = "../support/nested_reach.rs"]
mod nested;
#[path = "../support/elab.rs"]
#[allow(dead_code)]
mod support;

mod buck_converter;
mod files;
mod generation;
mod generic_modules;
mod generics;
mod handles;
mod instances;
mod nested_reach;
mod structure;
mod templates;
mod unlowered;
