//! The public surface a host needs to build a circuit, run an analysis, and
//! read results — gathered in one place so callers write `use
//! piperine_solver::prelude::*;` instead of reaching into module paths that are
//! free to move. Everything not re-exported here is an internal detail.
//!
//! Typical flow:
//! ```ignore
//! use piperine_solver::prelude::*;
//! let mut circuit = CircuitInstance::from_devices_and_netlist("top", devices, netlist);
//! let op = circuit.dc(Context::default())?.solve()?;
//! ```

// ── Building blocks ────────────────────────────────────────────────────────
pub use crate::analog::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier};
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::element::{ConvergenceHint, Element, ElementCapabilities};
pub use crate::core::introspect::{
    Bounds, Direction, Domain, Invalidation, ParamDescriptor, ParamError, ParamScope,
    QueryDescriptor, QueryKind, SignConvention, TerminalDescriptor, Value, ValueKind,
};
pub use crate::core::net::{Net, NetKind};
pub use crate::digital::{DigitalNet, LogicValue};

// ── Run configuration ──────────────────────────────────────────────────────
pub use crate::analysis::ac::AcSweepAnalysisOptions;
pub use crate::analysis::noise::{NoiseAnalysisOptions, NoiseKind};
pub use crate::analysis::tf::TransferFunctionAnalysisOptions;
pub use crate::analysis::transient::TransientAnalysisOptions;
pub use crate::math::integration::IntegrationMethod;
pub use crate::solver::Context;
pub use crate::solver::{Policy, Tolerances};

// ── Convergence policy (opt-in customization) ──────────────────────────────
pub use crate::solver::convergence::{
    ConvergencePlan, DampedNewton, HomotopyStrategy, NewtonStrategy, PiController, StepperStrategy,
};

// ── Results ────────────────────────────────────────────────────────────────
pub use crate::result::{
    AcAnalysisResult, AcAnalysisStep, DcAnalysisResult, NoiseAnalysisResult, NoiseContribution,
    TransferFunctionAnalysisResult, TransferType, TransientAnalysisResult, TransientStep,
};

// ── Errors ─────────────────────────────────────────────────────────────────
pub use crate::error::Error;
pub use crate::result::Result;
pub use crate::result::SolverStats;
