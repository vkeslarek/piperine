//! T51 (VAMS-20) — LRM chapter 8 stratified event-region ordering,
//! exercised through `piperine_solver`'s public surface
//! (`piperine_solver::abi::{Region, RegionAction, RegionQueue}`). Mirrors
//! T51's "Done when": active/inactive/NBA/monitor regions ordered per the
//! LRM simulation cycle, with a determinism test over a race-prone fixture
//! (two "processes" driving/reading the same net in the same timestep).

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
