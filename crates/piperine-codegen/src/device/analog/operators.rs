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
    /// `idtmod`'s `offset` (§4.5.5, LRMW2-30): added to every value
    /// [`Self::accept`] and [`Self::initial_output`] report, shifting the
    /// wrap window from `[0, modulus)` to `[offset, offset+modulus)`.
    /// `piperine_ir::ops::Operator::Integrate` itself always wraps into
    /// `[0, modulus)` internally — the shift is applied only at the
    /// reported output, never to the accumulator, so it does not change
    /// the integration dynamics. `0.0` (a no-op shift) for every operator
    /// kind but an `idtmod` with an `offset` argument.
    offset: f64,
}

impl SlottedOperator {
    pub(super) fn new(slot: usize, op: Operator) -> Self {
        SlottedOperator { slot, op, assert_ic: None, offset: 0.0 }
    }

    /// An `Integrate` operator (`idt`/`idtmod`) that additionally
    /// remembers its `ic` for `assert`-driven resets (§4.5.4) and its
    /// `offset` for the output wrap shift (§4.5.5). Safe to use for every
    /// `idt`/`idtmod` slot regardless of whether it actually named those
    /// arguments — `assert`/`offset` are simply never nonzero/nonzero-effect
    /// for a slot that didn't.
    pub(super) fn integrator(slot: usize, op: Operator, ic: f64, offset: f64) -> Self {
        SlottedOperator { slot, op, assert_ic: Some(ic), offset }
    }

    pub(super) fn slot(&self) -> usize {
        self.slot
    }

    /// The value this slot's state bank should be seeded with before any
    /// `accept` call (the DC/IC operating point) — `ic + offset` for an
    /// `Integrate` operator (§4.5.5's window shift applies from the very
    /// first reported value, not only after the first `accept`), `None`
    /// for every other operator kind (its slot starts at the bank's
    /// ordinary `0.0` default).
    pub(super) fn initial_output(&self) -> Option<f64> {
        match &self.op {
            Operator::Integrate { value, .. } => Some(value + self.offset),
            _ => None,
        }
    }

    /// Advance to `time` with the operator input `input`; returns the new
    /// output value. Call only on accepted timesteps.
    ///
    /// `assert` is `idt(x, ic, assert)`'s own runtime input (§4.5.4): while
    /// nonzero, an `Integrate` slot with a known `ic` (see
    /// [`Self::integrator`]) is forced back to `ic` instead of integrating
    /// `input` — every other operator kind ignores `assert` entirely.
    /// `offset` (§4.5.5) is added to whatever value is reported either way.
    pub(super) fn accept(&mut self, time: f64, input: f64, assert: f64) -> f64 {
        if assert != 0.0
            && let (Operator::Integrate { value, time: t, .. }, Some(ic)) = (&mut self.op, self.assert_ic)
        {
            *value = ic;
            *t = time;
            return ic + self.offset;
        }
        self.op.accept(time, input) + self.offset
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
    fn offset_shifts_the_wrap_window_without_changing_the_dynamics() {
        // §4.5.5/LRMW2-30: idtmod(x, ic=0, modulus=10, offset) wraps into
        // [offset, offset+modulus). Two offsets discriminate: the same
        // accumulated integral (7.0, no wrap yet) reports differently.
        let mut a = SlottedOperator::integrator(0, Operator::integrate(0.0, Some(10.0)), 0.0, 0.0);
        let mut b = SlottedOperator::integrator(0, Operator::integrate(0.0, Some(10.0)), 0.0, 5.0);
        assert_eq!(a.accept(1.0, 7.0, 0.0), 7.0, "offset=0: unshifted");
        assert_eq!(b.accept(1.0, 7.0, 0.0), 12.0, "offset=5: shifted by +5");
    }

    #[test]
    fn offset_seeds_the_initial_output_too() {
        // §4.5.5: the shift applies from the very first reported value
        // (the DC/IC seed), not only after the first accept.
        let op = SlottedOperator::integrator(0, Operator::integrate(3.0, None), 3.0, 2.0);
        assert_eq!(op.initial_output(), Some(5.0), "ic=3.0 + offset=2.0");
    }

    #[test]
    fn assert_forces_an_integrator_back_to_its_ic() {
        // §4.5.4/LRMW2-29: while `assert` is nonzero, the slot holds at its
        // `ic` (2.0) instead of integrating `input` (which alone would
        // drive it to 2.0 + 1.0*3.0 = 5.0).
        let mut op = SlottedOperator::integrator(0, Operator::integrate(2.0, None), 2.0, 0.0);
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
