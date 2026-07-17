//! # piperine
//!
//! The external Rust host interface to the Piperine analog/mixed-signal
//! simulator: elaborate a PHDL [`Design`](piperine_lang::Design), compile
//! it, run analyses, and read results — the complete external view of the
//! project (MD-19). The Python binding builds on this surface; the CLI
//! embeds it.
//!
//! ```text
//! parse_and_elaborate ─▶ Design ─▶ SimSession ─▶ lower ─▶ CircuitCompiler ─▶ solver
//!                                        │
//!                                        ▼
//!                            OpResult / Trace / AcTrace / NoiseTrace
//! ```

pub mod error;
pub mod hooks;
pub mod prelude;
pub mod results;
pub mod session;
pub mod waveform;

pub use error::Error;
pub use hooks::SimHooks;
pub use results::{NetRef, OpResult};
pub use session::{SimSession, SolverConfig};
pub use waveform::{AcTrace, ComplexWaveform, NoiseTrace, Trace, Waveform};
