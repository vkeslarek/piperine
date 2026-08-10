//! The noise family (ideal.md §1.6, the table row "noise"):
//! `white_noise`/`flicker_noise`. Analog-domain, stateless per-call
//! (the noise source's PSD is a parameter; the runtime noise generator
//! holds the RNG state outside the IR).
//!
//! **Not differentiable** — noise sources contribute to the noise
//! analysis, not the residual or Jacobian. The catalog declares this
//! with [`DerivativeRule::NotDifferentiable`].

use super::{DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::Ty;

/// One noise source entry. Signature per-name.
pub struct Noise {
    pub name: &'static str,
}

impl Intrinsic for Noise {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::Noise
    }
    fn signature(&self) -> Signature {
        match self.name {
            "white_noise" => Signature { args: vec![Ty::Real], ret: Ty::Real },
            "flicker_noise" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            _ => unreachable!("unknown noise name: {}", self.name),
        }
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
        LegalDomains::Analog
    }
}

pub const WHITE_NOISE: Noise = Noise { name: "white_noise" };
pub const FLICKER_NOISE: Noise = Noise { name: "flicker_noise" };

/// All noise entries.
pub const ALL: &[&dyn Intrinsic] = &[&WHITE_NOISE, &FLICKER_NOISE];
