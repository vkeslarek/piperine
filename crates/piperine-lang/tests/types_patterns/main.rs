//! The type checker over compound values and control: unit enums, `Option`, tuples and `match`, aggregates (structs, tuples, arrays, slices), mutation, loops, exits and function values. Each former target is a module of this one.


mod aggregates;
mod compound;
mod control;
