//! The two element-level checks of the integration scheme's LTE hook: the
//! default `AnalogDevice::suggest_transient_step` returns `None`, and an
//! override is returned unchanged. They build a full `Element`, so they live
//! with the owner of that trait rather than with the numeric engine
//! (moved here from the unit tests of `math/integration.rs`, bodies unchanged).

/// Mock element that returns a fixed timestep, used to verify the
/// stepper loop consults elements for LTE via `AnalogDevice::suggest_transient_step`.
#[test]
fn lte_default_element_returns_none() {
    use piperine_solver::abi::TransientAnalysisState;
    use piperine_solver::abi::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use piperine_solver::abi::CircularArrayBuffer2;
    use piperine_solver::abi::Context;

    struct PlainElement;
    impl AnalogDevice for PlainElement {}
    impl DigitalDevice for PlainElement {}
    impl Introspect for PlainElement {}
    impl Element for PlainElement {
        fn name(&self) -> &str { "plain" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
    }

    let buf = CircularArrayBuffer2::new(1, 2);
    let state = TransientAnalysisState::new(&buf, &[]);
    let ctx = Context::default();

    let el = PlainElement;
    assert!(el.suggest_transient_step(&state, &[1e-6], &ctx).is_none());
}

/// Mini element that always returns a fixed dt of 10 μs, confirming the
/// stepper can consume the suggestion through the trait.
#[test]
fn lte_element_override_returns_custom_dt() {
    use piperine_solver::abi::TransientAnalysisState;
    use piperine_solver::abi::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use piperine_solver::abi::CircularArrayBuffer2;
    use piperine_solver::abi::Context;

    struct FixedLte(f64);
    impl AnalogDevice for FixedLte {
        fn suggest_transient_step(
            &self,
            _state: &TransientAnalysisState<'_>,
            _time_history: &[f64],
            _context: &Context,
        ) -> Option<f64> {
            Some(self.0)
        }
    }
    impl DigitalDevice for FixedLte {}
    impl Introspect for FixedLte {}
    impl Element for FixedLte {
        fn name(&self) -> &str { "fixed_lte" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
    }

    let buf = CircularArrayBuffer2::new(1, 2);
    let state = TransientAnalysisState::new(&buf, &[]);
    let ctx = Context::default();

    let el = FixedLte(10e-6);
    let sug = el.suggest_transient_step(&state, &[1e-6], &ctx);
    assert_eq!(sug, Some(10e-6));
}
