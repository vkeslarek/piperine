//! The elaborator over structure: the analog-structure forms, modules and placements, reach through nested children, generation, generic modules and instances, handles, templates, files read at elaboration, the forms that are not lowered yet, and the buck example. Each former target is a module of this one.


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
mod refused_placements;
