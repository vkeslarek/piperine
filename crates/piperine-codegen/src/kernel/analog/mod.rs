//! Analog kernel compilation: a flattened analog body to native
//! residual/Jacobian/charge/force/noise functions.
//!
//! One [`AnalogKernel`] is compiled per module and shared (`Arc`) across
//! instances; instances own their parameter values and operator state.
//!
//! Every compiled function uses the same ABI:
//!
//! ```c
//! void fn(const f64 *volts, const f64 *params, const f64 *state,
//!         const SimCtx *sim, f64 *out);
//! ```
//!
//! `volts[i]` is the voltage at terminal `i` ([`AnalogKernel::terminals`]
//! order: ports first, then module-internal nodes); `state[i]` is the current
//! value of runtime-state slot `i` (serviced by the device between steps).
//!
//! The optional analog capabilities (reactive/charge, forces, limits, noise,
//! `ac_stim`) are grouped into their own sub-structs (`kernel/analog/{name}.rs`)
//! held as `Option<Capability>` on [`AnalogKernel`] — presence of the
//! capability *is* `Some(_)`; a `has_<cap>()` query is
//! `self.<cap>.is_some()` (or an emptiness check on the capability's inner
//! data), never a separately-tracked bool.

mod ac_stim;
mod compile;
mod forces;
mod kernel;
mod limits;
mod noise;
mod reactive;

pub use kernel::{
    AnalogKernel, Branch, CompiledEvent, CompiledTrigger, Disto2Pair, Disto3Triple, RuntimeState,
    RuntimeStateSpec,
};
use kernel::{AnalogCapability, AnalogCore, AnalogFn};
