//! The contract of the device ABI: the shipped constants and layouts hold their pinned values, the raw
//! views hold no public pointer, a shape built through the owned builder reads back through the raw
//! pointers, every shape-local rule of abi.md section 9 is enforced, and array pins validate their count.
//! Each former target is a module of this one.

mod array_pins;
mod contract;
mod layouts;
mod shape;
mod validate;
