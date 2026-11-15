//! The device-author surface: everything needed to implement [`Element`].
//! Hosts use [`crate::prelude`]; element implementors use this module.

// The contract
pub use crate::core::element::{
    AnalogDevice, Element, ElementCapabilities,
    ElementCheckpoint, Introspect, LimitReason, LimitingReport,
};
pub use piperine_digital_solver::interface::DigitalDevice;
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::introspect::{
    Bounds, Direction, Domain, Invalidation, ModelDescriptor, ObservableDescriptor, ObservableKind,
    ParamDescriptor, ParamError, ParamScope, ProbeSelection, QueryDescriptor, QueryKind,
    TerminalDescriptor, TerminalKind,
    Value, ValueKind, SignConvention,
};
// Stamping + naming
pub use piperine_analog_solver::math::linear::{AsIndex, Stamp};
pub use piperine_analog_solver::math::iv::InitialValue;
pub use piperine_analog_solver::netlist::{
    AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier, GND,
};
// Solution history + per-analysis states/contexts
pub use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
pub use crate::analyses::ac::AcAnalysisContext;
pub use crate::analyses::dc::DcAnalysisState;
pub use crate::prelude::DcAnalysisResult;
pub use crate::analyses::noise::{Noise, NoiseKind};
pub use crate::analyses::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisState,
};
// Integration (kernels read phase/coeffs)
pub use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
// Digital evaluation
pub use piperine_digital_solver::interface::{interpolate_a2d_crossing, CrossDir, DigitalPorts, EvalCtx, EventSink, QueueSink};
pub use piperine_digital_solver::events::{DigitalEvent, DigitalNet, LogicValue};
pub use piperine_digital_solver::drivers::{DriverId, DriverKind, DriverRecord, DriverTable, Strength};
// Path delays / timing-check limits (T19/T20, LRMW3-06/LRMW3-07) — a
// codegen circuit builder populates these per destination net at build
// time, so they need an ABI-surface re-export like every other
// codegen-visible digital type on this page.
pub use piperine_digital_solver::state::DigitalState;
pub use piperine_digital_solver::topology::DigitalTopology;
// Run config + results device code touches
pub use crate::analyses::{AnalysisKind, Context};
pub use piperine_analog_solver::tolerances::{Policy, Tolerances};
pub use crate::analyses::events::{
    EventEntry, EventKind, EventPriority, EventQueue, EventSource, EventTarget, RollbackBehavior,
};
pub use crate::core::result::{NoiseContribution, Result, SolverStats};
pub use piperine_analog_solver::error::{Error, SolverDomain};
// Element lifecycle allocator (ABI-09)
pub use crate::core::builder::UnknownAllocator;
// The device contract (abi.md): what a device author writes against.
pub use piperine_abi::{device, layout, raw, shape, thunk};
pub use crate::core::builder::CircuitBuilder;
// Bind (the device contract): what a netlist of device instances is made of.
pub use crate::core::bind::{Bound, Hold, NameMap, NetName, ParamValue, Port, Site};
