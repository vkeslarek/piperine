//! The type checker over analog behavior and events: probes, contributions and the core primitives, the analog operators, events and reactions, and the natures of every kind. Each former target is a module of this one.

#[path = "../support/buck.rs"]
#[allow(dead_code)]
mod buck;
#[path = "../support/elab.rs"]
#[allow(dead_code)]
mod elaborated;
#[path = "../support/pom.rs"]
mod support;

mod analog;
mod analog_operators;
mod events;
mod natures;
