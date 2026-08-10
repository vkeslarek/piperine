//! The analog-operator family (ideal.md §1.6, the table row "analog
//! operators"): `ddt`, `idt`, `idtmod`, `transition`, `delay`, `slew`,
//! `absdelay`, `table`, `ac_stim`. Reactive with private state, all
//! analog-domain.
//!
//! **Move, do not rewrite** (T11, design.md §3 lineage). The
//! implementations live in [`crate::ops`] (`Operator` enum) and
//! [`crate::behavior::table`] (`Table`); the catalog entries DECLARE the
//! surface — name, signature, state shape, derivative rule, breakpoints,
//! legal domain. The interpreter (T13) and the JIT backends (Phase 4)
//! dispatch to the family implementations by name.
//!
//! ## Closing GAPS.md B6
//!
//! The four undeclared operators (`idtmod`, `absdelay`, `table`,
//! `ac_stim`) gain catalog entries here, closing the gap where they
//! resolved through a Rust-side registry the IR could not see. The
//! catalog IS the declared surface; `idtmod` is now reachable as
//! `Catalog::find("idtmod")`, etc.
//!
//! ## LaplaceKind / ZKind
//!
//! `LaplaceKind`/`ZKind` (defined in `piperine-codegen/src/resolve/
//! symbols.rs`) are vestigial: no live code path constructs them, and
//! they exist only in codegen's resolve stage — which Phase 6/7 (T34)
//! replaces wholesale. The IR catalog gives them formal ABSENCE: there
//! is no `laplace_*`/`zi_*` entry. The codegen copies will be deleted
//! when `resolve/` is replaced; modifying codegen now is out of scope
//! for this phase (the frontier is frozen).

use super::{Breakpoint, DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
use crate::core::Ty;

/// One analog operator entry. The catalog metadata is built per-name
/// at call time (the signature shape varies; the field carries only the
/// name and the entry is a static singleton).
pub struct AnalogOp {
    pub name: &'static str,
}

impl Intrinsic for AnalogOp {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::AnalogOperator
    }
    fn signature(&self) -> Signature {
        match self.name {
            "ddt" => Signature { args: vec![Ty::Real], ret: Ty::Real },
            "idt" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            "idtmod" => Signature { args: vec![Ty::Real, Ty::Real, Ty::Real], ret: Ty::Real },
            "transition" => Signature { args: vec![Ty::Real, Ty::Real, Ty::Real, Ty::Real], ret: Ty::Real },
            "delay" | "absdelay" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            "slew" => Signature { args: vec![Ty::Real, Ty::Real, Ty::Real], ret: Ty::Real },
            "table" => Signature { args: vec![Ty::Real], ret: Ty::Real },
            "ac_stim" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            _ => unreachable!("unknown analog-op name: {}", self.name),
        }
    }
    fn state_shape(&self) -> StateShape {
        match self.name {
            "ddt" | "idt" | "idtmod" | "slew" => StateShape::ScalarRealWithTime,
            "transition" => StateShape::TransitionRamp,
            "delay" | "absdelay" => StateShape::DelayHistory,
            "table" => StateShape::OwnedByFamily("behavior::table::Table"),
            "ac_stim" => StateShape::None,
            _ => unreachable!("unknown analog-op name: {}", self.name),
        }
    }
    fn derivative(&self) -> DerivativeRule {
        match self.name {
            "ddt" | "idt" | "idtmod" => DerivativeRule::Companion,
            "transition" | "slew" | "table" => DerivativeRule::FiniteDifferenceOnly,
            "delay" | "absdelay" | "ac_stim" => DerivativeRule::NotDifferentiable,
            _ => unreachable!("unknown analog-op name: {}", self.name),
        }
    }
    fn breakpoints(&self) -> Vec<Breakpoint> {
        match self.name {
            "transition" => vec![Breakpoint::PendingRamp],
            _ => Vec::new(),
        }
    }
    fn domains(&self) -> LegalDomains {
        LegalDomains::Analog
    }
}

pub const DDT: AnalogOp = AnalogOp { name: "ddt" };
pub const IDT: AnalogOp = AnalogOp { name: "idt" };
pub const IDTMOD: AnalogOp = AnalogOp { name: "idtmod" };
pub const TRANSITION: AnalogOp = AnalogOp { name: "transition" };
pub const DELAY: AnalogOp = AnalogOp { name: "delay" };
pub const ABSDELAY: AnalogOp = AnalogOp { name: "absdelay" };
pub const SLEW: AnalogOp = AnalogOp { name: "slew" };
pub const TABLE: AnalogOp = AnalogOp { name: "table" };
pub const AC_STIM: AnalogOp = AnalogOp { name: "ac_stim" };

/// All analog-operator entries as trait objects, in stable order.
pub const ALL: &[&dyn Intrinsic] = &[
    &DDT, &IDT, &IDTMOD, &TRANSITION, &DELAY, &ABSDELAY, &SLEW, &TABLE, &AC_STIM,
];
