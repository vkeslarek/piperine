//! Periodic steady state (PSS) — options and result types. The shooting
//! driver lives in [`crate::solver::pss`].

use crate::result::TransientAnalysisResult;

/// Single-shooting PSS configuration. The user supplies the drive period
/// `T` (driven circuits only — autonomous period detection is out of
/// scope); `tstab` is an optional pre-roll transient that moves the
/// starting state near the orbit before Newton begins.
#[derive(Debug, Clone)]
pub struct PssAnalysisOptions {
    /// The period `T` the steady state must satisfy (`x(t0+T) = x(t0)`).
    pub period: f64,
    /// Pre-roll length: integrate `[0, tstab]` once and shoot from there.
    pub tstab: f64,
    /// Shooting-Newton iteration cap.
    pub max_shoot_iter: usize,
    /// Convergence bound on `max_i |x_i(T) − x_i(0)|`.
    pub shoot_tol: f64,
    /// Initial integrator dt for each shot; `None` → `period / 100`.
    pub dt: Option<f64>,
}

impl PssAnalysisOptions {
    pub fn new(period: f64) -> Self {
        Self { period, tstab: 0.0, max_shoot_iter: 40, shoot_tol: 1.0e-9, dt: None }
    }

    pub fn with_tstab(mut self, tstab: f64) -> Self {
        self.tstab = tstab;
        self
    }
}

/// Shooting diagnostics: how many Newton iterations the orbit took and the
/// final periodicity residual `max_i |x_i(T) − x_i(0)|`.
#[derive(Debug, Clone, Copy)]
pub struct PssStats {
    pub shoot_iterations: usize,
    pub residual: f64,
}

/// A converged periodic orbit (Debug elided: the trace is bulky): one period of transient samples
/// (`t ∈ [t0, t0+T]`) plus the shooting diagnostics.
pub struct PssResult {
    pub trace: TransientAnalysisResult,
    pub stats: PssStats,
}

impl std::fmt::Debug for PssResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PssResult").field("stats", &self.stats).finish_non_exhaustive()
    }
}
