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
    /// `idt`'s `ic`, snapshotted at construction — only consulted by
    /// [`Self::accept`] to force an `Operator::Integrate` slot back to its
    /// initial condition while `idt(x, ic, assert)`'s `assert` argument
    /// (§4.5.4, LRMW2-29) is nonzero. `piperine_ir::ops::Operator` forgets
    /// `ic` after its first `accept` uses it as the accumulator seed, so
    /// codegen keeps its own copy for the reset. `None` for every operator
    /// kind, and for an `idt`/`idtmod` with no `assert` argument (the
    /// caller then never passes a nonzero `assert` to [`Self::accept`]
    /// anyway, so the field is simply never consulted).
    assert_ic: Option<f64>,
}

impl SlottedOperator {
    pub(super) fn new(slot: usize, op: Operator) -> Self {
        SlottedOperator { slot, op, assert_ic: None }
    }

    /// An `Integrate` operator that additionally remembers its `ic` for
    /// `assert`-driven resets (§4.5.4). Safe to use for every `idt`/
    /// `idtmod` slot regardless of whether it actually named an `assert`
    /// argument — a slot with none simply never sees a nonzero `assert`
    /// value passed to [`Self::accept`].
    pub(super) fn with_assert_ic(slot: usize, op: Operator, ic: f64) -> Self {
        SlottedOperator { slot, op, assert_ic: Some(ic) }
    }

    pub(super) fn slot(&self) -> usize {
        self.slot
    }

    /// Advance to `time` with the operator input `input`; returns the new
    /// output value. Call only on accepted timesteps.
    ///
    /// `assert` is `idt(x, ic, assert)`'s own runtime input (§4.5.4): while
    /// nonzero, an `Integrate` slot with a known `ic` (see
    /// [`Self::with_assert_ic`]) is forced back to `ic` instead of
    /// integrating `input` — every other operator kind, and an `Integrate`
    /// slot with no `assert_ic`, ignores `assert` entirely.
    pub(super) fn accept(&mut self, time: f64, input: f64, assert: f64) -> f64 {
        if assert != 0.0
            && let (Operator::Integrate { value, time: t, .. }, Some(ic)) = (&mut self.op, self.assert_ic)
        {
            *value = ic;
            *t = time;
            return ic;
        }
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
        assert_eq!(op.accept(1.0, 5.0, 0.0), 1.0);
    }

    #[test]
    fn assert_forces_an_integrator_back_to_its_ic() {
        // §4.5.4/LRMW2-29: while `assert` is nonzero, the slot holds at its
        // `ic` (2.0) instead of integrating `input` (which alone would
        // drive it to 2.0 + 1.0*3.0 = 5.0).
        let mut op = SlottedOperator::with_assert_ic(0, Operator::integrate(2.0, None), 2.0);
        assert_eq!(op.accept(1.0, 3.0, 1.0), 2.0, "asserted: holds at ic, ignores input");
        assert_eq!(op.accept(2.0, 3.0, 0.0), 5.0, "de-asserted: integrates 3.0 over dt=1 from ic=2.0");
    }

    #[test]
    fn pending_edges_delegates_to_the_wrapped_signal_operator() {
        // A seeded transition with a pending ramp reports the ramp-start and
        // ramp-end edges — the same breakpoints
        // `piperine_ir::ops::Operator::pending_edges` computes directly
        // (mirrors piperine-ir's `transition_pending_edges_while_ramping`).
        let mut op = SlottedOperator::new(0, Operator::transition(0.0, 2.0, 2.0));
        op.accept(0.0, 0.0, 0.0);
        op.accept(1.0, 1.0, 0.0); // change at t=1, ramp over [1, 3]
        let mut out = Vec::new();
        op.pending_edges(0.5, 10.0, &mut out);
        assert_eq!(out, vec![1.0, 3.0], "ramp start + end");
    }
}
