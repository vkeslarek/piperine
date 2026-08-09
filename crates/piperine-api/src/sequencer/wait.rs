//! `wait(edge(...))` (T23, DVT-27): the digital half of the sequencer's one
//! `wait` verb, built on
//! [`HostElement::on_digital_settled`](crate::device::HostElement::on_digital_settled)'s
//! post-settle dispatch — never a mid-delta-cycle read. `wait(above(...))`'s
//! analog twin is `crossing::CrossingWait` (T24).

use piperine_solver::abi::{DigitalNet, LogicValue};

/// Which transition a `wait(edge(net, direction))` is watching for — the
/// shared IEEE 1364 detector (`piperine_ir::logic`), re-exported under
/// this crate's existing name/path (`pub use wait::EdgeDirection` in
/// `sequencer/mod.rs`) so Python bindings keep compiling unchanged.
pub use piperine_ir::logic::EdgeDirection;

/// `LogicValue` (`piperine-solver`) → `signal::logic::Logic`: both four-state
/// types share discriminants, but the conversion stays an explicit `match`,
/// never a transmute.
fn to_signal(value: LogicValue) -> piperine_ir::logic::Logic {
    match value {
        LogicValue::Zero => piperine_ir::logic::Logic::Zero,
        LogicValue::One => piperine_ir::logic::Logic::One,
        LogicValue::X => piperine_ir::logic::Logic::X,
        LogicValue::Z => piperine_ir::logic::Logic::Z,
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
                if self.direction.fires(to_signal(before), to_signal(current)) {
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
    fn rising_fires_on_ieee_1364_transitions_into_one_not_only_zero_to_one() {
        // Rewritten (not deleted, AC 32/CGS-14): the previous test name
        // ("...zero_to_one_only") named the narrow rule that was the bug
        // this feature fixes. Zero -> One still fires; no-change still does
        // not — the rewrite is the rule, not the zero/one case itself.
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Rising);
        w.observe(&[LogicValue::Zero]);
        assert!(!w.observe(&[LogicValue::Zero]), "no change is not a rising edge");
        assert!(w.observe(&[LogicValue::One]), "Zero -> One is a rising edge");
    }

    #[test]
    fn rising_fires_on_x_to_one() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Rising);
        w.observe(&[LogicValue::X]);
        assert!(w.observe(&[LogicValue::One]), "X -> One satisfies a Rising wait (AC 32)");
    }

    #[test]
    fn rising_fires_on_z_to_one() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Rising);
        w.observe(&[LogicValue::Z]);
        assert!(w.observe(&[LogicValue::One]), "Z -> One satisfies a Rising wait");
    }

    #[test]
    fn falling_fires_on_x_to_zero() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Falling);
        w.observe(&[LogicValue::X]);
        assert!(w.observe(&[LogicValue::Zero]), "X -> Zero satisfies a Falling wait");
    }

    #[test]
    fn falling_fires_on_z_to_zero() {
        let mut w = EdgeWait::new(DigitalNet(0), EdgeDirection::Falling);
        w.observe(&[LogicValue::Z]);
        assert!(w.observe(&[LogicValue::Zero]), "Z -> Zero satisfies a Falling wait");
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
