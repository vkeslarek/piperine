//! Run configuration: the sweep geometry a frequency analysis takes
//! ([`Scale`]) and the tolerances + convergence tunables every analysis reads
//! before it solves ([`SolverConfig`]).

use piperine_solver::prelude::{Context, Policy};

/// Frequency-sweep geometry (HOST-23): `Lin` steps `points` values evenly
/// over `[fstart, fstop]`; `Dec`/`Oct` step logarithmically (decade/octave
/// per `points`) — the same three-way choice the prelude's `enum Scale`
/// (and the Python facade's `Scale`) already name. `impl Into<bool>` lets an
/// analysis's `logarithmic` argument accept either a bare `bool` (unchanged
/// for every existing caller — `bool: Into<bool>` via the identity `From`)
/// or a `Scale` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Lin,
    Dec,
    Oct,
}

impl Scale {
    /// `true` for `Dec`/`Oct` (a logarithmic sweep), `false` for `Lin` —
    /// the boolean every sweep-options struct actually stamps.
    pub fn is_logarithmic(&self) -> bool {
        !matches!(self, Scale::Lin)
    }
}

impl From<Scale> for bool {
    fn from(s: Scale) -> bool {
        s.is_logarithmic()
    }
}

/// Analysis configuration (tolerances + convergence tunables) read before an
/// analysis runs.
#[derive(Debug, Clone)]
pub struct SolverConfig {
    pub temperature: f64,
    pub reltol: f64,
    pub abstol: f64,
    pub gmin: f64,
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
}

impl Default for SolverConfig {
    fn default() -> Self {
        let tol = piperine_solver::prelude::Tolerances::default();
        let policy = Policy::default();
        Self {
            temperature: tol.temperature,
            reltol: tol.reltol,
            abstol: tol.abstol,
            gmin: tol.gmin,
            max_iter: policy.max_iter,
            dc_damp_tolerance: policy.dc_damp_tolerance,
        }
    }
}

impl SolverConfig {
    /// The shared solver [`Context`] (tolerances) this config maps to.
    /// Public: hosts that drive `CircuitInstance` analyses directly (the
    /// Python live session) reuse the same mapping.
    pub fn to_context(&self) -> Context {
        Context {
            tolerances: piperine_solver::prelude::Tolerances {
                temperature: self.temperature,
                reltol: self.reltol,
                abstol: self.abstol,
                gmin: self.gmin,
                ..Default::default()
            },
        }
    }

    /// The convergence tunables (MD-04): set on each analysis solver so
    /// user `max_iter` / `dc_damp_tolerance` reach the Newton loop.
    /// Public for the same host reuse as [`Self::to_context`].
    pub fn to_policy(&self) -> Policy {
        Policy {
            max_iter: self.max_iter,
            dc_damp_tolerance: self.dc_damp_tolerance,
            ..Default::default()
        }
    }
}
