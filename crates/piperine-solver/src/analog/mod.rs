//! Analog naming: the `Netlist` mapping nodes and branch currents to MNA
//! unknown indices.

pub mod netlist;

pub use netlist::{
    AnalogReference, AnalogVariable, BranchIdentifier, GND, Netlist, NodeIdentifier,
};
