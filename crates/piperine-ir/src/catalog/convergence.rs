//! The convergence family (ideal.md §1.6, the table row "convergence"):
//! `$limit` — the junction limiter. Three flavors, each a separate
//! catalog entry:
//!
//! - `pnjlim(vnew, vold, vte, vcrit)` — the pn-junction limiter.
//! - `fetlim(vnew, vold, vto)` — the MOSFET gate-voltage limiter.
//! - `limvds(vnew, vold)` — the MOSFET drain-source limiter.
//!
//! **Move, do not rewrite** (T11). The implementations are the loose fns
//! [`crate::limit::pnjlim`]/[`crate::limit::fetlim`]/[`crate::limit::limvds`];
//! the catalog entries DECLARE the surface — name, signature, derivative
//! rule. The catalog keeps the loose fns reachable BY NAME (per the
//! task: "keep the function but ALSO expose it through the catalog
//! entry"). The five pnjlim rules (`vcrit` seed, seed-preservation,
//! limit-against-previous-limited, convergence-veto tolerance,
//! minus-node move) live in the family module unchanged — the catalog
//! entry does not restate them.
//!
//! Derivative-wise, `$limit` is transparent to differentiation
//! (`d/dV vnew = 1`; `vold` is constant within a Newton iteration) —
//! the source `crates/piperine-codegen/src/resolve/diff.rs` treats
//! `$limit` as a pass-through to its `vnew` argument. The catalog
//! declares this with [`DerivativeRule::ClosedForm("transparent")`].

use super::{DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::Ty;

/// One limiter entry. The signature is per-name.
pub struct Limiter {
    pub name: &'static str,
}

impl Intrinsic for Limiter {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::Convergence
    }
    fn signature(&self) -> Signature {
        match self.name {
            "pnjlim" => Signature { args: vec![Ty::Real, Ty::Real, Ty::Real, Ty::Real], ret: Ty::Real },
            "fetlim" => Signature { args: vec![Ty::Real, Ty::Real, Ty::Real], ret: Ty::Real },
            "limvds" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            _ => unreachable!("unknown limiter name: {}", self.name),
        }
    }
    fn state_shape(&self) -> StateShape {
        // The previous *limited* value (rule 3 — limit against the
        // previous limited). The runtime bank is the `Limiter` struct
        // in `crate::limit`; the catalog declares the slot shape.
        StateShape::ScalarReal
    }
    fn derivative(&self) -> DerivativeRule {
        // Transparent: `d(vnew)/dV`. The diff pass (T12) consults this
        // and treats the limiter as a pass-through to its first arg.
        DerivativeRule::ClosedForm("transparent")
    }
    fn breakpoints(&self) -> Vec<super::Breakpoint> {
        Vec::new()
    }
    fn domains(&self) -> LegalDomains {
        LegalDomains::Analog
    }
}

pub const PNJLIM: Limiter = Limiter { name: "pnjlim" };
pub const FETLIM: Limiter = Limiter { name: "fetlim" };
pub const LIMVDS: Limiter = Limiter { name: "limvds" };

/// All convergence entries.
pub const ALL: &[&dyn Intrinsic] = &[&PNJLIM, &FETLIM, &LIMVDS];
