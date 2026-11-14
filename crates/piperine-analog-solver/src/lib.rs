//! The analog numeric engine: the error taxonomy (`error`) and the circuit-agnostic
//! linear algebra (`math`). It names no digital concept and depends on no other
//! crate of the workspace.

pub mod config;
pub mod convergence;
pub mod error;
pub mod math;
pub mod netlist;
pub mod stamp_validation;
pub mod tolerances;
