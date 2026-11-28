//! The elaborator over analog code: the analog block, the forms of the analog operators, the buck's power stage and natures of every kind. Each former target is a module of this one.

#[path = "../support/buck.rs"]
#[allow(dead_code)]
mod buck;
#[path = "../support/elab.rs"]
#[allow(dead_code)]
mod support;

mod analog;
mod mixed;
mod natures;
mod operators;
