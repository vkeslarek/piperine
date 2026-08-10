//! The detector family (ideal.md §1.6, the table row "detectors"):
//! `cross`, `above`, `timer`. Event detectors — analog-domain, with
//! private state and declared breakpoints.
//!
//! **Move, do not rewrite** (T11). The implementation is
//! [`crate::events::EventDetector`] + [`crate::events::Trigger`]; the
//! catalog entries DECLARE the surface. The seeding asymmetry
//! (`unseeded cross never fires; unseeded above does`) lives in the
//! family — the catalog does not restate it, just points at the family
//! by name.

use super::{Breakpoint, DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::{CrossDir, Ty, ValueId};

/// One detector entry. The signature/breakpoint kind is per-name.
pub struct Detector {
    pub name: &'static str,
}

impl Intrinsic for Detector {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::Detector
    }
    fn signature(&self) -> Signature {
        match self.name {
            "cross" => Signature { args: vec![Ty::Real], ret: Ty::Real },
            "above" => Signature { args: vec![Ty::Real], ret: Ty::Real },
            "timer" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            _ => unreachable!("unknown detector name: {}", self.name),
        }
    }
    fn state_shape(&self) -> StateShape {
        StateShape::EventDetector
    }
    fn derivative(&self) -> DerivativeRule {
        DerivativeRule::NotDifferentiable
    }
    fn breakpoints(&self) -> Vec<Breakpoint> {
        match self.name {
            "cross" => vec![Breakpoint::Crossing { expr: ValueId(0), dir: CrossDir::Either }],
            "timer" => vec![Breakpoint::TimerPeriod { period: ValueId(0), phase: ValueId(0) }],
            "above" => Vec::new(),
            _ => unreachable!("unknown detector name: {}", self.name),
        }
    }
    fn domains(&self) -> LegalDomains {
        LegalDomains::Analog
    }
}

pub const CROSS: Detector = Detector { name: "cross" };
pub const ABOVE: Detector = Detector { name: "above" };
pub const TIMER: Detector = Detector { name: "timer" };

/// All detector entries as trait objects.
pub const ALL: &[&dyn Intrinsic] = &[&CROSS, &ABOVE, &TIMER];
