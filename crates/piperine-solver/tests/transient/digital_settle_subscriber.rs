//! T23 (DVT-27) on the `Device` trait — the settle subscriber: a reaction on
//! the `SETTLE` global runs once per time point, strictly after the instant
//! reached quiescence, so it reads the settled value of a net and never a
//! value from before the instant:
//! - a fixture whose settled value differs from the one before the instant
//!   (`c = a OR NOT(NOT(a))`): the subscriber sees the settled one;
//! - a classic AND-gate glitch fixture (`c = a AND NOT(a)`, the inverter one
//!   microstep late) whose settled value is the same before and after: `c`
//!   really reads `HIGH` for an intermediate microstep (the sink's changes
//!   show it), and the subscriber sees only the settled value.
//!
//! The two structural claims of the original (`EventKind` keeps its four
//! variants; `on_digital_settled` has one call site) were about the removed
//! per-hook contract and its event queue; they are listed in `removed.txt`.

use piperine_device_fixtures::mixed;
use piperine_device_fixtures::solver_devices::digital_settle_subscriber::{InitialLowSetter, LagInverter, SETTLED, Subscriber};
use piperine_device_fixtures::digital;
use std::cell::RefCell;

use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{Observation, ObservationKind, Sink};

const FLIP_AT: f64 = 1.0e-7;

/// A sink that keeps what it sees.
struct Collect(std::rc::Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

#[test]
fn the_subscriber_sees_the_post_settle_value_even_when_it_differs_from_the_pre_accept_snapshot() {
    // c = a OR NOT(NOT(a)): before the flip c = LOW, after the flip and the
    // two inverter stages' settle c = HIGH.
    let mut b = CircuitBuilder::new("before_after");
    DigitalSettleSubscriberCases::start(&mut b);
    b.device(digital::Inverter::module(), "inv1", &[], &[DigitalSettleSubscriberCases::net("a"), DigitalSettleSubscriberCases::net("b1")]);
    b.device(digital::Inverter::module(), "inv2", &[], &[DigitalSettleSubscriberCases::net("b1"), DigitalSettleSubscriberCases::net("b2")]);
    b.device(digital::Or2::module(), "or", &[], &[DigitalSettleSubscriberCases::net("a"), DigitalSettleSubscriberCases::net("b2"), DigitalSettleSubscriberCases::net("c")]);
    DigitalSettleSubscriberCases::subscribe(&mut b);
    DigitalSettleSubscriberCases::run(b.build().expect("build"));

    assert!(DigitalSettleSubscriberCases::settled_before(FLIP_AT).iter().all(|c| *c == LOW), "c starts LOW (a = LOW OR b2 = LOW) before the flip");
    let settled = DigitalSettleSubscriberCases::settled_at(FLIP_AT);
    assert_eq!(settled.len(), 1, "SETTLE fires exactly once for this one settle: {settled:?}");
    assert_eq!(
        settled[0], HIGH,
        "the subscriber must see c's SETTLED value (HIGH, once a = HIGH propagates through both inverters), \
         not the value before the instant (LOW)"
    );
}

#[test]
fn the_subscriber_sees_only_the_settled_value_never_the_mid_delta_cycle_glitch() {
    // c = a AND NOT(a): always LOW once settled, though it reads HIGH for one
    // microstep, while the late inverter still holds NOT(a) from before the flip.
    let mut b = CircuitBuilder::new("glitch");
    DigitalSettleSubscriberCases::start(&mut b);
    b.device(LagInverter::module(), "inv", &[], &[DigitalSettleSubscriberCases::net("a"), DigitalSettleSubscriberCases::net("nb")]);
    b.device(digital::And2::module(), "and", &[], &[DigitalSettleSubscriberCases::net("a"), DigitalSettleSubscriberCases::net("nb"), DigitalSettleSubscriberCases::net("c")]);
    DigitalSettleSubscriberCases::subscribe(&mut b);
    let c_during = DigitalSettleSubscriberCases::run(b.build().expect("build"));

    assert!(
        c_during.contains(&HIGH.to_vec()),
        "the fixture must actually glitch (c reads HIGH at some intermediate microstep of the flip): {c_during:?}"
    );
    let settled = DigitalSettleSubscriberCases::settled_at(FLIP_AT);
    assert_eq!(settled.len(), 1, "SETTLE fires exactly once for this one settle: {settled:?}");
    assert_eq!(settled[0], LOW, "the subscriber must see c's correct SETTLED value (LOW), never a transient glitch (HIGH)");
}

/// The helpers of the digital_settle_subscriber tests.
pub struct DigitalSettleSubscriberCases;

impl DigitalSettleSubscriberCases {

    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The start helper `start`.
    fn start(b: &mut CircuitBuilder) {
        b.device(mixed::Delay::module(), "kick", &[("at", ParamValue::Real(FLIP_AT))], &[DigitalSettleSubscriberCases::net("go")]);
        b.device(InitialLowSetter::module(), "driver", &[], &[DigitalSettleSubscriberCases::net("a"), DigitalSettleSubscriberCases::net("go")]);
    }

    /// The subscribe helper `subscribe`.
    fn subscribe(b: &mut CircuitBuilder) {
        b.device(Subscriber::module(), "subscriber", &[], &[DigitalSettleSubscriberCases::net("c")]);
    }

    /// A transient that stops at the flip: the instant of the flip is the last one,
    /// and the `SETTLE` of the stop time follows its quiescence. Returns every
    /// value `c` took during the flip's instant, in order (the sink's changes).
    fn run(circuit: CircuitInstance) -> Vec<Vec<u8>> {
        SETTLED.with(|s| s.borrow_mut().clear());
        let seen = std::rc::Rc::new(RefCell::new(Vec::new()));
        let mut solver = Solver::new(circuit)
            .with_tran_opts(TransientAnalysisOptions::new(FLIP_AT, 1.0e-8))
            .with_sink(Box::new(Collect(std::rc::Rc::clone(&seen))))
            .build();
        solver.tran().expect("transient").solve().expect("runs");
        let seen = seen.borrow();
        seen.iter()
            .filter(|o| o.path == "c" && (o.time - FLIP_AT).abs() < 1.0e-12)
            .filter_map(|o| match &o.kind {
                ObservationKind::Change { bytes } => Some(bytes.clone()),
                _ => None,
            })
            .collect()
    }

    /// The settled at helper `settled_at`.
    fn settled_at(time: f64) -> Vec<Vec<u8>> {
        SETTLED.with(|s| s.borrow().iter().filter(|(t, _)| (t - time).abs() < 1.0e-12).map(|(_, c)| c.clone()).collect())
    }

    /// The settled before helper `settled_before`.
    fn settled_before(time: f64) -> Vec<Vec<u8>> {
        SETTLED.with(|s| s.borrow().iter().filter(|(t, _)| *t < time - 1.0e-12).map(|(_, c)| c.clone()).collect())
    }
}
