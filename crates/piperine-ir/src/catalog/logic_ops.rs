//! The logic-encoding family (ideal.md §1.6, the table row "logic
//! encoding"): the 4-state ops over `bits<N>` — the (value, mask) word
//! pair (D3). Classic Verilog-simulator form: `and`/`or`/`xor` are
//! ordinary integer ops on the value word plus two mask ops.
//!
//! **Move, do not rewrite** (T11). The 4-state value is
//! [`crate::logic::Logic`]; the edge predicate is
//! [`crate::logic::EdgeDirection`]. The catalog entries DECLARE the
//! surface — name, signature (bits in / bits out), derivative rule (not
//! differentiable — digital). The implementations stay in `logic.rs`;
//! the catalog points at them by name.
//!
//! ## D3 — the (value, mask) encoding
//!
//! The 4-state encoding is `(value, mask)` — `value` is the bit pattern
//! (0 or 1 for known bits, 0 for X/Z bits), `mask` is 1 where the bit
//! is X or Z, 0 where it is known. The encoding permits a future pass
//! that spots an always-zero mask and drops to one word (most designs
//! are 2-state in practice). The catalog signature reports `Bits(N)`
//! for both arguments and return — the (value, mask) pair is a runtime
//! representation concern, not a type-level one.

use super::{DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::Ty;

/// One logic op entry. Signature per-name (binary/unary).
pub struct LogicOp {
    pub name: &'static str,
}

impl Intrinsic for LogicOp {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::Logic
    }
    fn signature(&self) -> Signature {
        match self.name {
            "bit-and" | "bit-or" | "bit-xor" => {
                Signature { args: vec![Ty::Bits(0), Ty::Bits(0)], ret: Ty::Bits(0) }
            }
            "bit-not" | "reduce-and" | "reduce-or" | "reduce-xor" => {
                Signature { args: vec![Ty::Bits(0)], ret: Ty::Bits(0) }
            }
            _ => unreachable!("unknown logic-op name: {}", self.name),
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
        LegalDomains::Digital
    }
}

pub const BIT_AND: LogicOp = LogicOp { name: "bit-and" };
pub const BIT_OR: LogicOp = LogicOp { name: "bit-or" };
pub const BIT_XOR: LogicOp = LogicOp { name: "bit-xor" };
pub const BIT_NOT: LogicOp = LogicOp { name: "bit-not" };
pub const REDUCE_AND: LogicOp = LogicOp { name: "reduce-and" };
pub const REDUCE_OR: LogicOp = LogicOp { name: "reduce-or" };
pub const REDUCE_XOR: LogicOp = LogicOp { name: "reduce-xor" };

/// All logic-encoding entries.
pub const ALL: &[&dyn Intrinsic] = &[
    &BIT_AND, &BIT_OR, &BIT_XOR, &BIT_NOT, &REDUCE_AND, &REDUCE_OR, &REDUCE_XOR,
];
