//! Analog behavior run from MSL source on the Rust backend: `ddt` and `ddx`, contributions under control flow, `delay`, `slew`, `limexp`, values read within one instant and the environment functions. Each former target is a module of this one.

#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod delay;
mod derivative;
mod environment;
mod limexp;
mod partial;
mod slew;
mod switch;
mod wraps;
