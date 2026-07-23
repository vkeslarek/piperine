//! Unified event model (ABI-36): one typed queue for every time-
//! discontinuity source the transient driver must land on.
//!
//! Today [`TransientSolver::predict_step`](crate::analyses::transient::TransientSolver)
//! merges four ad-hoc sources by hand: the digital event queue, analog
//! `next_breakpoints`, the scheduled live-set times, and the per-device
//! `$bound_step` hint. Adding a new event kind means rewriting `predict_step`.
//! This module replaces that with one [`EventQueue`] holding typed
//! [`EventEntry`] records — `predict_step` reads from a single source, and
//! each entry carries its own [`RollbackBehavior`] so the reject path can
//! honor per-source semantics (digital events restored, breakpoints
//! re-polled, crossings discarded).
//!
//! The queue does not *replace* the digital scheduler's storage overnight;
//! [`digital::DigitalState::event_queue`](crate::digital::DigitalState) can
//! remain as the backing store for digital events, with [`EventQueue`]
//! wrapping it via an adapter. The goal is ONE read path in `predict_step`,
//! not necessarily one storage layer.
//!
//! The wiring into the transient driver lands incrementally (T24 digital
//! adapter, T25 breakpoint/set/bound adapters, T26 predict_step unification,
//! T27 per-entry rollback). Until then the types live here with
//! `dead_code` allowed.

#![allow(dead_code)]

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use crate::digital::{DigitalEvent, DigitalNet};

// ── enums: kind, priority, source, rollback ────────────────────────────────

/// What sort of time-discontinuity this entry represents. Determines where
/// it is generated and how the solver treats the landing point (exact vs
/// advisory) and what happens on rejection (per [`RollbackBehavior`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Digital scheduler net change — emitted from a digital element's
    /// `comb_phase`/`seq_phase`. The landing point must be exact so the
    /// analog solve sees the post-edge D2A state at the right time.
    Digital,
    /// Analog discontinuity (pulse edge, PWL corner, `@timer` fire) declared
    /// via [`Element::next_breakpoints`](crate::core::element::AnalogDevice::next_breakpoints).
    /// The integrator lands exactly on the breakpoint and skips the Milne
    /// LTE gate for that step.
    Breakpoint,
    /// `$bound_step` advisory floor — a device's hint that no step larger
    /// than this should be taken. Soft, not mandatory; the stepper may
    /// shrink further for its own reasons but will not exceed it.
    StepHint,
    /// Analog crossing detected (A2D comparator without a digital
    /// scheduler). Currently advisory — the integrator notes the time but
    /// does not necessarily land on it exactly.
    Crossing,
}

/// The recipient / subject of the event — drives how the solver dispatches
/// it on landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTarget {
    /// Digital net index the event toggles.
    Net(DigitalNet),
    /// Analog source index (pulse/PWL element) for breakpoint re-evaluation.
    Source(usize),
    /// No specific target — the event is advisory (`$bound_step`).
    Advisory,
}

/// Whether the event's time is mandatory or advisory. `Exact` (digital,
/// breakpoints) forces the integrator to land on `time`; `Advisory`
/// (`$bound_step`) only caps the step size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPriority {
    /// Must land exactly on this time (digital, breakpoint).
    Exact,
    /// Soft floor (`$bound_step` — preferred, not mandatory).
    Advisory,
}

impl EventPriority {
    /// `Exact` outranks `Advisory` at the same time. The queue stores
    /// `Reverse<EventEntry>` in a max-heap, so the smaller rank (Exact)
    /// surfaces first at `peek()` — Exact before Advisory at equal times.
    fn rank(self) -> u8 {
        match self {
            EventPriority::Exact => 0,
            EventPriority::Advisory => 1,
        }
    }
}

/// Where the event originated — diagnostic only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSource {
    /// Element instance name (for diagnostics).
    Element(String),
    /// Scheduled live-parameter set (LIVE-06).
    ScheduledSet,
    /// System (integrator, digital scheduler).
    System,
}

/// What the queue does with this entry when its step is rejected. Per-source
/// semantics — the queue honors each entry's behavior individually (ABI-40,
/// ABI-41).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackBehavior {
    /// Restore this event on rejection (digital events — re-fire on retry).
    Restore,
    /// Re-poll next step (breakpoints are stateless — re-declared each
    /// step, so the queue drops them on reject and re-populates from
    /// `next_breakpoints` on the next prediction).
    RePoll,
    /// Discard on rejection (crossings — re-detected next attempt; the
    /// device re-emits if the condition still holds).
    Discard,
}

// ── EventEntry ─────────────────────────────────────────────────────────────

/// One entry in the unified event queue. Each entry is one time-discontinuity
/// the integrator must respect: a digital net edge, an analog breakpoint,
/// a `$bound_step` advisory, or an analog crossing.
///
/// Ordering is `(time, priority)` — earliest time first, `Exact` before
/// `Advisory` at ties — so a `BinaryHeap<Reverse<EventEntry>>` surfaces the
/// next-landing event at `peek()`.
#[derive(Debug, Clone)]
pub struct EventEntry {
    pub kind: EventKind,
    pub time: f64,
    pub target: EventTarget,
    pub priority: EventPriority,
    pub source: EventSource,
    pub rollback: RollbackBehavior,
}

impl EventEntry {
    /// Build a digital-net event entry with the standard digital semantics:
    /// `kind=Digital`, `priority=Exact`, `rollback=Restore`.
    pub fn digital(time: f64, net: DigitalNet, source: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Digital,
            time,
            target: EventTarget::Net(net),
            priority: EventPriority::Exact,
            source: EventSource::Element(source.into()),
            rollback: RollbackBehavior::Restore,
        }
    }

    /// Build an analog breakpoint entry: `kind=Breakpoint`, `priority=Exact`,
    /// `rollback=RePoll` (stateless — re-declared each step).
    pub fn breakpoint(time: f64, source_index: usize, source: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Breakpoint,
            time,
            target: EventTarget::Source(source_index),
            priority: EventPriority::Exact,
            source: EventSource::Element(source.into()),
            rollback: RollbackBehavior::RePoll,
        }
    }

    /// Build a scheduled live-set entry (LIVE-06): a host-scheduled
    /// parameter write due at `time`. Same landing semantics as a
    /// breakpoint (`kind=Breakpoint`, `priority=Exact`) but `Restore` on
    /// rejection — a pending write does not vanish because the step that
    /// would apply it was rejected; it stays pending for the retry.
    pub fn scheduled_set(time: f64) -> Self {
        Self {
            kind: EventKind::Breakpoint,
            time,
            target: EventTarget::Advisory,
            priority: EventPriority::Exact,
            source: EventSource::ScheduledSet,
            rollback: RollbackBehavior::Restore,
        }
    }

    /// Build a `$bound_step` advisory entry: `kind=StepHint`,
    /// `priority=Advisory`, `rollback=Discard`.
    pub fn step_hint(time: f64, source: impl Into<String>) -> Self {
        Self {
            kind: EventKind::StepHint,
            time,
            target: EventTarget::Advisory,
            priority: EventPriority::Advisory,
            source: EventSource::Element(source.into()),
            rollback: RollbackBehavior::Discard,
        }
    }

    /// Build an analog crossing entry: `kind=Crossing`, `priority=Advisory`,
    /// `rollback=Discard` (re-detected next attempt if the condition holds).
    pub fn crossing(time: f64, source: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Crossing,
            time,
            target: EventTarget::Advisory,
            priority: EventPriority::Advisory,
            source: EventSource::Element(source.into()),
            rollback: RollbackBehavior::Discard,
        }
    }
}

impl PartialEq for EventEntry {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.time.total_cmp(&other.time) == Ordering::Equal
            && self.target == other.target
            && self.priority == other.priority
            && self.source == other.source
            && self.rollback == other.rollback
    }
}

impl Eq for EventEntry {}

impl PartialOrd for EventEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EventEntry {
    /// Order by `(time, priority)`. Lower time sorts first; at equal times,
    /// `Exact` before `Advisory`. `BinaryHeap` is a max-heap, so the
    /// transient driver stores `Reverse<EventEntry>` and peeks the min.
    fn cmp(&self, other: &Self) -> Ordering {
        let time_cmp = self.time.total_cmp(&other.time);
        if time_cmp != Ordering::Equal {
            return time_cmp;
        }
        self.priority.rank().cmp(&other.priority.rank())
    }
}

// ── EventQueue ─────────────────────────────────────────────────────────────

/// One-deep queue checkpoint used by [`EventQueue::rollback`]. Captures the
/// pre-attempt heap so a rejected step restores the drained `Restore`-tagged
/// entries and drops anything pushed during the failed attempt (ABI-40/41).
#[derive(Clone)]
struct Checkpoint {
    heap: BinaryHeap<Reverse<EventEntry>>,
    /// Entries drained between [`EventQueue::checkpoint`] and the next
    /// [`EventQueue::rollback`] / [`EventQueue::commit`]. Tracked so the
    /// per-entry `RollbackBehavior` can be honored without the caller
    /// having to remember what was drained.
    drained_since: Vec<EventEntry>,
}

/// The unified event queue. Wraps a `BinaryHeap<Reverse<EventEntry>>` so
/// `peek()` returns the earliest entry (min-time, then `Exact` over
/// `Advisory`).
///
/// One-deep checkpoint semantics mirror [`DigitalState::Checkpoint`](crate::digital::DigitalState):
/// `checkpoint()` snapshots the heap before a candidate step; `rollback()`
/// restores it on rejection, honoring each drained entry's
/// [`RollbackBehavior`] (ABI-40/41) — `Restore`-tagged entries return to the
/// queue, `RePoll`/`Discard`-tagged entries stay out (re-populated by
/// `next_breakpoints` next step, or re-detected by the device).
/// `commit()` drops the snapshot on acceptance.
pub struct EventQueue {
    heap: BinaryHeap<Reverse<EventEntry>>,
    checkpoint: Option<Checkpoint>,
}

impl Default for EventQueue {
    fn default() -> Self {
        Self { heap: BinaryHeap::new(), checkpoint: None }
    }
}

impl EventQueue {
    /// Build an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event. The heap re-orders internally on `pop`, not `push`,
    /// so this is O(log n).
    pub fn push(&mut self, entry: EventEntry) {
        self.heap.push(Reverse(entry));
    }

    /// Push a digital scheduler event into the unified queue (ABI-37).
    /// Adapts [`DigitalEvent`] → [`EventEntry::digital`]: `kind=Digital`,
    /// `priority=Exact`, `rollback=Restore`. The source label is the
    /// emitting element's name (or `DigitalState::label_or_default` when
    /// no element-level name is available).
    pub fn push_digital_event(
        &mut self,
        event: &DigitalEvent,
        source_label: impl Into<String>,
    ) {
        self.push(EventEntry::digital(event.time, event.net, source_label));
    }

    /// Push an analog breakpoint declared via
    /// [`Element::next_breakpoints`](crate::core::element::AnalogDevice::next_breakpoints)
    /// (ABI-38). Adapts a polled time into [`EventEntry::breakpoint`]:
    /// `kind=Breakpoint`, `priority=Exact`, `rollback=RePoll` (breakpoints
    /// are stateless — re-declared each step).
    pub fn push_breakpoint(
        &mut self,
        time: f64,
        source_index: usize,
        source_label: impl Into<String>,
    ) {
        self.push(EventEntry::breakpoint(time, source_index, source_label));
    }

    /// Push a scheduled live-parameter set (LIVE-06) into the unified queue
    /// (ABI-38). Same landing semantics as a breakpoint (`kind=Breakpoint`,
    /// `priority=Exact`) but `Restore` on rejection — a pending write stays
    /// pending through a rejected step.
    pub fn push_scheduled_set(&mut self, time: f64) {
        self.push(EventEntry::scheduled_set(time));
    }

    /// Push a `$bound_step` advisory hint (ABI-39). Adapts the device's
    /// `bound_step_hint()` into [`EventEntry::step_hint`]:
    /// `kind=StepHint`, `priority=Advisory`, `rollback=Discard` (the
    /// device re-emits next attempt).
    pub fn push_step_hint(&mut self, time: f64, source_label: impl Into<String>) {
        self.push(EventEntry::step_hint(time, source_label));
    }

    /// The earliest event time, or `+inf` when the queue is empty. The
    /// transient driver's `predict_step` reads this to choose its landing
    /// point.
    pub fn peek_next_time(&self) -> f64 {
        self.heap.peek().map(|Reverse(e)| e.time).unwrap_or(f64::INFINITY)
    }

    /// Peek the earliest entry (if any) — used by callers that need the
    /// kind/target (e.g., to set `landed_on_breakpoint`).
    pub fn peek(&self) -> Option<&EventEntry> {
        self.heap.peek().map(|Reverse(e)| e)
    }

    /// Remove and return every entry due at or before `now`, in time order.
    /// Used by the predict/accept path to drain events whose time has come.
    /// Drained entries are recorded against the active checkpoint (if any)
    /// so [`rollback`](Self::rollback) can honor per-entry semantics.
    pub fn drain_due(&mut self, now: f64) -> Vec<EventEntry> {
        let mut due = Vec::new();
        while self
            .heap
            .peek()
            .map(|Reverse(front)| front.time.total_cmp(&now) != Ordering::Greater)
            .unwrap_or(false)
        {
            if let Some(Reverse(entry)) = self.heap.pop() {
                if let Some(chk) = &mut self.checkpoint {
                    chk.drained_since.push(entry.clone());
                }
                due.push(entry);
            }
        }
        due
    }

    /// Snapshot the current heap (one-deep) and start tracking drained
    /// entries. Call before a candidate step.
    pub fn checkpoint(&mut self) {
        self.checkpoint = Some(Checkpoint {
            heap: self.heap.clone(),
            drained_since: Vec::new(),
        });
    }

    /// Restore the heap from the last [`checkpoint`](Self::checkpoint),
    /// honoring each drained entry's [`RollbackBehavior`] (ABI-40/41):
    /// `Restore`-tagged entries return to the queue; `RePoll`/`Discard`
    /// entries stay out (they will be re-declared by `next_breakpoints`
    /// next step or re-detected by the device next attempt). Anything
    /// pushed during the failed attempt is dropped.
    pub fn rollback(&mut self) {
        let Some(chk) = self.checkpoint.take() else {
            return;
        };
        let mut restored = chk.heap;
        // Rebuild the heap keeping Restore-tagged drained entries; drop
        // RePoll/Discard ones. Future events (not drained) always survive.
        let non_restore: Vec<EventEntry> = chk
            .drained_since
            .into_iter()
            .filter(|e| e.rollback != RollbackBehavior::Restore)
            .collect();
        if non_restore.is_empty() {
            self.heap = restored;
            return;
        }
        let mut keep = BinaryHeap::with_capacity(restored.len());
        while let Some(Reverse(entry)) = restored.pop() {
            if !non_restore.contains(&entry) {
                keep.push(Reverse(entry));
            }
        }
        self.heap = keep;
    }

    /// Drop the snapshot without restoring — call when the step is accepted.
    pub fn commit(&mut self) {
        self.checkpoint = None;
    }

    /// Number of pending events (diagnostic / tests).
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Whether the queue has no pending events.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digital::LogicValue;

    fn entry(time: f64, priority: EventPriority, kind: EventKind) -> EventEntry {
        EventEntry {
            kind,
            time,
            target: EventTarget::Advisory,
            priority,
            source: EventSource::System,
            rollback: RollbackBehavior::Discard,
        }
    }

    /// ABI-36: ordering is `(time, priority)` — earliest first, then `Exact`
    /// before `Advisory` at equal times. With `Reverse<EventEntry>` in a
    /// max-heap, smaller Ord surfaces at peek — so `Exact` must rank below
    /// `Advisory` in the natural ordering.
    #[test]
    fn event_entry_orders_by_time_then_priority() {
        let early = entry(1e-6, EventPriority::Advisory, EventKind::Crossing);
        let late = entry(2e-6, EventPriority::Exact, EventKind::Breakpoint);
        assert!(early < late, "earlier time wins regardless of priority");

        let same_time_exact = entry(1e-6, EventPriority::Exact, EventKind::Digital);
        let same_time_advisory = entry(1e-6, EventPriority::Advisory, EventKind::StepHint);
        assert!(same_time_exact < same_time_advisory, "Exact < Advisory at equal time (so Reverse surfaces Exact first)");
    }

    #[test]
    fn binary_heap_peek_returns_earliest_via_reverse() {
        let mut queue = EventQueue::new();
        queue.push(entry(3e-6, EventPriority::Exact, EventKind::Breakpoint));
        queue.push(entry(1e-6, EventPriority::Advisory, EventKind::StepHint));
        queue.push(entry(2e-6, EventPriority::Exact, EventKind::Digital));

        // peek surfaces earliest time.
        assert!((queue.peek_next_time() - 1e-6).abs() < f64::MIN_POSITIVE);
        // At equal time, Exact sorts first.
        queue.push(entry(1e-6, EventPriority::Exact, EventKind::Breakpoint));
        let front = queue.peek().expect("non-empty");
        assert_eq!(front.priority, EventPriority::Exact);
        assert!((front.time - 1e-6).abs() < f64::MIN_POSITIVE);
    }

    #[test]
    fn drain_due_returns_earliest_entries_in_time_order() {
        let mut queue = EventQueue::new();
        queue.push(entry(3e-6, EventPriority::Exact, EventKind::Breakpoint));
        queue.push(entry(1e-6, EventPriority::Advisory, EventKind::StepHint));
        queue.push(entry(2e-6, EventPriority::Exact, EventKind::Digital));

        let due = queue.drain_due(2e-6);
        assert_eq!(due.len(), 2);
        assert!((due[0].time - 1e-6).abs() < f64::MIN_POSITIVE);
        assert!((due[1].time - 2e-6).abs() < f64::MIN_POSITIVE);
        assert_eq!(queue.len(), 1, "later event stays pending");
    }

    #[test]
    fn empty_queue_peek_is_positive_infinity() {
        let queue = EventQueue::new();
        assert!(queue.peek_next_time().is_infinite());
        assert!(queue.is_empty());
    }

    #[test]
    fn constructors_set_correct_kind_priority_and_rollback() {
        let digital = EventEntry::digital(1e-6, DigitalNet(0), "u1");
        assert_eq!(digital.kind, EventKind::Digital);
        assert_eq!(digital.priority, EventPriority::Exact);
        assert_eq!(digital.rollback, RollbackBehavior::Restore);

        let bp = EventEntry::breakpoint(2e-6, 7, "vpulse");
        assert_eq!(bp.kind, EventKind::Breakpoint);
        assert_eq!(bp.priority, EventPriority::Exact);
        assert_eq!(bp.rollback, RollbackBehavior::RePoll);

        let set = EventEntry::scheduled_set(3e-6);
        assert_eq!(set.kind, EventKind::Breakpoint);
        assert_eq!(set.priority, EventPriority::Exact);
        assert_eq!(set.rollback, RollbackBehavior::Restore);
        assert_eq!(set.source, EventSource::ScheduledSet);

        let hint = EventEntry::step_hint(4e-6, "x1");
        assert_eq!(hint.kind, EventKind::StepHint);
        assert_eq!(hint.priority, EventPriority::Advisory);
        assert_eq!(hint.rollback, RollbackBehavior::Discard);

        let crossing = EventEntry::crossing(5e-6, "cmp");
        assert_eq!(crossing.kind, EventKind::Crossing);
        assert_eq!(crossing.priority, EventPriority::Advisory);
        assert_eq!(crossing.rollback, RollbackBehavior::Discard);
    }

    /// ABI-38/39: the four event sources (digital, breakpoint, scheduled
    /// set, step hint) coexist in the unified queue. At equal times,
    /// `Exact` (digital/breakpoint/set) outranks `Advisory` (step hint).
    #[test]
    fn all_four_sources_coexist_in_unified_queue() {
        let mut queue = EventQueue::new();
        queue.push_digital_event(
            &DigitalEvent {
                time: 2e-6,
                net: DigitalNet(0),
                value: LogicValue::One,
                source: 0,
                seq: 0,
            },
            "u0",
        );
        queue.push_breakpoint(1e-6, 0, "vpulse");
        queue.push_scheduled_set(3e-6);
        queue.push_step_hint(1e-6, "x1");

        // Earliest time = 1e-6; at that time, Exact (breakpoint) outranks
        // Advisory (step hint).
        let front = queue.peek().expect("non-empty");
        assert_eq!(front.kind, EventKind::Breakpoint);
        assert_eq!(front.priority, EventPriority::Exact);
        assert!((front.time - 1e-6).abs() < f64::MIN_POSITIVE);
    }

    /// Per-source rollback behavior on rejection: digital and scheduled
    /// sets return (Restore); breakpoints and step hints stay out
    /// (RePoll/Discard).
    #[test]
    fn rollback_honors_per_source_rollback_behavior() {
        let mut queue = EventQueue::new();
        queue.push_digital_event(
            &DigitalEvent {
                time: 1e-6,
                net: DigitalNet(0),
                value: LogicValue::One,
                source: 0,
                seq: 0,
            },
            "u0",
        );
        queue.push_breakpoint(1e-6, 0, "vpulse");
        queue.push_scheduled_set(1e-6);
        queue.push_step_hint(1e-6, "x1");

        queue.checkpoint();
        let drained = queue.drain_due(1e-6);
        assert_eq!(drained.len(), 4);

        queue.rollback();
        // Digital + scheduled set return; breakpoint + step hint stay out.
        let mut kinds: Vec<EventKind> = Vec::new();
        while let Some(Reverse(e)) = queue.heap.pop() {
            kinds.push(e.kind);
        }
        assert_eq!(kinds.len(), 2, "only Restore-tagged entries return");
        assert!(kinds.contains(&EventKind::Digital));
        assert!(kinds.contains(&EventKind::Breakpoint), "scheduled set maps to Breakpoint kind");
    }

    /// One-deep checkpoint: snapshot before drain, restore brings back the
    /// Restore-tagged drained entries (digital events).
    #[test]
    fn checkpoint_rollback_restores_only_restore_tagged_entries() {
        let mut queue = EventQueue::new();
        queue.push(EventEntry::digital(1e-6, DigitalNet(0), "u1"));
        queue.push(EventEntry::digital(2e-6, DigitalNet(0), "u1"));

        queue.checkpoint();
        let drained = queue.drain_due(2e-6);
        assert_eq!(drained.len(), 2);
        assert!(queue.is_empty());

        // Both drained entries are Restore-tagged (digital); rollback
        // brings them back.
        queue.rollback();
        assert_eq!(queue.len(), 2, "Restore-tagged entries return");
    }

    /// RePoll/Discard-tagged entries are dropped on rollback (they will be
    /// re-declared by `next_breakpoints` next step / re-detected by the
    /// device next attempt).
    #[test]
    fn rollback_drops_non_restore_drained_entries() {
        let mut queue = EventQueue::new();
        queue.push(EventEntry::breakpoint(1e-6, 0, "src")); // RePoll
        queue.push(EventEntry::digital(2e-6, DigitalNet(0), "u1")); // Restore
        queue.push(EventEntry::crossing(3e-6, "cmp")); // Discard

        queue.checkpoint();
        let drained = queue.drain_due(3e-6);
        assert_eq!(drained.len(), 3);

        // Rollback honors per-entry behavior: only Restore returns.
        queue.rollback();
        assert_eq!(queue.len(), 1, "only the Restore entry returns");
        assert_eq!(queue.peek().unwrap().kind, EventKind::Digital);
    }

    /// Entries pushed during the failed attempt are dropped by rollback
    /// (the snapshot is the pre-attempt heap).
    #[test]
    fn rollback_drops_entries_pushed_during_attempt() {
        let mut queue = EventQueue::new();
        queue.push(EventEntry::digital(1e-6, DigitalNet(0), "u1"));

        queue.checkpoint();
        // Simulate a push during the attempt (e.g., by the digital settle).
        queue.push(EventEntry::digital(2e-6, DigitalNet(1), "u2"));

        // The new push should be dropped on rollback — the snapshot was
        // pre-attempt.
        queue.rollback();
        assert_eq!(queue.len(), 1, "new push during attempt is dropped");
        let front = queue.peek().unwrap();
        assert_eq!(front.target, EventTarget::Net(DigitalNet(0)));
    }

    #[test]
    fn commit_drops_checkpoint_so_rollback_is_noop() {
        let mut queue = EventQueue::new();
        queue.push(entry(1e-6, EventPriority::Exact, EventKind::Digital));
        queue.checkpoint();
        queue.drain_due(1e-6);
        queue.commit();
        // No checkpoint to restore from — rollback is a no-op.
        queue.rollback();
        assert!(queue.is_empty());
    }
}
