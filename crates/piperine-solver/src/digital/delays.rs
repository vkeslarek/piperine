//! Per-net delay and timing-check records (T60, LRMC-C10-12 — the *model*
//! half; native `vpi_get_delays`/`vpi_put_delays` read/write routines over
//! this table are T61's separate surface half).
//!
//! **`PathDelay`**: LRM §10 gate/module-path delay (Annex A.4.2's
//! `delay2`/`delay3` rise/fall/turn-off triad) — `crate::digital`'s
//! existing gate-primitive path (`GateElement`, `piperine-codegen`'s
//! `device/prim.rs`) already collapses `#delay` to one scalar fed straight
//! into [`crate::digital::interface::EventSink::emit`]'s `delay` argument
//! (a real, disclosed simplification, `piperine_lang::pom::GateInstance`'s
//! own doc comment); this table is a *separate*, additive per-net record
//! any caller (a VPI-style plugin, T61) can set/read independent of that
//! path, so richer rise/fall/turn-off timing becomes representable without
//! reopening the gate-instantiation grammar.
//!
//! **`TimingCheckTable`**: LRM ch.10's `$setup`/`$hold`/`$width`-family
//! timing-check *limits* as plain readable/writable records — enforcement
//! (raising a violation when a real transient run breaches a limit) is
//! explicitly out of this task's scope ("model half"); this table only
//! makes the limits themselves a first-class, queryable per-net record
//! instead of nonexistent.

use std::collections::HashMap;

use crate::digital::DigitalNet;

/// A gate/module path's propagation delay triad (LRM §10, `delay2`/
/// `delay3` form): separate rise, fall, and turn-off (to high-impedance)
/// times.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PathDelay {
    pub rise: f64,
    pub fall: f64,
    pub turn_off: f64,
}

impl PathDelay {
    /// A single scalar applied uniformly to all three transitions — the
    /// same collapsed form `GateInstance::delay` already uses.
    pub fn uniform(delay: f64) -> Self {
        Self { rise: delay, fall: delay, turn_off: delay }
    }
}

/// Per-net [`PathDelay`] records — readable and writable independent of
/// the gate-primitive `#delay` path.
#[derive(Debug, Clone, Default)]
pub struct DelayTable {
    entries: HashMap<DigitalNet, PathDelay>,
}

impl DelayTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes (or overwrites) `net`'s delay record.
    pub fn set(&mut self, net: DigitalNet, delay: PathDelay) {
        self.entries.insert(net, delay);
    }

    /// Reads `net`'s delay record — `PathDelay::default()` (all-zero) when
    /// nothing was ever set, matching "no delay annotated" rather than an
    /// `Option` a caller has to unwrap for the common case.
    pub fn get(&self, net: DigitalNet) -> PathDelay {
        self.entries.get(&net).copied().unwrap_or_default()
    }

    /// Removes `net`'s record, if any. Returns `true` if one existed.
    pub fn clear(&mut self, net: DigitalNet) -> bool {
        self.entries.remove(&net).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One `$setup`/`$hold`/`$width`-family timing-check kind (LRM ch.10's
/// most common forms — the smallest set worth a distinct variant rather
/// than a bare string, per this crate's own typed-enum convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimingCheckKind {
    Setup,
    Hold,
    Width,
    Period,
    Skew,
    /// `$recovery` (Annex A.7.5.1, T20, LRMW3-07).
    Recovery,
    /// `$removal` (Annex A.7.5.1, T20, LRMW3-07).
    Removal,
}

/// One recorded timing-check limit on a net.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimingCheckLimit {
    pub kind: TimingCheckKind,
    /// The limit value, seconds.
    pub limit: f64,
}

/// Per-net timing-check limit records — a net may carry more than one
/// (e.g. both `Setup` and `Hold` against the same clock edge).
#[derive(Debug, Clone, Default)]
pub struct TimingCheckTable {
    entries: HashMap<DigitalNet, Vec<TimingCheckLimit>>,
}

impl TimingCheckTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a limit to `net`'s record (does not replace existing entries
    /// of a different kind — two `Setup` limits on the same net replace
    /// each other only if the caller removes the old one first via
    /// [`Self::clear`]; a `set` never silently drops a different-kind
    /// entry).
    pub fn set(&mut self, net: DigitalNet, check: TimingCheckLimit) {
        self.entries.entry(net).or_default().push(check);
    }

    /// Reads every limit recorded for `net`, in insertion order. Empty
    /// when nothing was set.
    pub fn get(&self, net: DigitalNet) -> &[TimingCheckLimit] {
        self.entries.get(&net).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Removes every limit recorded for `net`. Returns `true` if any
    /// existed.
    pub fn clear(&mut self, net: DigitalNet) -> bool {
        self.entries.remove(&net).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_table_is_readable_and_writable() {
        let mut table = DelayTable::new();
        let net = DigitalNet(0);
        assert_eq!(table.get(net), PathDelay::default(), "unset net reads as all-zero");

        table.set(net, PathDelay { rise: 1e-9, fall: 2e-9, turn_off: 3e-9 });
        assert_eq!(table.get(net), PathDelay { rise: 1e-9, fall: 2e-9, turn_off: 3e-9 });

        // Overwrite.
        table.set(net, PathDelay::uniform(5e-9));
        assert_eq!(table.get(net), PathDelay::uniform(5e-9));

        assert!(table.clear(net));
        assert_eq!(table.get(net), PathDelay::default());
        assert!(!table.clear(net), "clearing an already-empty net reports false");
    }

    #[test]
    fn timing_check_table_accumulates_multiple_kinds_per_net() {
        let mut table = TimingCheckTable::new();
        let net = DigitalNet(1);
        assert!(table.get(net).is_empty());

        table.set(net, TimingCheckLimit { kind: TimingCheckKind::Setup, limit: 1e-9 });
        table.set(net, TimingCheckLimit { kind: TimingCheckKind::Hold, limit: 0.5e-9 });

        let limits = table.get(net);
        assert_eq!(limits.len(), 2);
        assert_eq!(limits[0].kind, TimingCheckKind::Setup);
        assert_eq!(limits[1].kind, TimingCheckKind::Hold);

        assert!(table.clear(net));
        assert!(table.get(net).is_empty());
    }
}
