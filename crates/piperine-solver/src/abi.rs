//! The device-author surface: everything needed to implement [`Element`].
//! Hosts use [`crate::prelude`]; element implementors use this module.

// The contract
pub use crate::core::element::{ConvergenceHint, Element, ElementCapabilities};
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::introspect::{
    Bounds, Direction, Domain, Invalidation, ParamDescriptor, ParamError,
    ParamScope, QueryDescriptor, QueryKind, TerminalDescriptor,
    Value, ValueKind,
    // TODO(T10): SignConvention, NoiseKind
};
// Stamping + naming
pub use crate::math::linear::{AsIndex, Stamp};
pub use crate::math::iv::InitialValue;
pub use crate::analog::{
    AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier, GND,
};
// Solution history + per-analysis states/contexts
pub use crate::math::circular_array::CircularArrayBuffer2;
pub use crate::analysis::ac::AcAnalysisContext;
pub use crate::analysis::dc::DcAnalysisState;
pub use crate::prelude::DcAnalysisResult;
pub use crate::analysis::noise::Noise;
pub use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisState,
};
// Integration (kernels read phase/coeffs)
pub use crate::math::integration::{IntegrationMethod, TrBdf2, TrBdf2Phase};
pub use crate::math::unit::Second;
// Digital evaluation
pub use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink, QueueSink};
pub use crate::digital::{DigitalEvent, DigitalNet, LogicValue};
pub use crate::digital::state::DigitalState;
pub use crate::digital::topology::DigitalTopology;
// Run config + results device code touches
pub use crate::solver::{Context, Policy, Tolerances};
pub use crate::result::{Result, SolverStats};
pub use crate::error::{Error, SolverDomain};
// Element lifecycle allocator (ABI-09)
// TODO(T9): pub use crate::core::builder::UnknownAllocator;
