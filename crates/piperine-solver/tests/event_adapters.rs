//! Adapters for the remaining three event sources (ABI-38, ABI-39):
//! analog breakpoints, scheduled live sets, and `$bound_step` hints —
//! each enters the unified [`EventQueue`] with the correct `EventKind`,
//! `EventPriority`, and `RollbackBehavior`.
//!
//! Together with the digital adapter (T24, ABI-37), this completes the
//! four-source unification surface. The next step (T26) wires
//! `predict_step` to read from the unified queue.

use piperine_solver::abi::{
    EventKind, EventPriority, EventQueue, EventSource, EventTarget, RollbackBehavior,
};

// ── ABI-38: analog breakpoints via next_breakpoints ────────────────────────

/// An analog breakpoint (pulse edge, PWL corner, `@timer`) declared via
/// `Element::next_breakpoints` enters the unified queue with
/// `kind=Breakpoint`, `priority=Exact`, `rollback=RePoll` — stateless, so
/// re-declared each step (not restored on reject).
#[test]
fn breakpoint_adapter_sets_correct_semantics() {
    let mut queue = EventQueue::new();
    queue.push_breakpoint(1.5e-6, 3, "vpulse");

    let front = queue.peek().expect("entry pushed");
    assert_eq!(front.kind, EventKind::Breakpoint);
    assert_eq!(front.priority, EventPriority::Exact);
    assert_eq!(front.rollback, RollbackBehavior::RePoll);
    assert!(matches!(front.target, EventTarget::Source(3)));
    assert!(matches!(front.source, EventSource::Element(_)));
    assert!((front.time - 1.5e-6).abs() < f64::MIN_POSITIVE);
}

/// Breakpoints drained during a rejected step stay out of the queue on
/// rollback — they will be re-declared by `next_breakpoints` on the next
/// prediction (stateless source).
#[test]
fn drained_breakpoint_stays_out_on_rollback() {
    let mut queue = EventQueue::new();
    queue.push_breakpoint(1e-6, 0, "vpulse");

    queue.checkpoint();
    let drained = queue.drain_due(1e-6);
    assert_eq!(drained.len(), 1);
    assert!(queue.is_empty());

    queue.rollback();
    assert!(queue.is_empty(), "RePoll-tagged breakpoint does NOT return on rollback");
}

// ── ABI-38: scheduled live sets ────────────────────────────────────────────

/// A scheduled live-parameter set (LIVE-06) enters the unified queue with
/// breakpoint-landing semantics (`kind=Breakpoint`, `priority=Exact`) but
/// `Restore` rollback — a pending write does not vanish because the step
/// that would apply it was rejected.
#[test]
fn scheduled_set_adapter_sets_correct_semantics() {
    let mut queue = EventQueue::new();
    queue.push_scheduled_set(2.5e-6);

    let front = queue.peek().expect("entry pushed");
    assert_eq!(front.kind, EventKind::Breakpoint);
    assert_eq!(front.priority, EventPriority::Exact);
    assert_eq!(front.rollback, RollbackBehavior::Restore);
    assert_eq!(front.source, EventSource::ScheduledSet);
    assert!((front.time - 2.5e-6).abs() < f64::MIN_POSITIVE);
}

/// A drained scheduled set returns to the queue on rollback — the pending
/// write survives the rejected step and applies at the retry.
#[test]
fn drained_scheduled_set_returns_on_rollback() {
    let mut queue = EventQueue::new();
    queue.push_scheduled_set(1e-6);

    queue.checkpoint();
    let drained = queue.drain_due(1e-6);
    assert_eq!(drained.len(), 1);
    assert!(queue.is_empty());

    queue.rollback();
    assert_eq!(queue.len(), 1, "Restore-tagged scheduled set returns");
    let front = queue.peek().unwrap();
    assert_eq!(front.source, EventSource::ScheduledSet);
}

// ── ABI-39: $bound_step hints ──────────────────────────────────────────────

/// A `$bound_step` hint enters the unified queue with `kind=StepHint`,
/// `priority=Advisory`, `rollback=Discard` — soft floor on the step size,
/// re-emitted next attempt.
#[test]
fn step_hint_adapter_sets_correct_semantics() {
    let mut queue = EventQueue::new();
    queue.push_step_hint(1e-7, "mos1");

    let front = queue.peek().expect("entry pushed");
    assert_eq!(front.kind, EventKind::StepHint);
    assert_eq!(front.priority, EventPriority::Advisory);
    assert_eq!(front.rollback, RollbackBehavior::Discard);
    assert!(matches!(front.source, EventSource::Element(_)));
    assert!((front.time - 1e-7).abs() < f64::MIN_POSITIVE);
}

/// Step hints yield to exact-priority events at the same time — a digital
/// event scheduled at the same time as a step hint surfaces first.
#[test]
fn step_hint_yields_to_exact_priority_at_equal_time() {
    let mut queue = EventQueue::new();
    queue.push_step_hint(1e-6, "x1");
    queue.push_breakpoint(1e-6, 0, "vpulse");

    let front = queue.peek().unwrap();
    assert_eq!(front.kind, EventKind::Breakpoint, "Exact (breakpoint) outranks Advisory (step hint)");
}

/// Step hints drained during a rejected step stay out — re-emitted next
/// attempt by the device's `bound_step_hint()`.
#[test]
fn drained_step_hint_stays_out_on_rollback() {
    let mut queue = EventQueue::new();
    queue.push_step_hint(1e-6, "x1");

    queue.checkpoint();
    let drained = queue.drain_due(1e-6);
    assert_eq!(drained.len(), 1);
    assert!(queue.is_empty());

    queue.rollback();
    assert!(queue.is_empty(), "Discard-tagged step hint does NOT return on rollback");
}

// ── Combined: all three new sources coexist with digital ───────────────────

/// ABI-36/38/39: peek surfaces the earliest-time event across all four
/// sources. With identical times, Exact (digital/breakpoint/set) wins
/// over Advisory (step hint).
#[test]
fn unified_queue_reads_earliest_across_all_sources() {
    let mut queue = EventQueue::new();
    queue.push_step_hint(0.5e-6, "x1"); // earliest but Advisory
    queue.push_breakpoint(1e-6, 0, "vpulse");
    queue.push_scheduled_set(1e-6);
    queue.push_digital_event(
        &piperine_solver::abi::DigitalEvent {
            time: 1e-6,
            net: piperine_solver::abi::DigitalNet(0),
            value: piperine_solver::abi::LogicValue::One,
            source: 0,
            seq: 0,
        },
        "u0",
    );

    // Earliest = 0.5e-6 (step hint, only entry at that time).
    assert!((queue.peek_next_time() - 0.5e-6).abs() < f64::MIN_POSITIVE);

    // Drain it; the next earliest is 1e-6 with three Exact entries.
    let first = queue.drain_due(0.5e-6);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].kind, EventKind::StepHint);

    let next = queue.peek().unwrap();
    assert!((next.time - 1e-6).abs() < f64::MIN_POSITIVE);
    assert_eq!(next.priority, EventPriority::Exact);
}
