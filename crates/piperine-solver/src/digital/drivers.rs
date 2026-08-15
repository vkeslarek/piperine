//! Per-net driver records for LRM §7.9 driver-receiver segregation
//! (LRMC-C7-05, T34 — data half; the `$driver_*` system-function surface
//! is T35).
//!
//! **Layering note (task-card correction):** the original task card names
//! `gates.rs`'s `Strength` table as reusable here — that type lives in
//! `piperine-codegen::kernel::digital::gates` (`piperine-codegen` depends
//! on `piperine-solver`, never the reverse — CLAUDE.md: "`piperine-solver`
//! ... does not depend on codegen"), so it is unreachable from this crate
//! by design. [`Strength`] below is this crate's own copy of the same
//! LRM Table 7-2/7-3 semantics (identical variant set and `resolve`
//! behavior, verified against codegen's own test cases), not a shared
//! type — consolidating the two is a follow-up, not this task's scope.
//!
//! §7.9's segregation rule itself ("ordinary module drivers/receivers
//! propagate through the analog domain instead of directly to ordinary
//! module receivers; connect module drivers group with ordinary module
//! receivers and vice versa") is a *routing* concern over the digital
//! network topology — this module only stores, per net, which drivers
//! exist, their kind (ordinary vs connect module — the piece segregation
//! routing needs to tell the two driver populations apart), value,
//! strength, and pending next-state, and resolves them per Table 7-3.
//! Building the actual segregated propagation graph is out of this
//! task's scope (T36/T37).

use std::collections::HashMap;

use super::events::{DigitalNet, LogicValue};

/// LRM §7.9 Table 7-2 drive-strength levels, ordered strongest-first
/// within the two classes the resolution table distinguishes: driving
/// strengths (`Supply`..`Weak`) and the non-driving `HighZ`. `#[repr(u8)]`
/// order is significant — [`Strength::resolve`] compares by discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Strength {
    Supply = 0,
    Strong = 1,
    Pull = 2,
    Weak = 3,
    HighZ = 4,
}

impl Strength {
    /// Resolves two drivers' `(value, strength)` pairs per Table 7-3: the
    /// stronger driver wins; equal-strength opposing values contend to
    /// `X` (at that strength); a `HighZ` (non-driving) side yields to the
    /// other unconditionally.
    pub fn resolve(a: (LogicValue, Strength), b: (LogicValue, Strength)) -> (LogicValue, Strength) {
        let (av, asr) = a;
        let (bv, bsr) = b;
        if asr == Strength::HighZ {
            return b;
        }
        if bsr == Strength::HighZ {
            return a;
        }
        match asr.cmp(&bsr) {
            std::cmp::Ordering::Less => a,
            std::cmp::Ordering::Greater => b,
            std::cmp::Ordering::Equal => {
                if av == bv {
                    (av, asr)
                } else {
                    (LogicValue::X, asr)
                }
            }
        }
    }
}

/// §7.9: whether a driver belongs to an ordinary module or an
/// auto-inserted connect module — the segregation rule groups these two
/// populations oppositely on each side of an analog crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    Ordinary,
    ConnectModule,
}

/// Opaque driver identity — which source (an instance's output port, a
/// `dv-tester`-style host driver, etc.) contributed this record. Callers
/// mint these; `DriverTable` never interprets the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DriverId(pub usize);

/// One driver's current contribution to a net, plus its already-scheduled
/// next transition (`$driver_next_state`/`$driver_next_strength`, T35).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriverRecord {
    pub source: DriverId,
    pub kind: DriverKind,
    pub value: LogicValue,
    pub strength: Strength,
    pub delay: f64,
    pub next_state: Option<LogicValue>,
    pub next_strength: Option<Strength>,
}

/// Per-net driver lists (§7.9) — multiple drivers on one net produce a
/// queryable, ordered list; [`DriverTable::resolved`] folds them through
/// [`Strength::resolve`] the same way the digital kernel would.
#[derive(Debug, Default, Clone)]
pub struct DriverTable {
    drivers: HashMap<DigitalNet, Vec<DriverRecord>>,
}

impl DriverTable {
    pub fn new() -> Self {
        Self { drivers: HashMap::new() }
    }

    /// Installs or updates `source`'s current driver record on `net`.
    /// Re-driving the same `source` replaces its prior record (a driver
    /// contributes at most one entry per net at a time) rather than
    /// accumulating duplicates.
    pub fn set_driver(&mut self, net: DigitalNet, source: DriverId, kind: DriverKind, value: LogicValue, strength: Strength, delay: f64) {
        let list = self.drivers.entry(net).or_default();
        match list.iter_mut().find(|r| r.source == source) {
            Some(r) => {
                r.kind = kind;
                r.value = value;
                r.strength = strength;
                r.delay = delay;
            }
            None => list.push(DriverRecord { source, kind, value, strength, delay, next_state: None, next_strength: None }),
        }
    }

    /// Records `source`'s pending next transition on `net`
    /// (`$driver_next_state`/`$driver_next_strength`, T35) without
    /// disturbing its current value/strength.
    pub fn set_next_state(&mut self, net: DigitalNet, source: DriverId, next_state: LogicValue, next_strength: Strength) {
        if let Some(r) = self.drivers.entry(net).or_default().iter_mut().find(|r| r.source == source) {
            r.next_state = Some(next_state);
            r.next_strength = Some(next_strength);
        }
    }

    /// Removes `source`'s driver record from `net` (e.g. a `Z` driver
    /// releasing the net entirely rather than contributing `HighZ`).
    pub fn remove_driver(&mut self, net: DigitalNet, source: DriverId) {
        if let Some(list) = self.drivers.get_mut(&net) {
            list.retain(|r| r.source != source);
        }
    }

    pub fn driver_count(&self, net: DigitalNet) -> usize {
        self.drivers.get(&net).map_or(0, |l| l.len())
    }

    /// The driver records on `net`, in insertion order — index-addressable
    /// for `$driver_state(net, i)` and friends (T35).
    pub fn drivers(&self, net: DigitalNet) -> &[DriverRecord] {
        self.drivers.get(&net).map_or(&[], |l| l.as_slice())
    }

    /// Folds every driver on `net` through [`Strength::resolve`] — `(X,
    /// HighZ)` (an undriven net) if there are none.
    pub fn resolved(&self, net: DigitalNet) -> (LogicValue, Strength) {
        self.drivers(net)
            .iter()
            .map(|r| (r.value, r.strength))
            .fold((LogicValue::X, Strength::HighZ), Strength::resolve)
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
    fn single_driver_resolves_to_its_own_value() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Strong, 0.0);
        assert_eq!(t.driver_count(net(0)), 1);
        assert_eq!(t.resolved(net(0)), (One, Strength::Strong));
    }

    #[test]
    fn stronger_driver_wins_table_7_3() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Strong, 0.0);
        t.set_driver(net(0), DriverId(2), DriverKind::Ordinary, Zero, Strength::Weak, 0.0);
        assert_eq!(t.driver_count(net(0)), 2);
        assert_eq!(t.resolved(net(0)), (One, Strength::Strong));
    }

    #[test]
    fn equal_strength_opposing_values_contend_to_x() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Pull, 0.0);
        t.set_driver(net(0), DriverId(2), DriverKind::Ordinary, Zero, Strength::Pull, 0.0);
        assert_eq!(t.resolved(net(0)), (X, Strength::Pull));
    }

    #[test]
    fn high_z_driver_yields_unconditionally() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, Z, Strength::HighZ, 0.0);
        t.set_driver(net(0), DriverId(2), DriverKind::Ordinary, One, Strength::Weak, 0.0);
        assert_eq!(t.resolved(net(0)), (One, Strength::Weak));
    }

    #[test]
    fn undriven_net_resolves_to_x_highz() {
        let t = DriverTable::new();
        assert_eq!(t.driver_count(net(0)), 0);
        assert_eq!(t.resolved(net(0)), (X, Strength::HighZ));
    }

    #[test]
    fn re_driving_the_same_source_replaces_not_accumulates() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Strong, 0.0);
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, Zero, Strength::Weak, 1.0);
        assert_eq!(t.driver_count(net(0)), 1);
        assert_eq!(t.resolved(net(0)), (Zero, Strength::Weak));
    }

    #[test]
    fn removing_a_driver_releases_the_net() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Strong, 0.0);
        t.remove_driver(net(0), DriverId(1));
        assert_eq!(t.driver_count(net(0)), 0);
        assert_eq!(t.resolved(net(0)), (X, Strength::HighZ));
    }

    /// §7.9: an ordinary-module driver and a connect-module driver on the
    /// same net are still resolved by strength like any pair, but their
    /// `kind` stays distinguishable per-record — the piece segregation
    /// routing (T36/T37) needs to group the two populations oppositely.
    #[test]
    fn driver_kind_distinguishes_ordinary_from_connect_module_sources() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Strong, 0.0);
        t.set_driver(net(0), DriverId(2), DriverKind::ConnectModule, Zero, Strength::Strong, 2e-9);
        let recs = t.drivers(net(0));
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].kind, DriverKind::Ordinary);
        assert_eq!(recs[1].kind, DriverKind::ConnectModule);
        assert_eq!(recs[1].delay, 2e-9);
    }

    #[test]
    fn next_state_is_queryable_independent_of_current_state() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, Zero, Strength::Strong, 0.0);
        t.set_next_state(net(0), DriverId(1), One, Strength::Strong);
        let rec = &t.drivers(net(0))[0];
        assert_eq!(rec.value, Zero, "current value unchanged by scheduling a next-state");
        assert_eq!(rec.next_state, Some(One));
        assert_eq!(rec.next_strength, Some(Strength::Strong));
    }

    #[test]
    fn multiple_nets_stay_independent() {
        let mut t = DriverTable::new();
        t.set_driver(net(0), DriverId(1), DriverKind::Ordinary, One, Strength::Strong, 0.0);
        t.set_driver(net(1), DriverId(1), DriverKind::Ordinary, Zero, Strength::Strong, 0.0);
        assert_eq!(t.resolved(net(0)), (One, Strength::Strong));
        assert_eq!(t.resolved(net(1)), (Zero, Strength::Strong));
    }
}
