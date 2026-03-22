//! # piperine-spice
//!
//! High-level orchestrator for ngspice simulations with process pool.
//!
//! This crate provides:
//! - Process pool for parallel simulations (num_cpus * 2 workers)
//! - JSON-based IPC protocol for communication with worker processes
//! - Safe, high-level API for running SPICE simulations
//!
//! ## Architecture
//!
//! ```text
//! NgspicePool (main process)
//!   ├─ Worker Process 1 (same binary re-executed with `--worker`, stdin/stdout JSON)
//!   ├─ Worker Process 2 (same binary re-executed with `--worker`, stdin/stdout JSON)
//!   ├─ Worker Process 3 (same binary re-executed with `--worker`, stdin/stdout JSON)
//!   └─ Worker Process N (same binary re-executed with `--worker`, stdin/stdout JSON)
//! ```
//!
//! Each worker process has its own ngspice instance (no thread-safety issues).
//! Workers are reused for multiple simulations via state reset commands.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use piperine_spice::NgspicePool;
//!
//! let mut pool = NgspicePool::with_size(4)?;  // 4 worker processes
//!
//! let result = pool.run_netlist(
//!     &[
//!         "V1 in 0 DC 5",
//!         "R1 in out 1k",
//!     ],
//!     "op"
//! )?;
//!
//! println!("{:?}", result);
//! ```

mod errors;
pub mod ngspice;
mod pool;
mod process;
pub mod protocol;

pub use errors::{Result, SpiceError};
pub use pool::NgspicePool;
pub use process::worker_main;
pub use protocol::{SimulationResult, WorkerRequest, WorkerResponse};
