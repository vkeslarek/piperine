//! The questions the checker asks of a nature's kind (part II §4): how many drives a carrier of
//! it has, whether a `resolve` combines them, and whether its branches carry a flow.

use crate::pom::{DeclId, DeclKind, Design, NatureKind, QuantityRole};

/// What a nature's kind decides for the rules of `types/`: the one owner of "what kind of carrier
/// or node is this", so no rule matches on [`NatureKind`] itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NatureRules {
    /// A carrier of the nature has one drive (§5.2.1): `Bit<T>`, a project discrete nature, and
    /// the potential of a signal-flow node (§6 rule 2).
    pub one_driver: bool,
    /// The nature declares `resolve`: its drives combine into one value (§4 rule 8).
    pub resolved: bool,
    /// The nature declares a `flow`: a conservative node, which a current may be contributed to.
    pub flow: bool,
    /// A carrier of the nature holds a discrete value (`Bit<T>`, a project nature, a resolved
    /// nature); the other kinds are nodes of the analog solver.
    pub carries_values: bool,
}

impl NatureRules {
    /// The rules of `nature`; a declaration that is no nature (or an invalid one) has none of the
    /// three, so no rule that asks is satisfied by it.
    pub(super) fn of(design: &Design, nature: DeclId) -> NatureRules {
        let kind = match &design.decl_data(nature).kind {
            DeclKind::Nature(data) => data.kind,
            _ => NatureKind::Invalid,
        };
        match kind {
            NatureKind::Discrete => NatureRules { one_driver: true, resolved: false, flow: false, carries_values: true },
            NatureKind::SignalFlow => NatureRules { one_driver: true, resolved: false, flow: false, carries_values: false },
            NatureKind::Resolved => NatureRules { one_driver: false, resolved: true, flow: false, carries_values: true },
            NatureKind::Conservative => NatureRules { one_driver: false, resolved: false, flow: true, carries_values: false },
            NatureKind::Invalid => NatureRules { one_driver: false, resolved: false, flow: false, carries_values: false },
        }
    }

    /// Whether two continuous natures are compatible and so connect (§4 rule 13): the same
    /// nature, or potential quantities of one name and, when both declare `flow`, flow
    /// quantities of one name. A signal-flow `VSignal` meets an `Electrical` port this way.
    pub(super) fn compatible(design: &Design, left: DeclId, right: DeclId) -> bool {
        if left == right {
            return true;
        }
        let quantity = |nature: DeclId, role: QuantityRole| {
            design.decl_data(nature).children.iter().find_map(|c| match &design.decl_data(*c).kind {
                DeclKind::Quantity(q) if q.role == role => Some(design.decl_data(*c).name.as_ref().map_or_else(String::new, |n| n.text.clone())),
                _ => None,
            })
        };
        let potentials = quantity(left, QuantityRole::Potential);
        let flows = (quantity(left, QuantityRole::Flow), quantity(right, QuantityRole::Flow));
        potentials.is_some() && potentials == quantity(right, QuantityRole::Potential) && (flows.0.is_none() || flows.1.is_none() || flows.0 == flows.1)
    }
}
