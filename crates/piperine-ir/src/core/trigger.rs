//! Block triggers and the two domains (ideal.md §1.5).
//!
//! A block is ops + a domain (`analog`|`digital`) + a trigger. Triggers
//! are events, not a mechanism: an `always` block runs every step, an
//! `edge(sig)` block runs on a clock edge, a `cross(expr)` block runs when
//! an analog expression crosses zero, a `timer(period)` block runs on a
//! parameter-constant schedule, an `initial` block runs once at start.
//! The two domains produce different things and the IR keeps them apart
//! (analog blocks stamp residuals; digital blocks drive signals).
//!
//! The shapes reuse the trigger vocabulary already in the resolve stage
//! (`EdgeKind`/`CrossDir`/event kinds), but the IR's own copies — purity
//! forbids importing codegen into the IR, and the IR carries `ValueId`/
//! `SigRef` references where the resolved form carried POM `Expr`.

use super::{SigRef, ValueId};

/// Which world a block produces into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Residual-stamping: reads `read_across`/`read_through`/`read_signal`,
    /// writes `contribute_through`/`force_across`.
    Analog,
    /// Signal-driving: reads `read_signal`/`read_across` (cross-sampling),
    /// writes `drive`.
    Digital,
}

/// The edge polarity of one atomic digital event term (mirrors
/// `piperine-codegen::resolve::stmt::EdgeKind` — purity forbids an import,
/// but the shape is identical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// `posedge` — 0 → 1.
    Rising,
    /// `negedge` — 1 → 0.
    Falling,
    /// `*` — either edge.
    Any,
}

/// Which sign changes a `cross` trigger fires on (mirrors the runtime
/// `crate::events::CrossDir`; the IR carries its own copy because purity
/// forbids reaching into another module's enum from a field type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossDir {
    /// Fires only on a rising (negative-to-positive) crossing.
    Rising,
    /// Fires only on a falling (positive-to-negative) crossing.
    Falling,
    /// Fires on either direction.
    Either,
}

/// What fires a block. The five triggers ideal.md §1.5 names: `always`,
/// `edge`, `cross`, `timer`, `initial`. There is no sixth and no catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// `always` — digital combinational, or analog residual-on-every-step.
    Always,
    /// `edge(sig, kind)` — digital synchronous. Fires on the named edge of
    /// `sig`. The signal is a [`SigRef`] (a chunk list, ideal.md §3) so a
    /// bus edge can pick a single bit without an `extract` op.
    Edge(SigRef, EdgeKind),
    /// `cross(expr, dir)` — analog. Fires when `expr` crosses zero in the
    /// named direction. Unseeded `cross` never fires (the seeding
    /// asymmetry, ideal.md §3.4).
    Cross(ValueId, CrossDir),
    /// `timer(period, phase)` — analog. Fires on a parameter-constant
    /// schedule: instants known a priori, so they declare static
    /// breakpoints the integrator lands on exactly.
    Timer { period: ValueId, phase: ValueId },
    /// `initial` — fires once at simulation start.
    Initial,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Chunk, SigRef};

    #[test]
    fn trigger_covers_always_edge_cross_timer_initial() {
        // Five triggers, no sixth. Each is a distinct variant.
        let always = Trigger::Always;
        let edge = Trigger::Edge(SigRef(vec![Chunk::Slice { value: ValueId(0), lo: 0, hi: 1 }]), EdgeKind::Rising);
        let cross = Trigger::Cross(ValueId(1), CrossDir::Rising);
        let timer = Trigger::Timer { period: ValueId(2), phase: ValueId(3) };
        let initial = Trigger::Initial;

        let triggers = [&always, &edge, &cross, &timer, &initial];
        for i in 0..triggers.len() {
            for j in (i + 1)..triggers.len() {
                assert_ne!(triggers[i], triggers[j], "triggers {i} and {j} are not distinct");
            }
        }
    }

    #[test]
    fn edge_takes_a_sigref_lets_a_bus_edge_pick_a_single_bit() {
        // The point of `Edge(SigRef, _)` rather than `Edge(ValueId, _)`:
        // a bus edge can pick bit k without an extract op. The SigRef
        // is the chunk-list representation.
        let bus_bit_3 = SigRef(vec![Chunk::Slice { value: ValueId(0), lo: 3, hi: 4 }]);
        let t = Trigger::Edge(bus_bit_3, EdgeKind::Falling);
        match &t {
            Trigger::Edge(chunk, EdgeKind::Falling) => {
                assert_eq!(chunk.0.len(), 1);
            }
            _ => panic!("expected Edge"),
        }
    }

    #[test]
    fn domain_distinguishes_analog_and_digital() {
        assert_ne!(Domain::Analog, Domain::Digital);
    }

    #[test]
    fn cross_dir_distinct_kinds() {
        assert_ne!(CrossDir::Rising, CrossDir::Falling);
        assert_ne!(CrossDir::Rising, CrossDir::Either);
        assert_ne!(CrossDir::Falling, CrossDir::Either);
    }

    #[test]
    fn edge_kind_distinct_polarities() {
        assert_ne!(EdgeKind::Rising, EdgeKind::Falling);
        assert_ne!(EdgeKind::Rising, EdgeKind::Any);
        assert_ne!(EdgeKind::Falling, EdgeKind::Any);
    }
}
