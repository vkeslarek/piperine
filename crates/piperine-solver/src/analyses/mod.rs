//! The analyses layer (design §1/§2 — Scheme B): one module per analysis,
//! each holding both its request/state types (what element and host
//! exchange) and its driver (how it runs). The run configuration every
//! driver speaks — `Context` (immutable `Tolerances`) and `Policy`
//! (per-analysis convergence tunables, MD-04) — lives in `context.rs`, the
//! `Solver` host entry that hands out the analyses in `solver.rs`, the
//! tunable literals in `config.rs`, and the Newton/homotopy/stepper
//! machinery in `convergence.rs`. A driver may call down into `analog`,
//! `digital`, `math`, and read config — never sideways into another
//! analysis, never up into the host.

pub mod ac;
pub mod adjoint;
pub mod config;
pub mod context;
pub mod convergence;
pub mod dc;
pub mod disto;
pub mod events;
pub mod noise;
pub mod pss;
pub mod pz;
pub mod sens;
pub mod sensitivity;
pub mod solver;
pub mod sp;
pub mod tf;
pub mod transient;

pub use context::{AnalysisKind, CheckPosture, Context, CoverPosture, Policy, RequireScope, Tolerances};
pub use solver::Solver;

/// §7.3.2.1 (T35/LRMW3-18): "the result of an analog expression ... shall
/// not evaluate to NaN or infinity" — checked right where a single
/// device's own stamps are collected (`AnalogDevice::load_dc`/`load_transient`'s
/// return value), *before* they ever reach the shared linear system. This
/// is what makes the branch nameable: by the time a non-finite value has
/// been summed into the assembled matrix/RHS alongside every other
/// device's contributions, there is no way back to which one produced it
/// — the pre-existing `math/newton_raphson.rs` guard (checking the
/// *solved* vector after a full Newton iteration) can only report a
/// generic "Linear solver returned NaN/Inf" convergence failure, not a
/// named branch. Returns `None` when every stamp is finite (the common
/// case, checked with zero allocation).
pub fn non_finite_stamp_error(
    device_name: &str,
    stamps: &[crate::math::linear::Stamp<crate::analog::AnalogReference, f64>],
) -> Option<crate::error::Error> {
    use crate::math::linear::Stamp;
    let is_non_finite = stamps.iter().any(|s| match s {
        Stamp::Matrix(_, _, v) | Stamp::Rhs(_, v) => !v.is_finite(),
    });
    if !is_non_finite {
        return None;
    }
    Some(crate::error::Error::simple(
        crate::error::SolverDomain::Element,
        format!(
            "device `{device_name}` contributed a NaN/±infinity value (LRM §7.3.2.1: the \
             result of an analog expression, including a contribution, shall not evaluate to \
             NaN or infinity)"
        ),
    ))
}
