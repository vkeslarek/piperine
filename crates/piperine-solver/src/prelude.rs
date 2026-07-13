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
pub use crate::core::element::{Element, ElementCapabilities};
pub use crate::digital::{DigitalNet, LogicValue};

// ── Run configuration ──────────────────────────────────────────────────────
pub use crate::analysis::ac::AcSweepAnalysisOptions;
pub use crate::analysis::noise::NoiseAnalysisOptions;
pub use crate::analysis::tf::TransferFunctionAnalysisOptions;
pub use crate::analysis::transient::TransientAnalysisOptions;
pub use crate::analysis::truncation::IntegrationMethod;
pub use crate::solver::Context;

// ── Convergence policy (opt-in customization) ──────────────────────────────
pub use crate::solver::convergence::{ConvergencePlan, HomotopyStrategy};

// ── Results ────────────────────────────────────────────────────────────────
pub use crate::analysis::ac::{AcAnalysisResult, AcAnalysisStep};
pub use crate::analysis::dc::DcAnalysisResult;
pub use crate::analysis::noise::NoiseAnalysisResult;
pub use crate::analysis::tf::TransferFunctionAnalysisResult;
pub use crate::analysis::transient::{TransientAnalysisResult, TransientStep};

// ── Errors ─────────────────────────────────────────────────────────────────
pub use crate::error::Error;
pub use crate::result::Result;
