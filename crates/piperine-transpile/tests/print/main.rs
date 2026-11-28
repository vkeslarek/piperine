//! The printer of `piperine-transpile` on hand-built IR: aggregate values, control flow, the derive functions, the names of generic instances and the number widths, printed, compiled with the runtime by `rustc` and run. Each former target is a module of this one.

mod aggregates;
mod control;
mod derive;
mod generics;
mod number_widths;
mod probe;
