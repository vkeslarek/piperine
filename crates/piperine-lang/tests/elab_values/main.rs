//! The elaborator over values and constants: value code lowered to the IR, number widths, aggregates, control, the explicit model of unknowns, constants and params, the entry point and the run settings. Each former target is a module of this one.

#[path = "../support/elab_forms.rs"]
#[allow(dead_code)]
mod elab_forms;
#[path = "../support/elab.rs"]
#[allow(dead_code)]
mod support;

mod aggregates;
mod control;
mod entry;
mod explicit;
mod implicit;
mod number_widths;
mod params;
mod values;
