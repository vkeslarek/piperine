//! The policy-parameterized optimization/centering driver (dv-gradients
//! T16, DVG-14): ONE driver type ([`Driver`]) with three policies —
//! optimize, center, high-sigma — over the restamp loop, so the three
//! policies never drift into three separate engines that duplicate the
//! same margin-reading, restamp-writing plumbing (design §C4).
//!
//! # The `collect`-posture invariant (T16 AC2)
//!
//! [`Driver::run`] ALWAYS solves in `collect` posture, regardless of the
//! `checks` field on the [`crate::SolverConfig`] the caller passes in — an
//! optimizer that aborted on its own infeasible iterates could not
//! search. See `entry.rs`.
//!
//! # Policies
//!
//! - [`Policy::Optimize`] (T17/T18, DVG-14/15): feasibility phase, then
//!   objective phase, gradient-based with a black-box fallback. See
//!   `optimize.rs`/`blackbox.rs`.
//! - [`Policy::Center`] (T19, DVG-19): maximize `min_i (m_i / σ_i)`. See
//!   `center.rs`.
//! - [`Policy::HighSigma`]: importance-sampling tail estimation (T20) —
//!   out of this delivery's scope; a structural placeholder that fails
//!   loud.

mod blackbox;
mod center;
mod entry;
mod optimize;
mod support;

pub use center::{CenterResult, CenterSpec, SigmaScheme};
pub use entry::{Driver, DriverOutcome, Policy};
pub use optimize::{Engine, OptimizeResult, OptimizeSpec};

pub(crate) use support::{nominal_param_value, read_metric};
