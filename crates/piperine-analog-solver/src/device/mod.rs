//! The device assembly (`.specs/features/device-abi/abi.md` §4, §9, §10): the
//! analog half of bind (`plan`) and, per solve, the evaluation of a device's
//! analog functions and the rows its terms and partials make. Devices are
//! called through `piperine_abi::caller::Handle`; nothing here is `unsafe`.

pub mod assembly;
mod ddt;
mod delay;
mod flow;
mod limit;
mod small_signal;
mod watch;
pub mod plan;

pub use assembly::{Ambient, Assembly, AssemblySnapshot, Rising};
pub use ddt::Stage;
pub use limit::LimitState;
pub use small_signal::{ComplexStamps, NoiseColumn, StimulusColumn, TermLinearization};
pub use watch::{Accepted, TraceRecord};
pub use plan::{AnalogBinding, AnalogInstance, AnalogPlan};

use crate::error::{Error, SolverDomain};

/// A device-side error of the analog assembly.
pub(crate) fn device_error(detail: String) -> Error {
    Error::simple(SolverDomain::Device, detail)
}
