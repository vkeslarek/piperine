//! # piperine-bench
//!
//! The effectful runtime for PHDL's `bench` block (`crates/piperine-lang/docs/piperine-bench/docs/SPEC.md`):
//! runs analyses over an elaborated [`piperine_lang::Design`], measures
//! through result objects, and stages parameter overrides.
//!
//! ```text
//! Design (+ bench blocks) ──BenchRunner──▶ per-entry-point fork ──▶ SimHost
//!                                                                    │
//!                          piperine_lang::eval::Interpreter ◀────────┘
//!                                     │ syscalls ($op, $assert, ...)
//!                                     ▼
//!                          SimSession ──▶ ppr_to_ir ──▶ CircuitCompiler ──▶ solver
//! ```
//!
//! [`eval::Interpreter`]: piperine_lang::eval::Interpreter

pub mod error;
pub mod host;
pub mod objects;
pub mod runner;
pub mod session;
pub mod tasks;
pub mod waveform;

pub use error::BenchError;
pub use objects::{InstanceRef, NetRef, OpResult};
pub use runner::{BenchOutcome, BenchReport, BenchResult, BenchRunner};
pub use session::{SimSession, SolverConfig};
pub use waveform::{AcTrace, ComplexWaveform, NoiseTrace, Trace, Waveform};
