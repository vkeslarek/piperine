//! IR nets — nature-generic `node` and `signal` (ideal.md §1.2).
//!
//! The IR never names a volt. A conservative node carries TWO named
//! natures — one ACROSS (measured between two nodes) and one THROUGH
//! (summed to zero at the node = KCL) — but the IR ops that read or
//! write them (`read_across`, `read_through`, `contribute_through`,
//! `force_across` in T5) are nature-generic. What is universal is the
//! STRUCTURE, not the names: `V`/`I` are POM-level metadata that never
//! descend, and a thermal discipline (`temp`/`pwr`) constructs through
//! the SAME API as an electrical one (`voltage`/`current`).
//!
//! A `signal` net carries a value but has NO conservation law and NO KCL
//! row by construction — a driven signal-flow net cannot produce a
//! `SymbolicSingular` (the A0b mechanism: the bug is unrepresentable).

use super::{NatureId, Ty};

/// Which side of a conservative pair a nature measures.
///
/// The names are the IR's own — `across` (between two nodes; the
/// potential-like quantity) and `through` (summed to zero at a node; the
/// flow-like quantity). The IR never says `V`/`I`: those are POM-level
/// access names tied to a specific discipline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatureKind {
    /// Measured BETWEEN two nodes (potential-like: voltage, temperature, …).
    Across,
    /// Summed to zero at a node (flow-like: current, power, …).
    Through,
}

/// What a discipline names one of its natures — the IR carries the id
/// (in [`NetKind::Node`]) and uses this info only for diagnostics; the
/// structure does not depend on the names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatureInfo {
    /// The discipline's access name for this nature: `"voltage"`, `"current"`,
    /// `"temp"`, `"pwr"`, … — never hardcoded as `V`/`I` in the IR.
    pub access: String,
    /// Whether this nature is the across or the through member of its pair.
    pub kind: NatureKind,
}

/// The kind of a net — conservative or discrete. There is no third option
/// and no catch-all arm: every op that touches a net (T5) matches exactly
/// one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetKind {
    /// A conservative node: KCL applies. Carries TWO named natures —
    /// `across` is what is measured between this node and another,
    /// `through` is what is summed to zero at this node. The pair is the
    /// discipline's business (Electrical=`voltage`/`current`,
    /// Thermal=`temp`/`pwr`, Magnetic=`mmf`/`phi`, …); the IR sees only
    /// the ids and never the names.
    Node {
        across: NatureId,
        through: NatureId,
    },
    /// A discrete signal: carries a value, no conservation law, no KCL row.
    /// `ty` is the value's shape — `Bits(N)` for digital logic, `Real` for
    /// a signal-flow (HD) analogue of an analog value.
    Signal { ty: Ty },
}

impl NetKind {
    /// `true` for the conservative kind (carries two natures, KCL applies).
    pub fn is_node(&self) -> bool {
        matches!(self, NetKind::Node { .. })
    }

    /// `true` for the discrete kind (no KCL row by construction).
    pub fn is_signal(&self) -> bool {
        matches!(self, NetKind::Signal { .. })
    }
}

/// A declared net. The name field is added in a later task when `Name`
/// lands; the IR storage is the [`NetKind`] plus whatever metadata every
/// kind shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    /// What kind of net this is — the variant decides which ops may touch
    /// it (an analog stamp on a `Signal` is the A0b bug, made unrepresentable).
    pub kind: NetKind,
}

impl Net {
    /// A conservative node between the two named natures of a discipline.
    pub fn node(across: NatureId, through: NatureId) -> Self {
        Self { kind: NetKind::Node { across, through } }
    }

    /// A discrete signal carrying a value of type `ty`.
    pub fn signal(ty: Ty) -> Self {
        Self { kind: NetKind::Signal { ty } }
    }

    /// The kind, by value (the struct is small and the kind is the data).
    pub fn kind(&self) -> &NetKind {
        &self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Module;

    /// The load-bearing test for nature-generality (LOV-13/LOV-14, the
    /// "V/I would break thermal" trap): two disciplines whose across and
    /// through names are entirely different construct IDENTICALLY through
    /// the same API. If the IR hardcoded `V`/`I` anywhere, the thermal
    /// discipline would either fail to build or carry the wrong names.
    #[test]
    fn thermal_and_electrical_disciplines_construct_identically() {
        let mut m = Module::new();

        // Register four natures: two for Electrical, two for Thermal.
        // The names prove the IR carries them WITHOUT hardcoding any.
        let electrical_voltage = m.push_nature(NatureInfo {
            access: "voltage".into(),
            kind: NatureKind::Across,
        });
        let electrical_current = m.push_nature(NatureInfo {
            access: "current".into(),
            kind: NatureKind::Through,
        });
        let thermal_temp = m.push_nature(NatureInfo {
            access: "temp".into(),
            kind: NatureKind::Across,
        });
        let thermal_pwr = m.push_nature(NatureInfo {
            access: "pwr".into(),
            kind: NatureKind::Through,
        });

        // Both disciplines flow through the SAME constructor — `Net::node`
        // takes two NatureIds, never any name.
        let electrical_net = m.push_net(Net::node(electrical_voltage, electrical_current));
        let thermal_net = m.push_net(Net::node(thermal_temp, thermal_pwr));

        // Both are conservative nodes with their respective (across, through)
        // pairs preserved — the structure does not care that one is "volts
        // over amps" and the other "degrees over watts".
        let ek = m.net_at(electrical_net).kind();
        let tk = m.net_at(thermal_net).kind();
        assert_eq!(ek, &NetKind::Node { across: electrical_voltage, through: electrical_current });
        assert_eq!(tk, &NetKind::Node { across: thermal_temp, through: thermal_pwr });

        // The nature table is name-driven (a discipline's own naming), and
        // the IR looks it up by id only — there is no electrical-specific
        // code path.
        assert_eq!(m.nature_at(electrical_voltage).access, "voltage");
        assert_eq!(m.nature_at(electrical_current).access, "current");
        assert_eq!(m.nature_at(thermal_temp).access, "temp");
        assert_eq!(m.nature_at(thermal_pwr).access, "pwr");
    }

    #[test]
    fn a_node_carries_two_natures_a_signal_carries_a_type() {
        let mut m = Module::new();
        let n = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
        let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
        let node = m.push_net(Net::node(n, i));
        let sig = m.push_net(Net::signal(Ty::Bits(8)));

        assert!(m.net_at(node).kind().is_node());
        assert!(!m.net_at(node).kind().is_signal());
        assert!(m.net_at(sig).kind().is_signal());
        assert!(!m.net_at(sig).kind().is_node());
    }

    #[test]
    fn a_signal_carries_no_conservation_law_and_no_kcl_row_by_construction() {
        // The A0b mechanism: a Signal CANNOT be handed to analog stamping,
        // because there is no `across`/`through` pair to read or contribute.
        // The variant does not have those fields — the bug is unrepresentable.
        let mut m = Module::new();
        let sig_id = m.push_net(Net::signal(Ty::Bits(1)));
        match m.net_at(sig_id).kind() {
            NetKind::Signal { ty } => assert_eq!(ty, &Ty::Bits(1)),
            NetKind::Node { .. } => panic!("a Signal is not a Node"),
        }
    }

    #[test]
    fn nature_ids_are_distinct_per_nature_and_round_trip_through_the_arena() {
        let mut m = Module::new();
        let a = m.push_nature(NatureInfo { access: "x".into(), kind: NatureKind::Across });
        let b = m.push_nature(NatureInfo { access: "y".into(), kind: NatureKind::Through });
        assert_ne!(a, b);
        assert_eq!(m.nature_at(a).access, "x");
        assert_eq!(m.nature_at(b).access, "y");
    }
}
