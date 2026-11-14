//! The analog numeric engine: the error taxonomy (`error`) and the circuit-agnostic
//! linear algebra (`math`). It names no digital concept and depends on no other
//! crate of the workspace.

pub mod config;
pub mod error;
pub mod math;
pub mod netlist;
pub mod tolerances;
