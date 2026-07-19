//! # piperine-api
//!
//! The external Rust host interface to the Piperine analog/mixed-signal
//! simulator: elaborate a PHDL [`Design`](piperine_lang::Design), compile
//! it, run analyses, and read results — the complete external view of the
//! project (MD-20). The Python binding builds on this surface; the CLI
//! embeds it; the root `piperine` crate re-exports it.
//!
//! ```text
//! parse_and_elaborate ─▶ Design ─▶ SimSession ─▶ lower ─▶ CircuitCompiler ─▶ solver
//!                                        │
//!                                        ▼
//!                            OpResult / Trace / AcTrace / NoiseTrace
//! ```

pub mod error;
pub mod fourier;
pub mod hooks;
pub mod prelude;
pub mod results;
pub mod session;
pub mod waveform;

pub use error::Error;
pub use fourier::{FourierComponent, FourierResult};
pub use hooks::SimHooks;
pub use piperine_solver::prelude::{PoleZeroResult, SpResult};
pub use results::{NetRef, OpResult, PssResult, SensResult};
pub use session::{SimSession, SolverConfig};
pub use waveform::{AcTrace, ComplexWaveform, NoiseTrace, Trace, Waveform};
