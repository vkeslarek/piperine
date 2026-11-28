//! Execution run from MSL source: commit semantics, events and the settle commit, time and timers, and records. Each former target is a module of this one.

#[path = "../buck/support.rs"]
#[allow(dead_code)] // item scope: this target needs only part of the module
mod support;

mod commits;
mod events;
mod records;
mod time;
