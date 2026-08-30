//! LRM-COVERS: 8.5.1
//! T68/LRMW2-57: a differential test running the same same-timestamp
//! scenarios through [`RegionQueue`] (the independently-verified
//! reference oracle, `digital::regions`) and the production scheduler
//! (`DigitalState::evaluate_until_stable`), asserting identical observable
//! ordering for the case both mechanisms actually model the same thing —
//! a net with exactly one live driver at a given instant, the
//! overwhelmingly common case for real digital traffic (every device's
//! `EventSink::emit` stamps one net at a time; `QueueSink::emit` hardcodes
//! `Region::Active`, so a genuine cross-region write to the same net from
//! two *different* device sources is a synthetic scenario, not one any
//! real compiled device produces today).
//!
//! **Disclosed, pinned difference (not a stale comment):** when a net has
//! *multiple* live drivers at the same instant — regardless of which
//! region(s) they're in — the production scheduler resolves the net
//! through [`piperine_solver::abi`]'s `DriverTable`/`Strength::resolve`
//! (T54/LRMW2-46, LRM Table 7-3's genuine multi-driver arbitration,
//! `tests/digital_driver_segregation.rs`'s own proof), while `RegionQueue`
//! (this module's reference oracle) has no driver-strength model at
//! all — every `RegionAction::Update` is an unconditional overwrite
//! (`regions.rs::RegionQueue::run`). The two mechanisms answer different
//! questions (region *ordering* vs driver *arbitration*) and only
//! coincide when there is one driver to arbitrate. `DriverTable` also
//! never resets a driver's record once set (`DriverTable::set_driver`
//! replaces only same-source records; a source that never re-fires stays
//! a permanent contender), so the divergence is not confined to one delta
//! cycle — closing it fully is a real per-net-kind (`wire` vs `reg`)
//! classification task on its own, out of this task's budget. Pinned here
//! (not left as a comment claiming parity) via
//! `two_sources_racing_across_regions_diverges_from_the_region_queue_oracle`.

use piperine_solver::abi::{
    DigitalEvent, DigitalNet, DigitalState, LogicValue, Region, RegionAction, RegionQueue,
};

fn net(i: usize) -> DigitalNet {
    DigitalNet(i)
}

/// Runs `events` (already region/seq-stamped) through the production
/// scheduler at time `t` and returns the settled net array.
fn run_production(t: f64, num_nets: usize, events: &[DigitalEvent]) -> Vec<LogicValue> {
    let mut state = DigitalState::new(num_nets);
    for ev in events {
        state.schedule(ev.clone());
    }
    state.evaluate_until_stable(t, &mut [], Default::default(), &[]).expect("settles");
    state.nets.clone()
}

/// Runs the same `events` through `RegionQueue`, starting from `X` on
/// every net (same convention `DigitalState::new` uses), and returns the
/// settled net array.
fn run_region_queue(events: &[DigitalEvent], num_nets: usize) -> Vec<LogicValue> {
    let mut q = RegionQueue::new();
    for ev in events {
        q.schedule(ev.region, ev.source, RegionAction::Update { net: ev.net, value: ev.value });
    }
    let mut nets = vec![LogicValue::X; num_nets];
    q.run(&mut nets);
    nets
}

/// Single-driver-per-net case (the common real one): a batch of writes to
/// *different* nets, spread across every modeled region, arriving out of
/// priority order. Both mechanisms settle to the identical final state —
/// the ordering rule `RegionQueue` proved is exactly what `DigitalEvent`'s
/// own `Ord` (extended in T38) now encodes for the production event queue
/// (`regions.rs`'s own module doc).
#[test]
fn single_driver_per_net_matches_the_region_queue_oracle_across_every_region() {
    let events = [
        DigitalEvent { time: 5.0, net: net(3), value: LogicValue::X, source: 0, seq: 0, region: Region::Monitor, strength: None },
        DigitalEvent { time: 5.0, net: net(2), value: LogicValue::One, source: 1, seq: 1, region: Region::Nba, strength: None },
        DigitalEvent { time: 5.0, net: net(1), value: LogicValue::One, source: 2, seq: 2, region: Region::Inactive, strength: None },
        DigitalEvent { time: 5.0, net: net(0), value: LogicValue::Zero, source: 3, seq: 3, region: Region::Active, strength: None },
    ];
    let production = run_production(5.0, 4, &events);
    let oracle = run_region_queue(&events, 4);
    assert_eq!(production, oracle, "one driver per net: production must match the region-queue oracle exactly");
    assert_eq!(oracle, vec![LogicValue::Zero, LogicValue::One, LogicValue::One, LogicValue::X]);
}

/// Single net, single source, two events at the same instant in different
/// regions (a `reg`'s own blocking-then-later-non-blocking write from the
/// *same* process) — still one live driver (`DriverTable::set_driver`
/// replaces `source`'s own record rather than accumulating), so this also
/// matches: the disclosed gap below is specifically about *different*
/// sources.
#[test]
fn same_source_writing_the_same_net_across_regions_matches_the_oracle() {
    let events = [
        DigitalEvent { time: 6.0, net: net(0), value: LogicValue::Zero, source: 0, seq: 0, region: Region::Active, strength: None },
        DigitalEvent { time: 6.0, net: net(0), value: LogicValue::One, source: 0, seq: 1, region: Region::Nba, strength: None },
    ];
    let production = run_production(6.0, 1, &events);
    let oracle = run_region_queue(&events, 1);
    assert_eq!(production, oracle, "same source, same net, cross-region: still matches (one live driver record)");
    assert_eq!(oracle[0], LogicValue::One, "Nba lands after Active, per §8.5.1's region order");
}

/// Disclosed, pinned difference: two *different* sources write the same
/// net at the same instant in different regions (this file's own module
/// doc has the full explanation). `RegionQueue` — plain region-ordered
/// overwrite — gives the LRM-literal answer (the later region's value,
/// regardless of source). The production scheduler instead arbitrates
/// both sources' values via `Strength::resolve` (T54): equal default
/// strength (`Strong`, neither is `Z`) with opposing values contends to
/// `X`, per Table 7-3 — same mechanism `tests/digital_driver_segregation.
/// rs::equal_strength_opposing_drivers_contend_to_x_on_the_live_net`
/// already proves for a same-region pair. Asserted explicitly, both ways,
/// so a future fix (or accidental regression either direction) is caught
/// here rather than silently drifting.
#[test]
fn two_sources_racing_across_regions_diverges_from_the_region_queue_oracle() {
    let events = [
        DigitalEvent { time: 7.0, net: net(0), value: LogicValue::Zero, source: 0, seq: 0, region: Region::Active, strength: None },
        DigitalEvent { time: 7.0, net: net(0), value: LogicValue::One, source: 1, seq: 1, region: Region::Nba, strength: None },
    ];
    let production = run_production(7.0, 1, &events);
    let oracle = run_region_queue(&events, 1);

    assert_eq!(oracle[0], LogicValue::One, "region-queue oracle: Nba (later region) overwrites Active's value");
    assert_eq!(
        production[0],
        LogicValue::X,
        "production: two distinct sources are live drivers, arbitrated by Strength::resolve (Table 7-3) \
         rather than region-ordered overwrite — equal default strength, opposing values, contends to X"
    );
    assert_ne!(production, oracle, "the two mechanisms diverge here (disclosed above) — pinned, not silently equal");
}
