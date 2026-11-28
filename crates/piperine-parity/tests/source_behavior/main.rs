//! Analog behavior run from MSL source on the Rust backend: `ddt` and `ddx`, contributions under control flow, `delay`, `slew`, `limexp`, values read within one instant and the environment functions. Each former target is a module of this one.


mod delay;
mod derivative;
mod environment;
mod limited_exponential;
mod partial;
mod slew;
mod switch;
mod wraps;
