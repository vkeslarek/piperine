//! # piperine-api
//!
//! The external Rust host interface to the Piperine analog/mixed-signal
//! simulator: elaborate a PHDL [`Design`](piperine_lang::Design), compile
//! it, run analyses, and read results — the complete external view of the
//! project (MD-20). The Python binding builds on this surface; the CLI
//! embeds it; the root `piperine` crate re-exports it.
//!
//! ```text
//! parse_and_elaborate ─▶ Design ─▶ Session ─▶ lower ─▶ CircuitCompiler ─▶ solver
//!                                        │
//!                                        ▼
//!                            OpResult / Trace / AcTrace / NoiseTrace
//! ```

pub mod aging;
pub mod device;
pub mod driver;
pub mod error;
pub mod fourier;
pub mod gradient;
pub mod hooks;
pub mod margins;
pub mod model;
pub mod monte_carlo;
pub mod prelude;
pub mod reduced;
pub mod results;
pub mod sampling;
pub mod session;
pub mod units;
pub mod waveform;

pub use aging::{age, AgingResult, AgingSpec, DriftRate, StressSource};
pub use driver::{
    CenterResult, CenterSpec, Driver, DriverOutcome, Engine, HighSigmaEstimator, HighSigmaResult,
    HighSigmaSpec, OptimizeResult, OptimizeSpec, Policy, SigmaScheme,
};
pub use device::{
    limiting_report, BehaviorStamp, Ctx, HostAdapter, HostDeviceRegistry, HostDeviceSpec, HostElement, MeasureFeed,
    StampError, StampSink, StateView, WaveStamp,
};
pub use error::Error;
pub use fourier::{FourierComponent, FourierResult};
pub use gradient::{
    classify_margin_gradient, Gradient, MarginDifferentiability, MarginGradientResult,
    Sensitivity, SensitivityMethod,
};
pub use hooks::SimHooks;
pub use margins::{ConstraintCatalog, MarginEntry, MarginShape, MarginValue, MarginsResult};
pub use monte_carlo::{MonteCarloResult, SampleOutcome};
pub use piperine_solver::prelude::{
    Bounds, Invalidation, ModelDescriptor, ObservableDescriptor, ObservableKind, ParamDescriptor,
    ParamScope, TerminalDescriptor, TerminalKind,
};
pub use piperine_solver::abi::NoiseContribution;
pub use results::{
    DistoResult, InstanceView, NetRef, NetSelector, OpResult, PssResult, PzResult, SParamResult, SensResult,
    TfResult,
};
pub use sampling::{SampleDraw, SampleEntry, SamplePlan, SampleScope};
pub use session::{Grid, Nested, Scale, Session, SessionBuilder, SolverConfig, Sweep, SweepPoint};
pub use units::{Freq, Time};
pub use waveform::{AcTrace, ComplexWaveform, CrossDirection, NoiseTrace, Trace, Waveform};
