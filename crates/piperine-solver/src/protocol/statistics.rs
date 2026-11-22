//! How a transient's accepted steps were spent: the protocol's own count of what ended each step
//! (`StepEnd`), the instants it ran and the steps it retook toward a crossing. The analog side
//! keeps its own counts (`SolverStats`); this module counts what only the protocol knows.

use std::collections::HashMap;

use super::sides::LandingCause;

/// How one accepted step ended: exactly one per step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepEnd {
    /// The step ended on a landing, for this cause.
    Landing(LandingCause),
    /// The step was retaken to a located crossing and ended there.
    Crossing,
    /// The analog side's step was bounded by its `max_step`.
    MaxStep,
    /// The analog side's step was bounded by a `bound_step` cap.
    Cap,
    /// The step ended on the stop time, or on the instant that finishes the analysis.
    Stop,
    /// The step ended where the step controller proposed.
    Proposal,
}

impl StepEnd {
    /// Every end a step can have.
    pub const ALL: [StepEnd; 10] = [
        StepEnd::Landing(LandingCause::AnalogReadChanged),
        StepEnd::Landing(LandingCause::ContinuousRead),
        StepEnd::Landing(LandingCause::DeclaredReach),
        StepEnd::Landing(LandingCause::Irreversible),
        StepEnd::Landing(LandingCause::HostWaiter),
        StepEnd::Crossing,
        StepEnd::MaxStep,
        StepEnd::Cap,
        StepEnd::Stop,
        StepEnd::Proposal,
    ];
}

/// Why the protocol ran a discrete instant at an accepted step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstantCause {
    /// A crossing fired at the step.
    Crossing,
    /// No crossing fired, and the discrete side held something at the step's time: a commit
    /// scheduled there, a suspended instant, a dirty function or a reaction on `settle`.
    Scheduled,
}

/// The protocol's count of a transient's steps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepStatistics {
    ends: HashMap<StepEnd, u64>,
    instants: HashMap<InstantCause, u64>,
    retaken: u64,
}

impl StepStatistics {
    /// Counts one accepted step under `end`.
    pub fn record_end(&mut self, end: StepEnd) {
        *self.ends.entry(end).or_insert(0) += 1;
    }

    /// Counts one discrete instant run by the protocol at an accepted step, for `cause`.
    pub fn record_instant(&mut self, cause: InstantCause) {
        *self.instants.entry(cause).or_insert(0) += 1;
    }

    /// Counts one step retaken toward a located crossing.
    pub fn record_retake(&mut self) {
        self.retaken += 1;
    }

    /// The accepted steps: each is counted under exactly one end.
    pub fn accepted(&self) -> u64 {
        self.ends.values().sum()
    }

    /// The accepted steps that ended as `end`.
    pub fn ended(&self, end: StepEnd) -> u64 {
        self.ends.get(&end).copied().unwrap_or(0)
    }

    /// The accepted steps that ended on a landing for `cause`.
    pub fn landings(&self, cause: LandingCause) -> u64 {
        self.ended(StepEnd::Landing(cause))
    }

    /// The instants the protocol ran at accepted steps.
    pub fn instants_run(&self) -> u64 {
        self.instants.values().sum()
    }

    /// The instants the protocol ran at accepted steps for `cause`.
    pub fn instants_for(&self, cause: InstantCause) -> u64 {
        self.instants.get(&cause).copied().unwrap_or(0)
    }

    /// The steps retaken toward a located crossing.
    pub fn retaken(&self) -> u64 {
        self.retaken
    }

    /// The share of accepted steps that ended on a landing only because a reaction reads a
    /// continuous pin, 0.0 when no step was accepted.
    pub fn sampling_share(&self) -> f64 {
        match self.accepted() {
            0 => 0.0,
            accepted => self.landings(LandingCause::ContinuousRead) as f64 / accepted as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> StepStatistics {
        let mut statistics = StepStatistics::default();
        for (index, end) in StepEnd::ALL.iter().enumerate() {
            for _ in 0..=index {
                statistics.record_end(*end);
            }
        }
        statistics
    }

    #[test]
    fn the_ends_sum_to_the_accepted_steps() {
        let statistics = filled();
        let summed: u64 = StepEnd::ALL.iter().map(|end| statistics.ended(*end)).sum();
        assert_eq!(statistics.accepted(), summed);
        assert_eq!(statistics.accepted(), 55, "1 + 2 + ... + 10");
        assert_eq!(statistics.ended(StepEnd::Cap), 8);
    }

    #[test]
    fn landings_read_the_landing_ends() {
        let statistics = filled();
        assert_eq!(statistics.landings(LandingCause::AnalogReadChanged), 1);
        assert_eq!(statistics.landings(LandingCause::HostWaiter), 5);
    }

    #[test]
    fn the_sampling_share_is_continuous_read_landings_over_accepted_steps() {
        assert_eq!(filled().sampling_share(), 2.0 / 55.0);
        assert_eq!(StepStatistics::default().sampling_share(), 0.0);
    }

    #[test]
    fn instants_and_retakes_are_counted_apart_from_ends() {
        let mut statistics = StepStatistics::default();
        statistics.record_instant(InstantCause::Scheduled);
        statistics.record_instant(InstantCause::Crossing);
        statistics.record_retake();
        assert_eq!((statistics.instants_run(), statistics.retaken(), statistics.accepted()), (2, 1, 0));
        assert_eq!((statistics.instants_for(InstantCause::Scheduled), statistics.instants_for(InstantCause::Crossing)), (1, 1));
    }
}
