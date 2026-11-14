//! The device-author surface: everything needed to implement [`Element`].
//! Hosts use [`crate::prelude`]; element implementors use this module.

// The contract
pub use crate::core::element::{
    AnalogDevice, DigitalDevice, Element, ElementCapabilities,
    ElementCheckpoint, Introspect, LimitReason, LimitingReport,
};
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::validation::{ValidationFinding, ValidationReport, ValidationSeverity};
pub use crate::core::coverage::{CoverageReport, ReportedCoverCross, ReportedCoverPoint};
pub use crate::core::margin::{MarginKind, MarginReport, ReportedMargin};
pub use crate::core::introspect::{
    Bounds, Direction, Domain, Invalidation, ModelDescriptor, ObservableDescriptor, ObservableKind,
    ParamDescriptor, ParamError, ParamScope, ProbeSelection, QueryDescriptor, QueryKind, simprobe, SimprobeError,
    TerminalDescriptor, TerminalKind,
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
pub use crate::prelude::DcAnalysisResult;
pub use crate::analyses::noise::{Noise, NoiseKind};
pub use crate::analyses::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisState,
};
// Integration (kernels read phase/coeffs)
pub use crate::math::integration::{TrBdf2, TrBdf2Phase};
// Digital evaluation
pub use crate::digital::interface::{interpolate_a2d_crossing, CrossDir, DigitalPorts, EvalCtx, EventSink, QueueSink};
pub use crate::digital::{DigitalEvent, DigitalNet, DriverId, DriverKind, DriverRecord, DriverTable, LogicValue, Strength};
// Path delays / timing-check limits (T19/T20, LRMW3-06/LRMW3-07) — a
// codegen circuit builder populates these per destination net at build
// time, so they need an ABI-surface re-export like every other
// codegen-visible digital type on this page.
pub use crate::digital::{DelayTable, PathDelay, TimingCheckKind, TimingCheckLimit, TimingCheckTable};
pub use crate::digital::regions::{Region, RegionAction, RegionEvent, RegionOutcome, RegionQueue};
pub use crate::digital::state::DigitalState;
pub use crate::digital::topology::DigitalTopology;
// Run config + results device code touches
pub use crate::analyses::{AnalysisKind, CheckPosture, Context, CoverPosture, Policy, RequireScope, Tolerances};
pub use crate::analyses::events::{
    EventEntry, EventKind, EventPriority, EventQueue, EventSource, EventTarget, RollbackBehavior,
};
pub use crate::core::result::{NoiseContribution, Result, SolverStats};
pub use crate::error::{Error, SolverDomain};
// Element lifecycle allocator (ABI-09)
pub use crate::core::builder::UnknownAllocator;
