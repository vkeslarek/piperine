//! The analog numeric engine: the error taxonomy (`error`), the circuit-agnostic
//! linear algebra (`math`) and the device assembly (`device`) that turns the
//! terms and partials of ABI devices into stamps, and the secant step of
//! crossing location (`crossing`). It names no digital concept
//! and depends on no crate of the workspace but `piperine-abi`.

pub mod config;
pub mod convergence;
pub mod crossing;
pub mod device;
pub mod error;
pub mod math;
pub mod netlist;
pub mod stamp_validation;
pub mod tolerances;
