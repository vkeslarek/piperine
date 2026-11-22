//! How a transient's steps were spent, as a host reads it: the protocol's count of what ended
//! each accepted step and the analog side's count of its rejections and jump steps. No run key
//! and no flag names it; a session returns it after its transient (`Session::statistics`).

use piperine_solver::prelude::SolverStats;
use piperine_solver::protocol::StepStatistics;

/// The counts of one transient run.
#[derive(Debug, Clone)]
pub struct TransientStatistics {
    steps: StepStatistics,
    analog: SolverStats,
}

impl TransientStatistics {
    /// The counts of a transient that ended with the protocol's `steps` and the analog side's
    /// `analog`.
    pub fn new(steps: StepStatistics, analog: SolverStats) -> TransientStatistics {
        TransientStatistics { steps, analog }
    }

    /// The protocol's count: accepted steps by end, instants run, steps retaken toward a crossing.
    pub fn steps(&self) -> &StepStatistics {
        &self.steps
    }

    /// The analog side's count: rejections by cause, jump steps, steps at `max_step`.
    pub fn analog(&self) -> &SolverStats {
        &self.analog
    }
}
