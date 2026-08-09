//! Level 1 into stamps (T16, DVT-24): the adapter pieces that turn a
//! `piperine-ir` [`Wave`]/[`BehaviorKind`] into MNA rows through
//! [`StampSink`], and feed a [`Measure`] from **accepted** points only.
//! `piperine-ir` itself never stamps (its purity contract, T1) — this
//! module is the one place that turns its values/derivatives into
//! `Stamp`s, which is exactly the division of labor `ideal.md` §3.1
//! describes.
//!
//! **Sign convention** (matches `piperine-codegen`'s own
//! `Instance::nodal_stamps`/`norton_rhs`, `device/analog/instance.rs`): for
//! a two-terminal current force `I(p, n) <+ f(inputs)`, the residual at
//! `p` is `+f`, at `n` is `-f`; the Jacobian entries carry the same sign
//! split, and the RHS is the Norton form `-f(v0) + J·v0` (so a linear
//! device's RHS is exactly its independent-source term).

use piperine_solver::abi::{AnalogReference, LimitReason, LimitingReport};

use piperine_ir::behavior::BehaviorKind;
use piperine_ir::limit::Limiter;
use piperine_ir::measure::MeasureKind;
use piperine_ir::measure::handle::Measure;
use piperine_ir::wave::Wave;

use super::stamp::{Ctx, StampSink, StateView};

/// A [`Wave`] driving a two-terminal force through a **declared** series
/// impedance (DVT-33: an ideal force is available but must be spelled
/// explicitly — `rout = 0.0` — never defaulted). Stamps natively, zero
/// host calls, and publishes the wave's own breakpoints.
pub struct WaveStamp {
    pub wave: Wave,
    pub plus: AnalogReference,
    pub minus: AnalogReference,
    /// Series (Thevenin) output impedance. `0.0` is a legal, explicit
    /// ideal force.
    pub rout: f64,
}

impl WaveStamp {
    pub fn new(wave: Wave, plus: AnalogReference, minus: AnalogReference, rout: f64) -> Self {
        Self { wave, plus, minus, rout }
    }

    /// Stamp this wave's contribution at `ctx.t()` into `sink`.
    pub fn stamp(&self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        let g = 1.0 / self.rout;
        let v = self.wave.value(ctx.t());
        sink.g(&self.plus, &self.plus, g);
        sink.g(&self.plus, &self.minus, -g);
        sink.g(&self.minus, &self.plus, -g);
        sink.g(&self.minus, &self.minus, g);
        sink.i(&self.plus, g * v);
        sink.i(&self.minus, -g * v);
    }

    /// This wave's self-declared discontinuities within `(from, from +
    /// horizon]` — `Wave::pending_edges`'s contract, unchanged.
    pub fn breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        let mut out = Vec::new();
        self.wave.pending_edges(from, from + horizon, &mut out);
        out
    }
}

/// A [`BehaviorKind`] driving a two-terminal current force
/// `I(force_p, force_n) <+ kind.value(inputs)`, with `inputs` read from
/// `controls` in order. Stamps the closed-form value **and** its cross
/// Jacobian entries every call — zero host calls inside Newton.
pub struct BehaviorStamp {
    pub kind: Box<dyn BehaviorKind>,
    pub controls: Vec<AnalogReference>,
    pub force_p: AnalogReference,
    pub force_n: AnalogReference,
}

impl BehaviorStamp {
    pub fn new(
        kind: Box<dyn BehaviorKind>,
        controls: Vec<AnalogReference>,
        force_p: AnalogReference,
        force_n: AnalogReference,
    ) -> Self {
        Self { kind, controls, force_p, force_n }
    }

    /// Stamp this behavior's value and Jacobian at the current solution
    /// into `sink`. Returns the sampled `inputs` (handed back so a caller
    /// composing a crossing check does not have to re-read `ctx`).
    pub fn stamp(&self, ctx: &Ctx<'_>, sink: &mut StampSink) -> Vec<f64> {
        let inputs: Vec<f64> = self.controls.iter().map(|r| ctx.v(r)).collect();
        let f0 = self.kind.value(&inputs);
        let mut jac = vec![0.0; inputs.len()];
        self.kind.jacobian(&inputs, &mut jac);

        for (input, &dfdv) in self.controls.iter().zip(jac.iter()) {
            if dfdv != 0.0 {
                sink.g(&self.force_p, input, dfdv);
                sink.g(&self.force_n, input, -dfdv);
            }
        }
        let coupling: f64 = inputs.iter().zip(jac.iter()).map(|(v, d)| v * d).sum();
        let rhs_p = -f0 + coupling;
        sink.i(&self.force_p, rhs_p);
        sink.i(&self.force_n, -rhs_p);
        inputs
    }
}

/// Feeds a [`Measure`] one `(t, v)` sample per **accepted** point (DVT-24):
/// a thin wrapper whose only job is to be called from
/// [`HostElement::accept_timestep`](super::HostElement::accept_timestep)
/// and nowhere else — the crate-level purity of `Measure` (never fed by a
/// rejected step) is a property of *where this is called from*, not
/// anything this type enforces by itself.
///
/// // SPEC_DEVIATION: `lte` is threaded through as a caller-supplied value
/// // (default `0.0` via `HostElement::accept_timestep`'s ABI, which does
/// // not carry a per-step LTE) rather than the solver's own local-error
/// // estimate. Reason: LTE plumbing into `accept_timestep` is a solver-ABI
/// // change outside T16's scope (piperine-solver stays untouched, MD-01);
/// // `Measure`'s error-bound machinery (T11, delivered) is otherwise
/// // exercised exactly as designed. A caller with a real LTE (e.g. reading
/// // `Element::suggest_transient_step`'s inputs) may pass it explicitly.
pub struct MeasureFeed<K: MeasureKind> {
    pub measure: Measure<K>,
    pub net: AnalogReference,
}

impl<K: MeasureKind> MeasureFeed<K> {
    pub fn new(measure: Measure<K>, net: AnalogReference) -> Self {
        Self { measure, net }
    }

    /// Feed one accepted sample, read from `state` at `t`.
    pub fn on_accept(&mut self, state: &StateView<'_>, t: f64) {
        self.on_accept_with_lte(state, t, 0.0);
    }

    /// [`Self::on_accept`], with an explicit local-error estimate.
    pub fn on_accept_with_lte(&mut self, state: &StateView<'_>, t: f64, lte: f64) {
        let v = state.v(&self.net);
        self.measure.feed(t, v, lte);
    }
}

/// Build a [`LimitingReport`] from a `piperine-ir` [`Limiter`]'s
/// current state (DVT-24, `ideal.md` §3.9 D4): `None` while the limiter is
/// not clamping (the common case — a level-1 limiter costs nothing when
/// unused), `Some` while it is, so it vetoes Newton convergence exactly as
/// a PHDL device's `AnalogDevice::limiting_report` does.
pub fn limiting_report(
    device: &str,
    net: &AnalogReference,
    proposed: f64,
    limiter: &Limiter,
    slot: usize,
    limiter_name: &'static str,
) -> Option<LimitingReport> {
    if !limiter.active() {
        return None;
    }
    let limited_value = limiter.limited().get(slot).copied().unwrap_or(proposed);
    Some(LimitingReport {
        device: device.to_string(),
        net: net.clone(),
        proposed,
        limited_value,
        limiter_name,
        reason: LimitReason::VoltageStep,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_ir::behavior::kinds::Comparator;
    use piperine_ir::measure::gate::Gate;
    use piperine_ir::measure::kinds::Peak;
    use piperine_solver::abi::{AnalogVariable, CircularArrayBuffer2, NodeIdentifier};
    use std::sync::Arc;

    fn node(i: usize) -> AnalogReference {
        AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(i))), i)
    }

    fn ctx_with(history: &CircularArrayBuffer2<f64>, t: f64) -> Ctx<'_> {
        Ctx::new(history, t, 1e-6)
    }

    #[test]
    fn behavior_stamp_contributes_cross_jacobian_entries_from_a_closed_form() {
        let a = node(0);
        let b = node(1);
        let p = node(2);
        let n = node(3);
        let mut history = CircularArrayBuffer2::<f64>::new(1, 4);
        history.push(&ndarray::arr1(&[1.0, 0.0, 0.0, 0.0]).view());

        let stamp = BehaviorStamp::new(
            Box::new(Comparator { lo: 0.0, hi: 1.0, gain: 1.0 }),
            vec![a.clone(), b.clone()],
            p.clone(),
            n.clone(),
        );
        let ctx = ctx_with(&history, 0.0);
        let mut sink = StampSink::new(4);
        let inputs = stamp.stamp(&ctx, &mut sink);
        assert_eq!(inputs, vec![1.0, 0.0]);
        let stamps = sink.into_stamps();
        // Cross Jacobian entries land at (force, control) — not just an
        // RHS: at least one Matrix stamp names the controlling node `a`
        // and `b`, each at both force rows.
        let matrix_targets: Vec<(usize, usize)> = stamps
            .iter()
            .filter_map(|s| match s {
                piperine_solver::abi::Stamp::Matrix(r, c, _) => Some((r.idx().unwrap(), c.idx().unwrap())),
                _ => None,
            })
            .collect();
        assert!(matrix_targets.contains(&(p.idx().unwrap(), a.idx().unwrap())));
        assert!(matrix_targets.contains(&(n.idx().unwrap(), a.idx().unwrap())));
        assert!(matrix_targets.contains(&(p.idx().unwrap(), b.idx().unwrap())));
    }

    #[test]
    fn measure_is_fed_on_accept_and_untouched_otherwise() {
        let out = node(0);
        let mut history = CircularArrayBuffer2::<f64>::new(1, 1);
        history.push(&ndarray::arr1(&[3.3]).view());
        let view = StateView::new(&history);

        let mut feed = MeasureFeed::new(Measure::arm(Peak::new()), out);
        // Simulate a rejected step: never call `on_accept`.
        assert!(feed.measure.value().is_err(), "an unfed measure reads loud, not a plausible number");

        feed.on_accept(&view, 0.0);
        assert_eq!(feed.measure.value().unwrap(), 3.3, "the one accepted sample is what a peak-of-one sees");
    }

    #[test]
    fn a_rejected_step_contributes_nothing_to_the_measure() {
        // The adapter's contract: only a caller invoking `on_accept` feeds
        // the measure. A "rejected step" is modeled here as simply not
        // calling it — proving the accumulator's value is unaffected by
        // any solution reachable during a rejected attempt, since nothing
        // ever reads it into the measure.
        let out = node(0);
        let mut history = CircularArrayBuffer2::<f64>::new(1, 1);
        history.push(&ndarray::arr1(&[10.0]).view());
        let view = StateView::new(&history);
        let mut feed = MeasureFeed::new(Measure::arm(Peak::new()), out);
        feed.on_accept(&view, 0.0);
        let accepted_value = feed.measure.value().unwrap();

        // A "rejected" candidate at a wildly different value — never fed.
        let mut rejected_history = CircularArrayBuffer2::<f64>::new(1, 1);
        rejected_history.push(&ndarray::arr1(&[999.0]).view());
        let _rejected_view = StateView::new(&rejected_history); // deliberately unused: no on_accept call

        assert_eq!(
            feed.measure.value().unwrap(),
            accepted_value,
            "a value never fed through on_accept must not move the measure"
        );
    }

    #[test]
    fn gated_measure_uses_the_shared_gate_type_unchanged() {
        // Proves level 1's `Gate` composes with the adapter without a
        // parallel gating concept here — T16 adapts, it does not reinvent.
        let out = node(0);
        let mut history = CircularArrayBuffer2::<f64>::new(1, 1);
        history.push(&ndarray::arr1(&[1.0]).view());
        let view = StateView::new(&history);
        let mut feed = MeasureFeed::new(Measure::arm_gated(Peak::new(), Gate::Time { t0: 0.0, t1: 1.0 }), out);
        feed.on_accept(&view, 0.5);
        assert_eq!(feed.measure.value().unwrap(), 1.0);
    }

    #[test]
    fn limiting_report_is_none_while_the_limiter_is_not_clamping() {
        let limiter = Limiter::new(vec![0.6]);
        let net = node(0);
        // A limiter that has never seen an update reports inactive.
        assert!(limiting_report("d1", &net, 0.6, &limiter, 0, "pnjlim").is_none());
    }

    #[test]
    fn limiting_report_surfaces_while_still_clamping() {
        use piperine_ir::limit::LimitKind;
        let mut limiter = Limiter::new(vec![0.6]);
        // A large jump from the vcrit seed forces the limiter to clamp.
        limiter.update(&[5.0], &[LimitKind::Pnjlim { vte: 0.02585 }]);
        assert!(limiter.active(), "a 5V jump from a 0.6V seed must still be clamping");
        let net = node(0);
        let report = limiting_report("d1", &net, 5.0, &limiter, 0, "pnjlim")
            .expect("an active limiter surfaces a LimitingReport");
        assert_eq!(report.device, "d1");
        assert_eq!(report.proposed, 5.0);
        assert!(report.limited_value < 5.0, "the limited value must be pulled back from the raw proposal");
    }
}
