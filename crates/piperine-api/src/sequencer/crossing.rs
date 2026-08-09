//! `wait(above(net, level))` (T24, DVT-26): the analog half of the
//! sequencer's `wait` verb, built on T10's tier-2 crossing prediction
//! (`piperine_ir::events::predict_crossing`) — never the exact retry a
//! tier-3 solver feature would need, and never a bare instant without its
//! bound.

use piperine_solver::abi::AnalogReference;

use piperine_ir::events::{predict_crossing, CrossDir, Prediction};

/// A single outstanding analog crossing wait: which net, which level and
/// direction, and the last accepted `(t, v)` sample — the bracket a fresh
/// pair completes.
pub(super) struct CrossingWait {
    pub net: AnalogReference,
    pub level: f64,
    pub direction: CrossDir,
    pub last: Option<(f64, f64)>,
}

impl CrossingWait {
    pub fn new(net: AnalogReference, level: f64, direction: CrossDir) -> Self {
        Self { net, level, direction, last: None }
    }

    /// Feed one accepted `(t, v)` sample (shifted by `level` happens
    /// internally). Returns the tier-2 [`Prediction`] the instant a
    /// bracketing pair is found — the sequencer resumes at the *current*
    /// accepted `t` (the earliest instant it can act), reporting the
    /// prediction's `error_bound` honestly rather than claiming the
    /// crossing landed exactly there.
    pub fn observe(&mut self, t: f64, v: f64) -> Option<Prediction> {
        let shifted = v - self.level;
        let result = match self.last {
            None => None,
            Some((t0, v0)) => predict_crossing(self.direction, t0, v0, t, shifted),
        };
        self.last = Some((t, shifted));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_solver::abi::{AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    fn node() -> AnalogReference {
        AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(0))), 0)
    }

    #[test]
    fn the_first_sample_only_establishes_a_baseline() {
        let mut w = CrossingWait::new(node(), 1.0, CrossDir::Rising);
        assert!(w.observe(0.0, 0.0).is_none());
    }

    #[test]
    fn a_bracketing_pair_reports_a_prediction_with_an_honest_bound() {
        let mut w = CrossingWait::new(node(), 1.0, CrossDir::Rising);
        w.observe(0.0, 0.5); // v - level = -0.5
        let pred = w.observe(1.0, 1.5).expect("0.5 -> 1.5 brackets level 1.0"); // v - level = 0.5
        assert!((pred.t_cross - 0.5).abs() < 1e-9, "linear interpolation crosses at the midpoint: {pred:?}");
        assert_eq!(pred.error_bound, 1.0, "the bound is the bracket width, never a claimed exactness");
    }

    #[test]
    fn a_non_bracketing_pair_reports_nothing_and_keeps_watching() {
        let mut w = CrossingWait::new(node(), 1.0, CrossDir::Rising);
        w.observe(0.0, 0.5);
        assert!(w.observe(1.0, 0.6).is_none(), "still below the level — no crossing yet");
        let pred = w.observe(2.0, 1.5).expect("now it brackets");
        assert!(pred.t_cross > 1.0 && pred.t_cross < 2.0);
    }
}
