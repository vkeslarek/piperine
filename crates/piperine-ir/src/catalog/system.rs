//! The system family (ideal.md §1.6, the table row "system"):
//! `$temperature`, `$abstime`, `$analysis`. Analog-domain (they are
//! read inside analog residuals), but `$analysis` is also legal in
//! digital blocks (a digital block branches on the analysis kind just
//! as `analog` does).
//!
//! None differentiable — `$temperature` is a runtime constant within a
//! Newton iteration, `$abstime` is the independent variable (its
//! "derivative" is 1 but the residual sees it as a constant), and
//! `$analysis` returns a discrete enum.

use super::{DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::Ty;

/// One system entry. Domain per-name.
pub struct System {
    pub name: &'static str,
}

impl Intrinsic for System {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::System
    }
    fn signature(&self) -> Signature {
        Signature { args: vec![], ret: Ty::Real }
    }
    fn state_shape(&self) -> StateShape {
        StateShape::None
    }
    fn derivative(&self) -> DerivativeRule {
        DerivativeRule::NotDifferentiable
    }
    fn breakpoints(&self) -> Vec<super::Breakpoint> {
        Vec::new()
    }
    fn domains(&self) -> LegalDomains {
        match self.name {
            "$analysis" => LegalDomains::Both,
            _ => LegalDomains::Analog,
        }
    }
}

pub const TEMPERATURE: System = System { name: "$temperature" };
pub const ABSTIME: System = System { name: "$abstime" };
pub const ANALYSIS: System = System { name: "$analysis" };

/// All system entries.
pub const ALL: &[&dyn Intrinsic] = &[&TEMPERATURE, &ABSTIME, &ANALYSIS];
