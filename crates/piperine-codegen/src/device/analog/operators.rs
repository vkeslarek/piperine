//! Operators capability: `delay`/`slew`/`transition`/`idt`/`idtmod`
//! runtime-serviced state, updated once per accepted timestep. The pure
//! semantics live in `piperine_ir::ops::Operator` — this file only adds
//! the state-array slot the signal type deliberately does not carry.

use piperine_ir::ops::Operator;

/// A runtime-serviced analog operator paired with the state-array slot it
/// writes: `piperine_ir::ops::Operator` has no notion of a slot, so
/// codegen carries it alongside.
pub(super) struct SlottedOperator {
    pub(super) slot: usize,
    pub(super) op: Operator,
}

impl SlottedOperator {
    pub(super) fn new(slot: usize, op: Operator) -> Self {
        SlottedOperator { slot, op }
    }

    pub(super) fn slot(&self) -> usize {
        self.slot
    }

    /// Advance to `time` with the operator input `input`; returns the new
    /// output value. Call only on accepted timesteps.
    pub(super) fn accept(&mut self, time: f64, input: f64) -> f64 {
        self.op.accept(time, input)
    }

    /// Ramp edges strictly inside `(from, end]` for breakpoint declaration —
    /// see `piperine_ir::ops::Operator::pending_edges`.
    pub(super) fn pending_edges(&self, from: f64, end: f64, out: &mut Vec<f64>) {
        self.op.pending_edges(from, end, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The pure `delay`/`slew`/`transition`/`idt` semantics are
    // `piperine_ir::ops::Operator`'s own tests (crates/piperine-ir/
    // src/ops.rs); these cover only what this file adds — the slot the
    // signal type deliberately does not carry, and that `accept`/
    // `pending_edges` delegate rather than reimplement (CGS-01, CGS-02).

    #[test]
    fn slottedoperator_delay_carries_its_slot() {
        let op = SlottedOperator::new(7, Operator::delay(1.0));
        assert_eq!(op.slot(), 7);
    }

    #[test]
    fn slottedoperator_slew_carries_its_slot() {
        let op = SlottedOperator::new(3, Operator::slew(1.0, 1.0));
        assert_eq!(op.slot(), 3);
    }

    #[test]
    fn slottedoperator_transition_carries_its_slot() {
        let op = SlottedOperator::new(2, Operator::transition(0.0, 1.0, 1.0));
        assert_eq!(op.slot(), 2);
    }

    #[test]
    fn slottedoperator_integrate_carries_its_slot() {
        let op = SlottedOperator::new(5, Operator::integrate(0.0, None));
        assert_eq!(op.slot(), 5);
    }

    #[test]
    fn accept_delegates_to_the_wrapped_signal_operator() {
        // Slew limited to rise*dt = 1.0*1.0 = 1.0 for an input asking for 5.0 —
        // the same result `Operator::slew(1.0, 2.0).accept(1.0, 5.0)` returns
        // (piperine-ir's `slew_limits_rise_and_fall_asymmetrically`),
        // proving the wrapper does not reimplement the limiter math.
        let mut op = SlottedOperator::new(0, Operator::slew(1.0, 2.0));
        assert_eq!(op.accept(1.0, 5.0), 1.0);
    }

    #[test]
    fn pending_edges_delegates_to_the_wrapped_signal_operator() {
        // A seeded transition with a pending ramp reports the ramp-start and
        // ramp-end edges — the same breakpoints
        // `piperine_ir::ops::Operator::pending_edges` computes directly
        // (mirrors piperine-ir's `transition_pending_edges_while_ramping`).
        let mut op = SlottedOperator::new(0, Operator::transition(0.0, 2.0, 2.0));
        op.accept(0.0, 0.0);
        op.accept(1.0, 1.0); // change at t=1, ramp over [1, 3]
        let mut out = Vec::new();
        op.pending_edges(0.5, 10.0, &mut out);
        assert_eq!(out, vec![1.0, 3.0], "ramp start + end");
    }
}
