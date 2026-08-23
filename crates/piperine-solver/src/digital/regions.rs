//! LRM chapter 8 — the stratified event-region simulation cycle (T51,
//! VAMS-20). Scope per `.specs/features/verilog-ams-support/tasks.md` T50
//! / `.specs/STATE.md` MD-46: "solver out of scope" means no redesign of
//! the *analog* solver or the `Element` ABI; extending the *digital*
//! event scheduler to the LRM's stratified regions is in-scope-by-
//! necessity for VAMS-20.
//!
//! **Wired into the live scheduler (T38/LRMC-C7-12).** `DigitalEvent`
//! (`events.rs`) now carries a [`Region`] field, and its `Ord` compares
//! `(time, region, seq)` instead of `(time, seq)` — the exact ordering
//! rule this file's [`RegionQueue`] independently proved
//! (region-priority beats submission order, FIFO within a region) is now
//! *the* tie-break the production event queue itself (`DigitalState::
//! event_queue`, a `BinaryHeap<Reverse<DigitalEvent>>`) uses. Both
//! `evaluate_until_stable` and `evaluate_dag_ordered` (`scheduler.rs`)
//! drain that same heap via successive `pop()`, which already yields
//! events in ascending `Ord` — so both production paths now apply a
//! same-timestamp batch in strict region-priority order with **no
//! further change to either method's own code**: extending `DigitalEvent`
//! instead of literally invoking `RegionQueue::run()` in the hot delta-
//! cycle loop avoids adding a second heap/allocation to the
//! (heavily-relied-on) scheduler's inner loop, while reusing the same
//! ordering rule `RegionQueue` proved correct. `core/element.rs`'s trait
//! surface is untouched (MD-46) — every existing event source (the JIT
//! `QueueSink::emit` runtime path) stamps `Region::Active`, so this is a
//! behavior-preserving superset for every pre-T38 caller (see
//! `events.rs`'s own `stratified_regions_resolve_the_canonical_active_
//! nba_race_through_evaluate_until_stable`/`inactive_region_lands_
//! after_active_...`/`monitor_region_lands_last_...` — the production-
//! path proofs). `RegionQueue` itself stays as the independently-verified
//! reference oracle its own tests already exercise; nothing here forces a
//! caller to *also* route through it.
//!
//! **The six modeled regions, in per-time-step processing order (LRM
//! §8.5.1; T58/LRMW2-51 added the two mixed-signal ones, marked below):**
//! 1. `Active` — blocking assignments, continuous assigns, `$display`.
//! 1b. `D2a` — explicit D2A events (an analog-to-digital-domain bridge
//!    write), processed after every `Active` event at this instant.
//! 2. `Inactive` — `#0`-delay statements (deferred to the *same* time,
//!    after the active and D2A regions drain).
//! 3. `Nba` (non-blocking assign) — `<=` right-hand-side values, computed
//!    during `Active` but not written to their target net until here —
//!    this is *the* mechanism that makes an NBA-written value invisible to
//!    same-timestep `Active`-region readers.
//! 3b. `AnalogMacroProcess` — the analog kernel's own settled-step
//!    notification, processed after active, D2A, inactive, and NBA.
//! 4. `Monitor` — `$monitor`/`$strobe`, which read the fully-settled
//!    value only after every earlier region at this time has drained.
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

/// One LRM §8.5.1 simulation-cycle region, in strict processing-priority
/// order for one time slot. `#[repr(u8)]`/derived `Ord` make the
/// discriminant order the priority order — `Active` always sorts before
/// `Monitor`. §8.5.1's own text: "The Verilog event queue is logically
/// segmented into *seven* different regions... Regions 1b and 3b have
/// been added for mixed-signal simulation" — [`D2a`](Region::D2a) (1b)
/// and [`AnalogMacroProcess`](Region::AnalogMacroProcess) (3b) are those
/// two mixed-signal additions (T58/LRMW2-51); this crate does not model
/// every one of the LRM's seven (future/non-blocking-future split, PLI
/// callback regions) since nothing in this codebase schedules into
/// them yet — the same "model what has a real caller" posture the
/// original four-region set already followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Region {
    Active = 0,
    /// §8.5.1 region 1b: "Explicit D2A events that occur at the current
    /// simulation time shall be processed after all the active events
    /// are processed" — an analog-to-digital-domain boundary write
    /// (an A2D/D2A bridge device driving a digital net from a settled
    /// analog sample) lands here, strictly after every ordinary
    /// `Active`-region digital write at the same instant but still
    /// before `Inactive`.
    D2a = 1,
    Inactive = 2,
    Nba = 3,
    /// §8.5.1 region 3b: "Analog macro-process events shall be
    /// processed after all active, explicit D2A events, inactive
    /// events and non blocking assign update events are processed" —
    /// the analog kernel's own settled-step notification (the D2A
    /// direction's counterpart to region 1b, run once digital events at
    /// this instant have otherwise fully resolved) lands here, strictly
    /// before `Monitor`.
    AnalogMacroProcess = 4,
    Monitor = 5,
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

    /// §8.5.1's own stated order across all six modeled regions (the
    /// four pre-T58 regions plus 1b/3b): "Active... 1b... Inactive...
    /// NBA... 3b... Monitor".
    #[test]
    fn all_six_regions_order_per_section_8_5_1() {
        assert!(Region::Active < Region::D2a);
        assert!(Region::D2a < Region::Inactive);
        assert!(Region::Inactive < Region::Nba);
        assert!(Region::Nba < Region::AnalogMacroProcess);
        assert!(Region::AnalogMacroProcess < Region::Monitor);
    }

    /// Done-when: "a D2A event and an analog macro-process event resolve
    /// in the LRM's stated order against a §8.5.1 worked scenario" — a
    /// scenario mixing every region (submitted out of priority order,
    /// per this file's own established discrimination style) must drain
    /// in exactly `Active, D2a, Inactive, Nba, AnalogMacroProcess,
    /// Monitor` order.
    #[test]
    fn d2a_and_analog_macro_process_resolve_in_the_lrm_stated_order() {
        let mut q = RegionQueue::new();
        // Submitted deliberately out of priority order.
        q.schedule(Region::Monitor, 5, RegionAction::Observe { net: net(0) });
        q.schedule(Region::AnalogMacroProcess, 4, RegionAction::Update { net: net(0), value: One });
        q.schedule(Region::Nba, 3, RegionAction::Update { net: net(0), value: Zero });
        q.schedule(Region::Inactive, 2, RegionAction::Observe { net: net(0) });
        q.schedule(Region::D2a, 1, RegionAction::Update { net: net(0), value: One });
        q.schedule(Region::Active, 0, RegionAction::Update { net: net(0), value: Zero });

        let mut nets = vec![X; 1];
        let trace = q.run(&mut nets);

        let order: Vec<Region> = trace.iter().map(|o| o.event.region).collect();
        assert_eq!(
            order,
            vec![Region::Active, Region::D2a, Region::Inactive, Region::Nba, Region::AnalogMacroProcess, Region::Monitor]
        );

        // The Inactive-region reader observes the D2A write (region 1b
        // has already drained by the time Inactive runs), not the
        // Active-region write it overwrote, nor either later write —
        // "processed after all the active events" (1b's own text).
        let inactive_observe = trace.iter().find(|o| o.event.region == Region::Inactive).unwrap();
        assert_eq!(inactive_observe.observed, Some(One), "Inactive reader must see the D2A write, not the Active one");

        // Final value is the AnalogMacroProcess write — the last region
        // to touch the net in this scenario.
        assert_eq!(nets[0], One, "AnalogMacroProcess (region 3b) is the last write in this scenario");
    }

    /// Existing four-region ordering tests must still pass unchanged
    /// (Done-when) — proven by the fact that
    /// `regions_process_in_lrm_priority_order_regardless_of_submission_order`/
    /// `blocking_and_nonblocking_write_race_resolves_deterministically`/
    /// `events_within_a_region_are_fifo_by_submission_order`/
    /// `empty_queue_runs_to_empty_trace` above are untouched by this
    /// task's diff — this test only re-asserts the four original
    /// regions' relative order still holds now that two more sit
    /// between them.
    #[test]
    fn original_four_regions_keep_their_relative_order_with_the_new_ones_inserted() {
        assert!(Region::Active < Region::Inactive);
        assert!(Region::Inactive < Region::Nba);
        assert!(Region::Nba < Region::Monitor);
    }
}
