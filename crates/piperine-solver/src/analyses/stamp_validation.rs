//! §7.3.2.1 (T35/LRMW3-18) — a non-finite device stamp names the branch.
//!
//! "The result of an analog expression, including a contribution, shall
//! not evaluate to NaN or infinity" (LRM §7.3.2.1). Checked right where a
//! single device's own stamps are collected (`AnalogDevice::load_dc`/
//! `load_transient`'s return value), *before* they ever reach the shared
//! linear system. This is what makes the branch nameable: by the time a
//! non-finite value has been summed into the assembled matrix/RHS
//! alongside every other device's contributions, there is no way back to
//! which one produced it — the pre-existing `math/newton_raphson.rs`
//! guard (checking the *solved* vector after a full Newton iteration) can
//! only report a generic "Linear solver returned NaN/Inf" convergence
//! failure, not a named branch.

use crate::analog::AnalogReference;
use crate::error::{Error, SolverDomain};
use crate::math::linear::Stamp;

/// Returns `None` when every stamp is finite (the common case, checked
/// with zero allocation); otherwise a typed `SolverDomain::Element` error
/// naming `device_name` and citing §7.3.2.1.
pub fn non_finite_stamp_error(device_name: &str, stamps: &[Stamp<AnalogReference, f64>]) -> Option<Error> {
    let is_non_finite = stamps.iter().any(|s| match s {
        Stamp::Matrix(_, _, v) | Stamp::Rhs(_, v) => !v.is_finite(),
    });
    if !is_non_finite {
        return None;
    }
    Some(Error::simple(
        SolverDomain::Element,
        format!(
            "device `{device_name}` contributed a NaN/±infinity value (LRM §7.3.2.1: the \
             result of an analog expression, including a contribution, shall not evaluate to \
             NaN or infinity)"
        ),
    ))
}
