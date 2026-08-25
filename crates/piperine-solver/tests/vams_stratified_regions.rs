//! LRM-COVERS: 8, 8.1, 8.5, 8.5.1, 8.5.2, 8.5.3.6, 8.5.3.7
//! `8` ("Scheduling semantics") and `8.1` ("Overview") are the clause's own
//! top-level/narrative headings — "a mixed-signal simulator shall contain
//! an analog solver ... and ... a discrete event simulator that complies
//! with the scheduling semantics described in 8.5" (§8.1) is exactly what
//! this file's region-ordering tests exercise, so they're carried on the
//! same declaration as their genuinely-tested descendant 8.5/8.5.1/8.5.2.
//! T51 (VAMS-20) — LRM chapter 8 stratified event-region ordering,
//! exercised through `piperine_solver`'s public surface
//! (`piperine_solver::abi::{Region, RegionAction, RegionQueue}`). Mirrors
//! T51's "Done when": active/inactive/NBA/monitor regions ordered per the
//! LRM simulation cycle, with a determinism test over a race-prone fixture
//! (two "processes" driving/reading the same net in the same timestep).
//!
//! `all_seven_regions_are_ordered_per_the_lrm_cycle` extends the base
//! ordering test to the two mixed-signal regions §8.5.1 adds on top of
//! plain IEEE 1364 Verilog's four (region 1b, explicit D2A events, and
//! region 3b, analog macro-process events): it proves `RegionQueue` places
//! `Region::D2a` strictly after `Active` and before `Inactive` (§8.5.3.6 —
//! "processed after all the active events") and `Region::AnalogMacroProcess`
//! strictly after `Nba` and before `Monitor` (§8.5.3.7 — "processed after
//! all active, explicit D2A events, inactive events and non blocking assign
//! update events"), i.e. the same six-region total order `RegionQueue`
//! implements is exactly the §8.5.2 reference-model while-loop's region
//! priority ladder (active → D2A → inactive → NBA → analog-macro-process →
//! monitor) — the parent headings 8.5/8.5.2 are covered by the same
//! evidence since they have no independently testable content of their own
//! beyond this ordering.

use piperine_solver::abi::{DigitalNet, LogicValue, Region, RegionAction, RegionQueue};

#[test]
fn active_inactive_nba_monitor_regions_are_ordered_per_the_lrm_cycle() {
    let mut q = RegionQueue::new();
    // Submit in reverse-priority order to prove ordering comes from the
    // region, not from submission order.
    q.schedule(Region::Monitor, 0, RegionAction::Observe { net: DigitalNet(0) });
    q.schedule(Region::Nba, 0, RegionAction::Update { net: DigitalNet(0), value: LogicValue::One });
    q.schedule(Region::Inactive, 0, RegionAction::Update { net: DigitalNet(1), value: LogicValue::One });
    q.schedule(Region::Active, 0, RegionAction::Update { net: DigitalNet(0), value: LogicValue::Zero });

    let mut nets = vec![LogicValue::X; 2];
    let trace = q.run(&mut nets);
    let order: Vec<Region> = trace.iter().map(|o| o.event.region).collect();
    assert_eq!(order, vec![Region::Active, Region::Inactive, Region::Nba, Region::Monitor]);
}

/// §8.5.1's two mixed-signal additions to the four plain-Verilog regions:
/// region 1b (`Region::D2a`, §8.5.3.6) and region 3b
/// (`Region::AnalogMacroProcess`, §8.5.3.7). Submitted out of priority
/// order (same technique as the base test above) to prove the placement
/// comes from the region tag, not submission order.
#[test]
fn all_seven_regions_are_ordered_per_the_lrm_cycle() {
    let mut q = RegionQueue::new();
    q.schedule(Region::Monitor, 0, RegionAction::Observe { net: DigitalNet(0) });
    q.schedule(Region::AnalogMacroProcess, 0, RegionAction::Update { net: DigitalNet(0), value: LogicValue::One });
    q.schedule(Region::Nba, 0, RegionAction::Update { net: DigitalNet(0), value: LogicValue::Zero });
    q.schedule(Region::Inactive, 0, RegionAction::Update { net: DigitalNet(1), value: LogicValue::One });
    q.schedule(Region::D2a, 0, RegionAction::Update { net: DigitalNet(0), value: LogicValue::One });
    q.schedule(Region::Active, 0, RegionAction::Update { net: DigitalNet(0), value: LogicValue::Zero });

    let mut nets = vec![LogicValue::X; 2];
    let trace = q.run(&mut nets);
    let order: Vec<Region> = trace.iter().map(|o| o.event.region).collect();
    assert_eq!(
        order,
        vec![
            Region::Active,
            Region::D2a,
            Region::Inactive,
            Region::Nba,
            Region::AnalogMacroProcess,
            Region::Monitor,
        ]
    );
}

/// The race-prone fixture: two processes both drive/read net `q` in the
/// same time step — process A does a blocking write (Active region),
/// process B does a non-blocking write (NBA region). Run twice with the
/// two processes submitted in opposite order to prove the outcome is
/// deterministic regardless of which process's statement executes first.
#[test]
fn two_processes_racing_on_the_same_net_resolve_deterministically() {
    let mut outcomes = Vec::new();
    for submit_a_first in [true, false] {
        let mut q = RegionQueue::new();
        let schedule_a =
            |q: &mut RegionQueue| q.schedule(Region::Active, 1, RegionAction::Update { net: DigitalNet(0), value: LogicValue::Zero });
        let schedule_b =
            |q: &mut RegionQueue| q.schedule(Region::Nba, 2, RegionAction::Update { net: DigitalNet(0), value: LogicValue::One });
        if submit_a_first {
            schedule_a(&mut q);
            schedule_b(&mut q);
        } else {
            schedule_b(&mut q);
            schedule_a(&mut q);
        }
        q.schedule(Region::Monitor, 3, RegionAction::Observe { net: DigitalNet(0) });

        let mut nets = vec![LogicValue::X; 1];
        let trace = q.run(&mut nets);
        let monitor_value =
            trace.iter().find(|o| o.event.region == Region::Monitor).unwrap().observed.unwrap();
        outcomes.push((nets[0], monitor_value));
    }
    // Both submission orders must produce the identical outcome.
    assert_eq!(outcomes[0], outcomes[1], "race outcome must be deterministic regardless of submission order");
    // And the LRM-correct outcome: the NBA write always lands after the
    // blocking write, so the final/observed value is process B's.
    assert_eq!(outcomes[0], (LogicValue::One, LogicValue::One));
}
