//! The model of `piperine-ir`: the storage, the node set, the handles and the body tables, `Ir::check`, the dependency sets, and the IR forms of each feature (aggregates, amendments, arrays of carriers, control, digital nodes, generics, number widths, analog operators, resolved nets, analog structure), all on hand-built IRs. Each former target is a module of this one.

mod aggregates;
mod amendments;
mod carrier_arrays;
mod check;
mod control;
mod dependencies;
mod digital;
mod generics;
mod model;
mod number_widths;
mod operators;
mod printed;
mod resolved;
mod shape;
mod structure;
