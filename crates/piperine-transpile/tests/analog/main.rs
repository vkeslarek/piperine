//! The generated code of the analog forms, on hand-built IR compiled once with `rustc` and evaluated through the device ABI: an `if` on a carrier in an `analog` block, the compiled functions against the interpreter's, the analog operators, the linear-term rule and the analog structure forms. Each former target is a module of this one.

mod analog_if;
mod both;
mod differential;
mod operators;
mod residue;
mod structure;
#[path = "../support/compiled.rs"]
mod compiled;
