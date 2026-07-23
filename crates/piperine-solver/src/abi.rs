//! The device-author surface: everything needed to implement [`Element`].
//! Hosts use [`crate::prelude`]; element implementors use this module.

// The contract
pub use crate::core::element::{
    AnalogDevice, DigitalDevice, Element, ElementCapabilities,
    ElementCheckpoint, Introspect, LimitReason, LimitingReport,
};
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::introspect::{
    Bounds, Direction, Domain, Invalidation, ModelDescriptor, ParamDescriptor, ParamError,
    ParamScope, QueryDescriptor, QueryKind, TerminalDescriptor, TerminalKind,
    Value, ValueKind, SignConvention,
};
// Stamping + naming
pub use crate::math::linear::{AsIndex, Stamp};
pub use crate::math::iv::InitialValue;
pub use crate::analog::{
    AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier, GND,
};
// Solution history + per-analysis states/contexts
pub use crate::math::circular_array::CircularArrayBuffer2;
pub use crate::analyses::ac::AcAnalysisContext;
pub use crate::analyses::dc::DcAnalysisState;
pub use crate::analyses::disto::{Disto2, Disto3};
pub use crate::prelude::DcAnalysisResult;
pub use crate::analyses::noise::{Noise, NoiseKind};
pub use crate::analyses::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisState,
};
// Integration (kernels read phase/coeffs)
pub use crate::math::integration::{TrBdf2, TrBdf2Phase};
// Digital evaluation
pub use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink, QueueSink};
pub use crate::digital::{DigitalEvent, DigitalNet, LogicValue};
pub use crate::digital::state::DigitalState;
pub use crate::digital::topology::DigitalTopology;
// Run config + results device code touches
pub use crate::analyses::{Context, Policy, Tolerances};
pub use crate::result::{NoiseContribution, Result, SolverStats};
pub use crate::error::{Error, SolverDomain};
// Element lifecycle allocator (ABI-09)
pub use crate::core::builder::UnknownAllocator;
