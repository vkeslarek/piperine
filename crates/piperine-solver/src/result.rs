use crate::error::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Per-analysis convergence + performance diagnostics. Accumulated during
/// the solve; returned on the result type. Always-on (counter increments
/// are negligible); `Default::default()` zeroes everything for analyses
/// that haven't been instrumented yet.
#[derive(Debug, Clone, Default)]
pub struct SolverStats {
    // Newton (DC + each transient step's inner loop)
    pub newton_iterations: usize,
    pub converged: bool,
    // Transient step loop
    pub steps_accepted: usize,
    pub steps_rejected: usize,
    pub dt_min_floor_hits: usize,
    pub dt_min: f64,
    pub dt_max: f64,
    // Device-level
    pub bypass_hits: usize,
    pub bypass_misses: usize,
    // Homotopy / convergence strategy
    pub homotopy_strategy: Option<String>,
    pub homotopy_levels: usize,
    // Timing (nanoseconds)
    pub assembly_time_ns: u64,
    pub solve_time_ns: u64,
}
