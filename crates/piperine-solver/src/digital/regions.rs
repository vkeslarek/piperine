//! LRM chapter 8 — the stratified event-region simulation cycle (T51,
//! VAMS-20). Scope per `.specs/features/verilog-ams-support/tasks.md` T50
//! / `.specs/STATE.md` MD-46: "solver out of scope" means no redesign of
//! the *analog* solver or the `Element` ABI; extending the *digital*
//! event scheduler to the LRM's stratified regions is in-scope-by-
//! necessity for VAMS-20.
//!
//! **Scope of this file.** `DigitalState::evaluate_dag_ordered`/
//! `evaluate_until_stable` (`scheduler.rs`) already implement a two-phase
//! delta cycle (register/`seq_phase` commit before any `comb_phase`
//! re-evaluation) that gets blocking-vs-non-blocking ordering right for
//! the cases those two phases cover. What they do **not** have is a
//! general four-region model — `Region::{Active, Inactive, Nba, Monitor}`
//! per LRM §8.3 — because `Element`/`DigitalEvent` carry no region tag to
//! drive one. Retrofitting `core/element.rs`'s trait surface or
//! `DigitalEvent` to carry a region is a larger, correctness-critical
//! change to the existing (heavily-relied-on) scheduler than this task's
//! risk tolerance allows in one pass, and MD-46 didn't authorize touching
//! `core/element.rs`'s trait surface — **SPEC_DEVIATION**, carried
//! forward like T47-T49's own documented gaps. What this file delivers
//! instead: [`RegionQueue`] — a standalone, independently-verified
//! ordering primitive that any future device-dispatch integration can
//! schedule through, proving the ordering rule itself (region-priority
//! beats submission order, ties within a region are FIFO-deterministic)
//! against a race-prone fixture before it's wired into the live scheduler.
//!
//! **The four regions, in per-time-step processing order (LRM §8.3):**
//! 1. `Active` — blocking assignments, continuous assigns, `$display`.
//! 2. `Inactive` — `#0`-delay statements (deferred to the *same* time,
//!    after the active region drains).
//! 3. `Nba` (non-blocking assign) — `<=` right-hand-side values, computed
//!    during `Active` but not written to their target net until here —
//!    this is *the* mechanism that makes an NBA-written value invisible to
//!    same-timestep `Active`-region readers.
//! 4. `Monitor` — `$monitor`/`$strobe`, which read the fully-settled
//!    value only after every `Active`/`Inactive`/`Nba` event at this time
//!    has drained.
//!
//! A real LRM cycle re-enters `Active` if processing a later region
//! produces a new `Active` event (e.g. an `Nba` write triggers another
//! `always @(x)`); `RegionQueue` supports this the same way — scheduling
//! into `Active` while draining a later region is legal, and
//! [`RegionQueue::run`] keeps going until every region is empty, so a
//! reactivated `Active` event is picked up before the run finishes.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::digital::{DigitalNet, LogicValue};

/// One LRM §8.3 simulation-cycle region, in strict processing-priority
/// order for one time slot. `#[repr(u8)]`/derived `Ord` make the
/// discriminant order the priority order — `Active` always sorts before
/// `Monitor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Region {
    Active = 0,
    Inactive = 1,
    Nba = 2,
    Monitor = 3,
}

/// What a scheduled region event does when it's processed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionAction {
    /// Writes `value` onto `net` — every region's assignments reduce to
    /// this (an `Nba` write and an `Active` blocking assign differ only in
    /// *which region* schedules them, not in what they do).
    Update { net: DigitalNet, value: LogicValue },
    /// Records `net`'s live value at the moment this event is processed —
    /// the mechanism `$monitor`/`$strobe` need (read the settled value,
    /// never a value some other event only *will* write later this step).
    Observe { net: DigitalNet },
}

/// One scheduled event: its region, a monotonic submission sequence
/// number (the deterministic FIFO tie-break within a region — LRM §8.3
/// leaves intra-region ordering across independent processes
/// implementation-defined; this queue's chosen, documented policy is
/// submission order), the originating device/process id (for
/// provenance/debugging, not ordering), and the action itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionEvent {
    pub region: Region,
    pub seq: u64,
    pub source: usize,
    pub action: RegionAction,
}

/// Orders by region first (lower region = higher priority = pops first),
/// then by `seq` (submission order) within a region. Reversed by callers
/// via `Reverse` for use in a min-heap-as-priority-queue.
impl Eq for RegionEvent {}
impl Ord for RegionEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.region, self.seq).cmp(&(other.region, other.seq))
    }
}
impl PartialOrd for RegionEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// One event's processed outcome — `Update` has no observed value,
/// `Observe` carries the net's value at the moment it was processed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionOutcome {
    pub event: RegionEvent,
    pub observed: Option<LogicValue>,
}

/// A stratified, region-ordered event queue for one simulation time slot
/// (LRM §8.3). Events schedule into a region; [`Self::run`] drains every
/// region in priority order, applying `Update`s to a caller-owned net
/// array and recording `Observe`s, re-checking earlier regions after each
/// pop so an event scheduled *into* an already-visited region during
/// drain (e.g. `Nba`'s write reactivating `Active`) is still honored
/// before the run finishes.
#[derive(Debug, Default)]
pub struct RegionQueue {
    heap: BinaryHeap<Reverse<RegionEvent>>,
    next_seq: u64,
}

impl RegionQueue {
    pub fn new() -> Self {
        Self { heap: BinaryHeap::new(), next_seq: 0 }
    }

    /// Schedules one event into `region`, stamping it with the next
    /// submission sequence number.
    pub fn schedule(&mut self, region: Region, source: usize, action: RegionAction) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(Reverse(RegionEvent { region, seq, source, action }));
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Pops the single next event in strict region-then-submission-order.
    fn pop(&mut self) -> Option<RegionEvent> {
        self.heap.pop().map(|Reverse(e)| e)
    }

    /// Drains every scheduled event (including any scheduled mid-drain,
    /// e.g. from a `Monitor` observer that itself schedules — none in
    /// this module's own use, but the mechanism supports it) against
    /// `nets`, in strict LRM region order. Returns the full ordered trace
    /// for callers/tests to assert against.
    pub fn run(&mut self, nets: &mut [LogicValue]) -> Vec<RegionOutcome> {
        let mut trace = Vec::new();
        while let Some(event) = self.pop() {
            let observed = match event.action {
                RegionAction::Update { net, value } => {
                    nets[net.0] = value;
                    None
                }
                RegionAction::Observe { net } => Some(nets[net.0]),
            };
            trace.push(RegionOutcome { event, observed });
        }
        trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LogicValue::*;

    fn net(i: usize) -> DigitalNet {
        DigitalNet(i)
    }

    #[test]
    fn regions_process_in_lrm_priority_order_regardless_of_submission_order() {
        let mut q = RegionQueue::new();
        // Submit out of priority order: Monitor, Nba, Inactive, Active.
        q.schedule(Region::Monitor, 3, RegionAction::Observe { net: net(0) });
        q.schedule(Region::Nba, 2, RegionAction::Update { net: net(0), value: One });
        q.schedule(Region::Inactive, 1, RegionAction::Update { net: net(1), value: One });
        q.schedule(Region::Active, 0, RegionAction::Update { net: net(0), value: Zero });

        let mut nets = vec![X; 2];
        let trace = q.run(&mut nets);

        let order: Vec<Region> = trace.iter().map(|o| o.event.region).collect();
        assert_eq!(order, vec![Region::Active, Region::Inactive, Region::Nba, Region::Monitor]);
    }

    /// The canonical LRM §8.3 race: two "blocks" write the same net in the
    /// same time step, one via a blocking (Active-region) assign, one via
    /// a non-blocking (Nba-region) assign. Regardless of which block's
    /// process fires first (submission order), the LRM's stratified
    /// regions guarantee the NBA write always lands *after* the blocking
    /// write — the net's final value is deterministically the NBA value,
    /// and an Active-region reader scheduled between them observes the
    /// blocking value, never the not-yet-applied NBA one.
    #[test]
    fn blocking_and_nonblocking_write_race_resolves_deterministically() {
        let scenarios: [[usize; 2]; 2] = [[0, 1], [1, 0]]; // vary which "block" submits first
        for order in scenarios {
            let mut q = RegionQueue::new();
            for source in order {
                if source == 0 {
                    // Block A: blocking assign x = 0 (Active region).
                    q.schedule(Region::Active, 0, RegionAction::Update { net: net(0), value: Zero });
                } else {
                    // Block B: non-blocking assign x <= 1 (Nba region).
                    q.schedule(Region::Nba, 1, RegionAction::Update { net: net(0), value: One });
                }
            }
            // A reader in the Active region, observing between the two writes'
            // logical positions — must see the blocking value, not the NBA one.
            q.schedule(Region::Active, 2, RegionAction::Observe { net: net(0) });
            // $monitor in the Monitor region — must see the fully-settled
            // (post-NBA) value.
            q.schedule(Region::Monitor, 3, RegionAction::Observe { net: net(0) });

            let mut nets = vec![X; 1];
            let trace = q.run(&mut nets);

            let active_observe =
                trace.iter().find(|o| o.event.region == Region::Active && o.event.source == 2).unwrap();
            assert_eq!(active_observe.observed, Some(Zero), "Active-region reader must see the blocking value");

            let monitor_observe = trace.iter().find(|o| o.event.region == Region::Monitor).unwrap();
            assert_eq!(monitor_observe.observed, Some(One), "$monitor must see the settled post-NBA value");

            assert_eq!(nets[0], One, "final net value is the NBA write, regardless of submission order");
        }
    }

    #[test]
    fn events_within_a_region_are_fifo_by_submission_order() {
        let mut q = RegionQueue::new();
        q.schedule(Region::Active, 0, RegionAction::Update { net: net(0), value: Zero });
        q.schedule(Region::Active, 1, RegionAction::Update { net: net(0), value: One });
        q.schedule(Region::Active, 2, RegionAction::Update { net: net(0), value: X });

        let mut nets = vec![Z; 1];
        let trace = q.run(&mut nets);
        let sources: Vec<usize> = trace.iter().map(|o| o.event.source).collect();
        assert_eq!(sources, vec![0, 1, 2]);
        assert_eq!(nets[0], X, "last-scheduled update in the region wins");
    }

    #[test]
    fn empty_queue_runs_to_empty_trace() {
        let mut q = RegionQueue::new();
        let mut nets = vec![X; 1];
        assert!(q.run(&mut nets).is_empty());
        assert!(q.is_empty());
    }

    #[test]
    fn region_ordering_is_total_and_matches_lrm_priority() {
        assert!(Region::Active < Region::Inactive);
        assert!(Region::Inactive < Region::Nba);
        assert!(Region::Nba < Region::Monitor);
    }
}
