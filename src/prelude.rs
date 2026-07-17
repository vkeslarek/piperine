//! One-import host surface: the session, result objects, and waveforms, plus
//! re-exports of the lang/codegen/solver public faces — the complete
//! external view of the project (MD-19).

pub use crate::error::Error;
pub use crate::results::{NetRef, OpResult};
pub use crate::session::{SimSession, SolverConfig};
pub use crate::waveform::{AcTrace, ComplexWaveform, NoiseTrace, Trace, Waveform};
pub use piperine_codegen::device::{CircuitBuildInfo, CircuitCompiler, DeviceProvider};
pub use piperine_lang::{Design, SourceMap, parse_and_elaborate, parse_and_elaborate_seeded};
pub use piperine_solver::prelude::*;
