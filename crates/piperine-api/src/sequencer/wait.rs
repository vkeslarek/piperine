//! `wait(edge(...))` (T23, DVT-27): the digital half of the sequencer's one
//! `wait` verb, built on [`HostElement::on_digital_settled`]'s post-settle
//! dispatch (`crate::device::host_element`) — never a mid-delta-cycle read.
//! `wait(above(...))`'s analog twin arrives in T24 (`crate::sequencer::wait`
//! gains `Wake::WaitCrossing` there).

use piperine_solver::abi::{DigitalNet, LogicValue};

/// Which transition a `wait(edge(net, direction))` is watching for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Rising,
    Falling,
    Either,
}

impl EdgeDirection {
    fn matches(self, before: LogicValue, after: LogicValue) -> bool {
        match self {
            EdgeDirection::Rising => before == LogicValue::Zero && after == LogicValue::One,
            EdgeDirection::Falling => before == LogicValue::One && after == LogicValue::Zero,
            EdgeDirection::Either => before != after,
        }
    }
}

/// A single outstanding digital wait: which net, which transition, and the
/// last **settled** value observed (`None` until the first post-settle
/// dispatch after this wait was armed — a wait armed mid-run has no prior
/// settled sample to compare against until the next one arrives).
pub(super) struct EdgeWait {
    pub net: DigitalNet,
    pub direction: EdgeDirection,
    pub baseline: Option<LogicValue>,
}

impl EdgeWait {
    pub fn new(net: DigitalNet, direction: EdgeDirection) -> Self {
        Self { net, direction, baseline: None }
    }

    /// Feed one post-settle observation. Returns `true` exactly when the
    /// watched transition has just been observed (the wait is satisfied);
    /// otherwise updates the baseline for the next comparison.
    pub fn observe(&mut self, nets: &[LogicValue]) -> bool {
        let Some(&current) = nets.get(self.net.0) else { return false };
        match self.baseline {
            None => {
                self.baseline = Some(current);
                false
            }
            Some(before) => {
                if self.direction.matches(before, current) {
                    true
                } else {
                    if before != current {
                        self.baseline = Some(current);
                    }
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_observation_only_establishes_a_baseline_never_matches() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Rising);
        assert!(!w.observe(&[LogicValue::One]), "the first sample can never be a transition — nothing to compare against");
    }

    #[test]
    fn rising_matches_zero_to_one_only() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Rising);
        w.observe(&[LogicValue::Zero]);
        assert!(!w.observe(&[LogicValue::Zero]), "no change is not a rising edge");
        assert!(w.observe(&[LogicValue::One]), "Zero -> One is a rising edge");
    }

    #[test]
    fn rising_ignores_a_falling_transition_and_keeps_watching() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Rising);
        w.observe(&[LogicValue::One]);
        assert!(!w.observe(&[LogicValue::Zero]), "One -> Zero is not a rising edge");
        assert!(w.observe(&[LogicValue::One]), "the later Zero -> One still matches, baseline correctly advanced");
    }

    #[test]
    fn either_matches_any_change_in_either_direction() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Either);
        w.observe(&[LogicValue::Zero]);
        assert!(w.observe(&[LogicValue::One]));
        let mut w2 = EdgeWait::new(DigitalNet(0), EdgeDirection::Either);
        w2.observe(&[LogicValue::One]);
        assert!(w2.observe(&[LogicValue::Zero]));
    }
}
