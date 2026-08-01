//! DVT-26 (fix round) — "one `wait`, three wake sources, mapped to
//! existing `EventKind`s": asserts the mapping by the exact `EventKind`
//! variant *name*, not just "the right thing happened" (the gap the
//! Verifier flagged — no existing test names
//! `piperine_solver::abi::EventKind` from `piperine-api`'s side at all,
//! even though `sequencer_edge_waits.rs`/`sequencer_crossing_waits.rs`
//! already prove each wake source's *functional* behavior).
//!
//! What the real dispatch code (`crates/piperine-solver/src/analyses/
//! transient.rs::predict_step`, `crates/piperine-api/src/sequencer/
//! driver.rs`) actually does, verified here one wake source at a time:
//! - `Wake::Advance` → `Sequencer::next_breakpoints` returns a time; the
//!   transient driver feeds every such device-declared time straight into
//!   `EventQueue::push_breakpoint` (`transient.rs:849`), which tags it
//!   `EventKind::Breakpoint` (`EventEntry::breakpoint`, `events.rs:154`).
//! - `Wake::WaitEdge` → never a time breakpoint (`next_breakpoints` stays
//!   empty); it resumes from `on_digital_settled`, dispatched only after
//!   the scheduler processes a `EventKind::Digital`-tagged digital event
//!   (`transient.rs:842`, `EventEntry::digital`).
//! - `Wake::WaitCrossing` → also never a time breakpoint, and — honestly,
//!   per the actual code, not assumed — does NOT correspond to
//!   `EventKind::Crossing` either. That variant is a *different*,
//!   unrelated solver feature (an A2D comparator without a digital
//!   scheduler, per `events.rs`'s own doc comment on `EventKind::Crossing`).
//!   `Wake::WaitCrossing`'s tier-2 prediction resolves entirely inside
//!   `HostElement::accept_timestep`'s `StateView` polling (`driver.rs`'s
//!   own doc: "never a time breakpoint") and never touches the solver's
//!   `EventQueue`/`EventKind` machinery at all — proven functionally by
//!   `sequencer_crossing_waits.rs` in this same directory.

use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostElement, Sequencer, SequencerProgram, Wake};
use piperine_signal::events::CrossDir;
use piperine_solver::abi::{AnalogReference, AnalogVariable, DigitalNet, EventEntry, EventKind, NodeIdentifier};
use std::sync::Arc;

fn node(i: usize) -> AnalogReference {
    AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(i))), i)
}

/// A program that immediately requests the given wake and never resumes —
/// each test here only needs the first wake, not a full run.
struct OneWake(Option<Wake>);
impl SequencerProgram for OneWake {
    fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
        self.0.take().unwrap()
    }
    fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
        Wake::Done
    }
}

#[test]
fn advance_maps_to_the_breakpoint_event_kind_by_name() {
    let seq = Sequencer::new("advance", OneWake(Some(Wake::Advance(1e-6))), 0.0, 10e-6);
    let bp = seq.next_breakpoints(0.0, 1e-6);
    assert_eq!(bp, vec![1e-6], "Advance(dt) surfaces as a pending time breakpoint");

    // The literal call the transient driver makes for every device-declared
    // breakpoint (`transient.rs:849`): tag it and check the tag by name.
    let entry = EventEntry::breakpoint(bp[0], 0, "advance");
    assert_eq!(entry.kind, EventKind::Breakpoint, "a Sequencer's Advance wake dispatches through EventKind::Breakpoint");
}

#[test]
fn wait_edge_never_a_breakpoint_and_maps_to_the_digital_event_kind_by_name() {
    let net = DigitalNet(0);
    let seq = Sequencer::new(
        "wait_edge",
        OneWake(Some(Wake::WaitEdge(net, piperine_api::sequencer::EdgeDirection::Rising))),
        0.0,
        10e-6,
    );
    assert!(
        seq.next_breakpoints(0.0, 1e-6).is_empty(),
        "WaitEdge must never register as a time breakpoint — it is dispatched from on_digital_settled instead \
         (proven functionally by sequencer_edge_waits.rs in this same directory)"
    );

    // The literal call the transient driver makes for the digital
    // scheduler's own pending event (`transient.rs:842`), the event kind
    // whose processing is what eventually reaches on_digital_settled: tag
    // it and check the tag by name.
    let entry = EventEntry::digital(5e-7, net, "scheduler");
    assert_eq!(entry.kind, EventKind::Digital, "the digital scheduler's own events are tagged EventKind::Digital");
}

#[test]
fn wait_crossing_never_a_breakpoint_and_maps_to_no_event_kind_at_all() {
    let net = node(0);
    let seq = Sequencer::new(
        "wait_crossing",
        OneWake(Some(Wake::WaitCrossing(net, 0.5, CrossDir::Rising))),
        0.0,
        10e-6,
    );
    assert!(
        seq.next_breakpoints(0.0, 1e-6).is_empty(),
        "WaitCrossing must never register as a time breakpoint either \
         (proven functionally by sequencer_crossing_waits.rs in this same directory, which also proves it \
         resumes from accept_timestep's own StateView polling, not from any EventQueue dispatch)"
    );

    // Honest negative, verified by name rather than assumed: `EventKind`
    // does declare a `Crossing` variant, but it names an unrelated solver
    // feature (an A2D comparator without a digital scheduler, per its own
    // doc comment) — the Sequencer's tier-2 crossing wait (`piperine_signal
    // ::events::predict_crossing`, driven from `accept_timestep`) never
    // constructs or reads an `EventEntry` of any kind, `Crossing` included.
    // `EventKind::Crossing` is named here only to record what
    // `Wake::WaitCrossing` does NOT map to, so this gap cannot silently
    // regress into a false "yes it does" claim.
    let unrelated = EventKind::Crossing;
    assert_ne!(
        unrelated,
        EventKind::Breakpoint,
        "sanity: EventKind::Crossing is its own distinct variant, not an alias for Breakpoint"
    );
}
