//! LRM-COVERS: 7.9, 9.22
//!
//! T54/LRMW2-46: `piperine_solver::digital::DriverTable` is populated by
//! the live scheduler (`digital::scheduler`'s event-apply loop, not a
//! mock), and a multiply-driven net resolves per Table 7-3 instead of
//! "whichever event landed last" — the data source
//! `$driver_count`/`$driver_state`/`$driver_strength` (§9.22.1-9.22.3)
//! read from.

use piperine_solver::abi::{DigitalEvent, DigitalNet, DigitalState, DriverId, DriverKind, DriverTable, LogicValue, Region, Strength};

/// A classic tristate-bus scenario: two drivers contend for the same net
/// at the same instant. The stronger one (LRM Table 7-3) wins, and
/// `DriverTable` — the data `$driver_count`/`$driver_state`/
/// `$driver_strength` are specified against (§9.22.1-9.22.3) — reports
/// both live records, not an empty table.
#[test]
fn multiply_driven_net_resolves_per_table_7_3_and_driver_table_is_live() {
    let mut state = DigitalState::new(1);
    let net = DigitalNet(0);

    // Source 0 drives Zero (default strength: Strong, since it's not Z).
    state.schedule(DigitalEvent { time: 1e-9, net, value: LogicValue::Zero, source: 0, seq: 0, region: Region::Active, strength: None });
    // Source 1 drives Z (default strength: HighZ) — should yield entirely
    // to source 0, per Table 7-3's "HighZ side yields unconditionally".
    state.schedule(DigitalEvent { time: 1e-9, net, value: LogicValue::Z, source: 1, seq: 1, region: Region::Active, strength: None });

    state.evaluate_until_stable(1e-9, &mut [], Default::default(), &[]).expect("settles");

    assert_eq!(state.nets[0], LogicValue::Zero, "Strong driver wins over a HighZ (non-driving) one");
    assert_eq!(state.driver_table.driver_count(net), 2, "$driver_count must see both live records");
    let recs = state.driver_table.drivers(net);
    assert_eq!(recs[0].value, LogicValue::Zero, "$driver_state(net, 0)");
    assert_eq!(recs[0].strength, Strength::Strong, "$driver_strength(net, 0)");
    assert_eq!(recs[1].value, LogicValue::Z, "$driver_state(net, 1)");
    assert_eq!(recs[1].strength, Strength::HighZ, "$driver_strength(net, 1)");
}

/// Two equal-strength drivers with opposing values contend to `X`
/// (Table 7-3) — the resolved net value is neither driver's raw value,
/// proving the net is genuinely strength-resolved and not just
/// last-writer-wins.
#[test]
fn equal_strength_opposing_drivers_contend_to_x_on_the_live_net() {
    let mut state = DigitalState::new(1);
    let net = DigitalNet(0);
    state.schedule(DigitalEvent { time: 2e-9, net, value: LogicValue::One, source: 0, seq: 0, region: Region::Active, strength: None });
    state.schedule(DigitalEvent { time: 2e-9, net, value: LogicValue::Zero, source: 1, seq: 1, region: Region::Active, strength: None });

    state.evaluate_until_stable(2e-9, &mut [], Default::default(), &[]).expect("settles");

    assert_eq!(state.nets[0], LogicValue::X, "equal-strength opposing drivers contend to X, not last-writer");
    assert_eq!(state.driver_table.driver_count(net), 2);
}

/// A single-driver net is bit-for-bit unaffected by strength resolution:
/// the resolved value equals the raw event value (regression guard for
/// the overwhelmingly common case — this must not perturb any existing
/// single-driver digital test in the suite).
#[test]
fn single_driver_net_is_unaffected_by_strength_resolution() {
    let mut state = DigitalState::new(1);
    let net = DigitalNet(0);
    state.schedule(DigitalEvent { time: 3e-9, net, value: LogicValue::One, source: 0, seq: 0, region: Region::Active, strength: None });
    state.evaluate_until_stable(3e-9, &mut [], Default::default(), &[]).expect("settles");
    assert_eq!(state.nets[0], LogicValue::One);
    assert_eq!(state.driver_table.driver_count(net), 1);
}

/// §7.9 segregation primitive: an ordinary-module driver population and
/// a connect-module driver population on the same net resolve
/// independently of each other (`DriverTable::resolved_by_kind`) rather
/// than contending directly — asserted on a topology that mixes both
/// kinds on one net, the scenario §7.9's own text describes ("ordinary
/// module drivers/receivers propagate through the analog domain instead
/// of directly to ordinary module receivers").
#[test]
fn segregation_routing_keeps_ordinary_and_connect_module_populations_apart_on_a_mixed_net() {

    let mut table = DriverTable::new();
    let net = DigitalNet(0);
    // Ordinary-module side: two ordinary drivers contend, Strong wins.
    table.set_driver(net, DriverId(0), DriverKind::Ordinary, LogicValue::One, Strength::Strong, 0.0);
    table.set_driver(net, DriverId(1), DriverKind::Ordinary, LogicValue::Zero, Strength::Weak, 0.0);
    // Connect-module side: a single Supply-strength driver.
    table.set_driver(net, DriverId(2), DriverKind::ConnectModule, LogicValue::Zero, Strength::Supply, 0.0);

    // Each population resolves on its own — the ordinary side's answer
    // does not change because a stronger connect-module driver exists.
    assert_eq!(table.resolved_by_kind(net, DriverKind::Ordinary), (LogicValue::One, Strength::Strong));
    assert_eq!(table.resolved_by_kind(net, DriverKind::ConnectModule), (LogicValue::Zero, Strength::Supply));
    // The topology is genuinely mixed (both kinds present, 3 total records).
    assert_eq!(table.driver_count(net), 3);
}
