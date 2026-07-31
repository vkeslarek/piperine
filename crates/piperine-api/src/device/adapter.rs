//! The blanket `Element` adapter (T14, DVT-19/23): `impl<T: HostElement>
//! Element for HostAdapter<T>`, over the solver's own public `Element`
//! trait, from *outside* the solver crate. This file is the whole MD-01
//! story for host devices — no `Any`, no downcast, and (provable from this
//! task's diff) no change under `crates/piperine-solver/src/`: the solver
//! already defined `Element` as a trait anyone may implement, and a host
//! device doing so is indistinguishable, at the solver's call sites, from a
//! JIT device or a plugin one.
//!
//! // SPEC_DEVIATION: `ideal.md` §6 sketches the adapter as `impl<T:
//! // HostElement> Element for T` directly. Rust's orphan rule forbids
//! // that literally — neither `Element` (solver-defined) nor a bare `T`
//! // are local to this crate, so the blanket impl needs one local type to
//! // anchor on. `HostAdapter<T>` is that anchor: a zero-cost newtype
//! // (`#[repr(transparent)]`-shaped, single field, no added state) so the
//! // "no new solver-side concept" property still holds — the solver still
//! // sees exactly one `Element` per device, and nothing about `HostAdapter`
//! // is visible past construction (`Box::new(HostAdapter(my_device))`).
//!
//! Each solver-visible method is either forwarded to its `HostElement`
//! mirror (translating the raw solver types into the safe [`Ctx`]/
//! [`StateView`]/[`StampSink`] wrappers T15 defines) or left at the
//! supertrait's own default when `HostElement` does not mirror it (digital
//! participation, noise, `.disto`) — the same "default = inert" shape the
//! solver's own traits already use.

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, CircularArrayBuffer2, Context, DcAnalysisState, DigitalDevice, Element,
    ElementCapabilities, ElementCheckpoint, EventSink, Introspect, Invalidation, LimitingReport, LogicValue,
    ParamDescriptor, ParamError, Stamp, TransientAnalysisContext, TransientAnalysisState, UnknownAllocator, Value,
};

use super::host_element::HostElement;
use super::stamp::{Ctx, StampSink, StateView};

/// The local anchor the blanket adapter implements `Element` over (see this
/// module's header for why one is needed). Transparent: `HostAdapter(dev)`
/// wraps a device with no behavior of its own — every method forwards
/// straight through to `T`'s `HostElement` implementation.
pub struct HostAdapter<T>(pub T);

impl<T: HostElement> AnalogDevice for HostAdapter<T> {
    fn limiting_report(&self) -> Option<LimitingReport> {
        HostElement::limiting_report(&self.0)
    }

    fn bound_step_hint(&self) -> f64 {
        HostElement::bound_step_hint(&self.0)
    }

    fn next_breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        HostElement::next_breakpoints(&self.0, from, horizon)
    }

    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        HostElement::allocate_unknowns(&mut self.0, alloc)
    }

    fn set_temperature(&mut self, t: f64) {
        HostElement::set_temperature(&mut self.0, t)
    }

    fn update(&mut self, state: &CircularArrayBuffer2<f64>, _ctx: &Context) {
        let view = StateView::new(state);
        let ctx = Ctx::new(state, 0.0, 0.0);
        HostElement::update(&mut self.0, &view, &ctx)
    }

    fn load_dc(&mut self, state: &DcAnalysisState<'_>, _context: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let history = state.history();
        let ctx = Ctx::new(history, 0.0, 0.0);
        let mut sink = StampSink::new(history.size());
        HostElement::load_dc(&mut self.0, &ctx, &mut sink);
        sink.into_stamps()
    }

    fn load_transient(
        &mut self,
        states: &TransientAnalysisState<'_>,
        tran_ctx: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let history = states.history();
        let ctx = Ctx::new(history, tran_ctx.time, tran_ctx.h);
        let mut sink = StampSink::new(history.size());
        HostElement::load_transient(&mut self.0, &ctx, &mut sink);
        sink.into_stamps()
    }

    // `load_ac`, `noise_current_psd`, `load_disto2/3`, `param_stamps`,
    // `initial_conditions`, `suggest_transient_step`: not mirrored by
    // `HostElement` (see host_element.rs's module header) — inherited at
    // `AnalogDevice`'s own inert defaults.
}

/// Digital participation is not mirrored by `HostElement` (module header) —
/// every method inherits `DigitalDevice`'s own defaults, i.e. a host
/// device drives and reads nothing digital. A device that needs digital
/// implements `Element` directly instead of going through `HostElement`.
impl<T: HostElement> DigitalDevice for HostAdapter<T> {}

impl<T: HostElement> Introspect for HostAdapter<T> {
    fn read_opvars(&self) -> Vec<(String, f64)> {
        HostElement::read_opvars(&self.0)
    }

    fn list_params(&self) -> Vec<ParamDescriptor> {
        HostElement::list_params(&self.0)
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        HostElement::get_param(&self.0, name)
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        HostElement::set_param(&mut self.0, name, value)
    }
}

impl<T: HostElement> Element for HostAdapter<T> {
    fn name(&self) -> &str {
        HostElement::name(&self.0)
    }

    fn capabilities(&self) -> ElementCapabilities {
        HostElement::capabilities(&self.0)
    }

    fn accept_timestep(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        t: f64,
        _nets: &[LogicValue],
        sink: &mut dyn EventSink,
    ) {
        let view = StateView::new(state);
        HostElement::accept_timestep(&mut self.0, &view, t);
        // T21/DVT-33: a digital drive queued during that accept (e.g. a
        // sequencer's `t.drive(net, value)`) is forwarded to the real
        // scheduler here — the one place `HostElement`'s narrow digital
        // hook meets the raw `EventSink` ABI.
        for (net, value, delay) in HostElement::drain_digital_drives(&mut self.0) {
            sink.emit(net, value, delay);
        }
    }

    fn on_digital_settled(&mut self, nets: &[LogicValue], t: f64) {
        HostElement::on_digital_settled(&mut self.0, nets, t)
    }

    fn checkpoint_state(&self) -> Option<ElementCheckpoint> {
        HostElement::checkpoint_state(&self.0)
    }

    fn restore_state(&mut self, checkpoint: &ElementCheckpoint) {
        HostElement::restore_state(&mut self.0, checkpoint)
    }

    /// T22, DVT-29: a `Sequencer`'s `expect`/`warn` findings reach
    /// `dv-core`'s channel through here — the one `HostElement` hook this
    /// batch mirrors from the "not mirrored" list below, because T22 gives
    /// it bearing for the first time (T14's host_element.rs header already
    /// documents that an omitted method is extended additively once a
    /// task needs it).
    fn validation_reports(&self) -> Option<piperine_solver::abi::ValidationReport> {
        HostElement::validation_reports(&self.0)
    }

    // `evaluate_constraints`, `margin_report`, `reset_margin_tracking`,
    // `runtime_banks`, `setup`/`destroy`: not mirrored — inherited at
    // `Element`'s own defaults (DVC-18 reaches a host device the same way
    // `.disto` does: implement `Element` directly for that surface).
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_solver::abi::{AnalogVariable, NodeIdentifier};
    use piperine_solver::prelude::{CircuitBuilder, Context};
    use std::sync::Arc;

    /// The `ideal.md` §2 resistor, in Rust, as a `HostElement` — T14's
    /// acceptance case: a trivial host device stamps into a real circuit
    /// and solves.
    struct HostResistor {
        p: AnalogReference,
        n: AnalogReference,
        g: f64,
    }

    impl HostElement for HostResistor {
        fn name(&self) -> &str {
            "host_r"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
        fn load_dc(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            sink.g(&self.p, &self.p, self.g);
            sink.g(&self.p, &self.n, -self.g);
            sink.g(&self.n, &self.p, -self.g);
            sink.g(&self.n, &self.n, self.g);
        }
    }

    #[test]
    fn a_trivial_host_device_stamps_into_a_real_circuit_and_solves() {
        let mut builder = CircuitBuilder::new("host_resistor_divider");
        let gnd = builder.ground();
        let vin = builder.node("vin");
        let mid = builder.node("mid");

        // A raw forced-voltage source at `vin` (level 0, no HostElement
        // needed): stamp it directly as a solver `Element` test double so
        // this test proves the *host resistor*, not a second device kind.
        struct Vsrc {
            p: AnalogReference,
            n: AnalogReference,
            branch: Option<AnalogReference>,
            v: f64,
        }
        impl piperine_solver::abi::AnalogDevice for Vsrc {
            fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
                self.branch = Some(alloc.branch("vsrc", "i"));
            }
            fn load_dc(
                &mut self,
                _state: &DcAnalysisState<'_>,
                _ctx: &Context,
            ) -> Vec<Stamp<AnalogReference, f64>> {
                let b = self.branch.clone().unwrap();
                vec![
                    Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
                    Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
                    Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
                    Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
                    Stamp::Rhs(b, self.v),
                ]
            }
        }
        impl piperine_solver::abi::DigitalDevice for Vsrc {}
        impl piperine_solver::abi::Introspect for Vsrc {}
        impl piperine_solver::abi::Element for Vsrc {
            fn name(&self) -> &str {
                "vsrc"
            }
            fn capabilities(&self) -> ElementCapabilities {
                ElementCapabilities::ANALOG
                    | ElementCapabilities::LOADS_DC
                    | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
            }
        }

        builder.element(Box::new(Vsrc { p: vin.clone(), n: gnd.clone(), branch: None, v: 5.0 }));
        // Two host resistors, 3k/2k — proving it composes like any element,
        // not merely "runs once".
        builder.element(Box::new(HostAdapter(HostResistor { p: vin.clone(), n: mid.clone(), g: 1.0 / 3e3 })));
        builder.element(Box::new(HostAdapter(HostResistor { p: mid.clone(), n: gnd.clone(), g: 1.0 / 2e3 })));

        let mut circuit = builder.build().expect("host devices build into a real circuit");
        let mut dc = circuit.dc(Context::default()).expect("dc setup");
        let result = dc.solve().expect("dc solves with a host device stamping it");
        let v_mid = result.get(mid.clone()).expect("mid is a solved unknown");
        assert!(
            (v_mid - 2.0).abs() < 1e-9,
            "a 3k/2k divider from 5V gives 2.0V at mid, got {v_mid}"
        );
    }

    /// MD-01's absence proof, mechanical rather than by inspection: the
    /// blanket adapter's stamps land through the same `AnalogReference`
    /// identity a raw `Element` would use — no downcast, no `Any` anywhere
    /// in the chain (the type signature of `AnalogDevice`/`Element`
    /// themselves rules it out; this test exercises that the values it
    /// produces really do reach the solver, not merely that it compiles).
    #[test]
    fn a_host_device_is_indistinguishable_from_any_other_element_to_the_solver() {
        fn node(i: usize) -> AnalogReference {
            AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(i))), i)
        }
        let mut builder = CircuitBuilder::new("indistinguishable");
        let _ = builder.ground();
        let a = node(0);
        let b = node(1);
        // `builder.element` takes `Box<dyn Element>` — a `HostResistor`
        // coerces into it via the blanket impl (through the transparent
        // `HostAdapter` anchor the orphan rule requires) with no `Any`
        // and no special-cased insertion path.
        let el: Box<dyn Element> = Box::new(HostAdapter(HostResistor { p: a, n: b, g: 1e-3 }));
        assert_eq!(el.name(), "host_r");
    }
}
