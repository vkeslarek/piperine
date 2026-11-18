//! The public surface a host needs to build a circuit, run an analysis, and
//! read results — gathered in one place so callers write `use
//! piperine_solver::prelude::*;` instead of reaching into module paths that are
//! free to move. Everything not re-exported here is an internal detail.
//!
//! Typical flow:
//! ```ignore
//! use piperine_solver::prelude::*;
//! let mut b = CircuitBuilder::new("top");
//! b.device(module, "x1", &[], &[Port::Node(b.ground())]);
//! let mut circuit = b.build()?;
//! let op = circuit.dc(Context::default())?.solve()?;
//! ```

// ── Building blocks ────────────────────────────────────────────────────────
pub use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier};
pub use crate::core::bind::{ParamValue, Port};
pub use crate::core::builder::CircuitBuilder;
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::introspect::{ProbeSelection, Value, ValueKind};
pub use crate::core::net::{Net, NetKind};

// ── Run configuration ──────────────────────────────────────────────────────
pub use crate::analyses::ac::AcSweepAnalysisOptions;
pub use crate::analyses::noise::{NoiseAnalysisOptions, NoiseKind};
pub use crate::analyses::tf::TransferFunctionAnalysisOptions;
pub use crate::analyses::pss::{PssAnalysisOptions, PssResult, PssStats};
pub use crate::analyses::pz::{PoleZeroOptions, PoleZeroSolver};
pub use crate::analyses::sens::{SensAnalysisOptions, SensResult};
pub use crate::analyses::adjoint::{AdjointOptions, AdjointResult, AdjointSolver};
pub use crate::analyses::sensitivity::{
    ParamDerivative, ParamRequest, ParamStampCollector, SensitivityEngine,
};
pub use crate::analyses::sp::{SpOptions, SpPort, SpSolver};
pub use crate::analyses::transient::{Keep, Reentry, TransientAnalysisOptions};
pub use crate::analyses::Context;
pub use piperine_analog_solver::tolerances::{Policy, Tolerances};
pub use crate::analyses::Solver;
pub use crate::protocol::{StepObserver, StepView};

// ── Convergence policy (opt-in customization) ──────────────────────────────
pub use piperine_analog_solver::config::{GminSchedule, Schedules, SourceSchedule, StepperGains, TraceFlags};
pub use piperine_analog_solver::convergence::{
    ConvergencePlan, DampContext, HomotopyStrategy, NewtonStrategy, NoDamping, NodeDamping, PiController, StepLimits,
    StepperStrategy,
};

// ── Results ────────────────────────────────────────────────────────────────
pub use crate::core::result::{
    AcAnalysisResult, AcAnalysisStep, DcAnalysisResult, NoiseAnalysisResult, NoiseContribution,
    PoleZeroResult, SpResult, TransferFunctionAnalysisResult, TransferType, TransientAnalysisResult, TransientStep, DigitalNets,
};

// ── Errors ─────────────────────────────────────────────────────────────────
pub use piperine_analog_solver::error::Error;
pub use crate::core::result::Result;
pub use crate::core::result::SolverStats;
