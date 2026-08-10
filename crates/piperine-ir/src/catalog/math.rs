//! The math family — the catalog's first citizens (ideal.md §1.6).
//! Stateless, real→real (or real×real→real for the binary ones),
//! differentiable by a named closed-form rule the diff pass (T12)
//! consults.
//!
//! Implementations live where they always have — there is no
//! `piperine-ir::math` module because Rust's `f64` math is the
//! implementation, and the JIT backends call straight into `libm`.
//! The catalog entry's job is to DECLARE the surface (name, signature,
//! derivative rule); the implementation is the platform math library.

use super::{DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::Ty;

/// One math intrinsic. Stateless real-valued, with a closed-form
/// derivative the diff pass knows by name.
pub struct MathEntry {
    pub name: &'static str,
    pub derivative: &'static str,
}

impl Intrinsic for MathEntry {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::Math
    }
    fn signature(&self) -> Signature {
        match self.name {
            "pow" | "min" | "max" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            _ => Signature { args: vec![Ty::Real], ret: Ty::Real },
        }
    }
    fn state_shape(&self) -> StateShape {
        StateShape::None
    }
    fn derivative(&self) -> DerivativeRule {
        match self.name {
            "pow" => DerivativeRule::BinaryPow,
            "abs" | "min" | "max" | "floor" => DerivativeRule::FiniteDifferenceOnly,
            _ => DerivativeRule::ClosedForm(self.derivative),
        }
    }
    fn breakpoints(&self) -> Vec<super::Breakpoint> {
        Vec::new()
    }
    fn domains(&self) -> LegalDomains {
        LegalDomains::Both
    }
}

// ── the data table (table + trait, never a macro — MD-13 rule 5) ──────────

pub const SIN: MathEntry = MathEntry { name: "sin", derivative: "chain:cos" };
pub const COS: MathEntry = MathEntry { name: "cos", derivative: "chain:neg-sin" };
pub const TAN: MathEntry = MathEntry { name: "tan", derivative: "sec2-chain" };
pub const ASIN: MathEntry = MathEntry { name: "asin", derivative: "reciprocal-sqrt-1-minus-x2" };
pub const ACOS: MathEntry = MathEntry { name: "acos", derivative: "neg-reciprocal-sqrt-1-minus-x2" };
pub const ATAN: MathEntry = MathEntry { name: "atan", derivative: "reciprocal-1-plus-x2" };
pub const SINH: MathEntry = MathEntry { name: "sinh", derivative: "chain:cosh" };
pub const COSH: MathEntry = MathEntry { name: "cosh", derivative: "chain:sinh" };
pub const TANH: MathEntry = MathEntry { name: "tanh", derivative: "sech2-chain" };
pub const EXP: MathEntry = MathEntry { name: "exp", derivative: "chain:self" };
pub const LIMEXP: MathEntry = MathEntry { name: "limexp", derivative: "chain:self" };
pub const LN: MathEntry = MathEntry { name: "ln", derivative: "reciprocal" };
pub const LOG: MathEntry = MathEntry { name: "log", derivative: "reciprocal" };
pub const LOG10: MathEntry = MathEntry { name: "log10", derivative: "reciprocal-ln10" };
pub const SQRT: MathEntry = MathEntry { name: "sqrt", derivative: "half-reciprocal-sqrt" };
pub const POW: MathEntry = MathEntry { name: "pow", derivative: "binary-pow" };
pub const ABS: MathEntry = MathEntry { name: "abs", derivative: "signum-chain" };
pub const MIN: MathEntry = MathEntry { name: "min", derivative: "select-le" };
pub const MAX: MathEntry = MathEntry { name: "max", derivative: "select-ge" };
pub const FLOOR: MathEntry = MathEntry { name: "floor", derivative: "zero" };

/// All math entries as trait objects, in stable declaration order.
pub const ALL: &[&dyn Intrinsic] = &[
    &SIN, &COS, &TAN, &ASIN, &ACOS, &ATAN, &SINH, &COSH, &TANH,
    &EXP, &LIMEXP, &LN, &LOG, &LOG10, &SQRT, &POW, &ABS, &MIN, &MAX, &FLOOR,
];
