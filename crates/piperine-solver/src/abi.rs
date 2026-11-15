//! The device-author surface: everything needed to implement a [`device::Device`]
//! and bind it into a circuit. Hosts use [`crate::prelude`]; device implementors
//! use this module.

// The device contract (abi.md): what a device author writes against.
pub use piperine_abi::{device, layout, raw, shape, thunk};
pub use crate::core::builder::CircuitBuilder;
pub use crate::core::circuit::CircuitInstance;
// Bind: what a netlist of device instances is made of.
pub use crate::core::bind::{Bound, Hold, NameMap, NetName, ParamValue, Port, Site};
// What a host names about a device
pub use crate::core::introspect::{ProbeSelection, Value, ValueKind};
// Naming
pub use piperine_analog_solver::math::linear::{AsIndex, Stamp};
pub use piperine_analog_solver::math::iv::InitialValue;
pub use piperine_analog_solver::netlist::{
    AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier, GND,
};
// Solution history + per-analysis options
pub use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
pub use crate::prelude::DcAnalysisResult;
pub use crate::analyses::noise::{Noise, NoiseKind};
pub use crate::analyses::transient::TransientAnalysisOptions;
// Integration (kernels read phase/coeffs)
pub use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
// Run config + results
pub use crate::analyses::{AnalysisKind, Context};
pub use piperine_analog_solver::tolerances::{Policy, Tolerances};
pub use crate::core::result::{NoiseContribution, Result, SolverStats};
pub use piperine_analog_solver::error::{Error, SolverDomain};
