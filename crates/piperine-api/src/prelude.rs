//! One-import host surface: the session, result objects, and waveforms, plus
//! re-exports of the lang/codegen/solver public faces — the complete
//! external view of the project (MD-20).

pub use crate::error::Error;
pub use crate::fourier::{FourierComponent, FourierResult};
pub use crate::hooks::SimHooks;
pub use crate::results::{
    DistoResult, NetRef, NetSelector, OpResult, PssResult, PzResult, SParamResult, SensResult, TfResult,
};
pub use crate::session::{Grid, Nested, Scale, Session, SessionBuilder, SimSession, SolverConfig, Sweep, SweepPoint};
pub use crate::units::{Freq, Time};
pub use crate::waveform::{AcTrace, ComplexWaveform, CrossDirection, NoiseTrace, Trace, Waveform};
pub use piperine_codegen::device::{CircuitBuildInfo, CircuitCompiler, DeviceProvider};
pub use piperine_lang::{Design, SourceMap, parse_and_elaborate, parse_and_elaborate_seeded};
pub use piperine_solver::prelude::*;
