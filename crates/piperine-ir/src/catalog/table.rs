//! The catalog table — one [`Intrinsic`] per entry, the declared
//! language surface both backends and the interpreter read (MD-24). The
//! five-question shape lives in [`super::types`]; the family modules
//! declared by [`super`] are the implementation behind one or more
//! entries.

use super::types::*;
use super::{analog_ops, convergence, detectors, logic_ops, math, noise, system};

/// The chapter of the runtime semantics an intrinsic belongs to
/// (ideal.md §1.6). One row per family in the catalog table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// `sin`, `cos`, `exp`, `ln`, `sqrt`, `pow`, `abs`, `min`, `max`, ...
    Math,
    /// `ddt`, `idt`, `idtmod`, `transition`, `delay`, `slew`, `absdelay`,
    /// `table`, `ac_stim`.
    AnalogOperator,
    /// `cross`, `above`, `timer`.
    Detector,
    /// `$limit` / pnjlim / fetlim / limvds.
    Convergence,
    /// `white_noise`, `flicker_noise`.
    Noise,
    /// 4-state logic ops over `bits<N>` (the (value, mask) word pair, D3).
    Logic,
    /// `$temperature`, `$abstime`, `$analysis`.
    System,
}

impl Family {
    /// The display name of the family.
    pub fn as_str(self) -> &'static str {
        match self {
            Family::Math => "math",
            Family::AnalogOperator => "analog-operator",
            Family::Detector => "detector",
            Family::Convergence => "convergence",
            Family::Noise => "noise",
            Family::Logic => "logic",
            Family::System => "system",
        }
    }
}

/// One declared intrinsic. The contract every backend and the interpreter
/// read; the implementation behind it lives in a family module. Methods
/// only (no free functions — MD-13 rule 2). The trait is `Sync` because a
/// catalog entry is a static singleton.
pub trait Intrinsic: Sync {
    /// The intrinsic's surface name — `"sin"`, `"ddt"`, `"cross"`, ...
    /// Matches the spelling PHDL uses and the printer (T8) emits.
    fn name(&self) -> &'static str;

    /// The family chapter (ideal.md §1.6).
    fn family(&self) -> Family;

    /// Argument and return types.
    fn signature(&self) -> Signature;

    /// Private state shape (the backend allocates the slot).
    fn state_shape(&self) -> StateShape;

    /// How this intrinsic differentiates w.r.t. its real-valued inputs.
    fn derivative(&self) -> DerivativeRule;

    /// Declared breakpoints, if any. Empty `Vec` for the majority.
    fn breakpoints(&self) -> Vec<Breakpoint>;

    /// Where this intrinsic is legal.
    fn domains(&self) -> LegalDomains;
}

/// The catalog — the declared language surface (MD-24). A data table of
/// entries, plus methods to enumerate and find. The owner of the table
/// is this struct; the table itself is a `&'static [&dyn Intrinsic]` so
/// lookups are by stable name.
pub struct Catalog;

impl Catalog {
    /// Iterate every registered entry, in declaration order. The order
    /// is fixed for stable printing and diagnostics: family-chapter
    /// order (ideal.md §1.6), entries within a family in the family
    /// file's `ALL` order.
    pub fn entries() -> impl Iterator<Item = &'static dyn Intrinsic> {
        // Concatenate the family tables in chapter order. `chain` is
        // stable and the families are static.
        math::ALL
            .iter()
            .chain(analog_ops::ALL.iter())
            .chain(detectors::ALL.iter())
            .chain(convergence::ALL.iter())
            .chain(noise::ALL.iter())
            .chain(logic_ops::ALL.iter())
            .chain(system::ALL.iter())
            .copied()
    }

    /// Find an entry by its surface name.
    pub fn find(name: &str) -> Option<&'static dyn Intrinsic> {
        Self::entries().find(|e| e.name() == name)
    }

    /// The total number of entries.
    pub fn len() -> usize {
        math::ALL.len()
            + analog_ops::ALL.len()
            + detectors::ALL.len()
            + convergence::ALL.len()
            + noise::ALL.len()
            + logic_ops::ALL.len()
            + system::ALL.len()
    }

    /// `false` if the catalog has any entries (it always has).
    pub fn is_empty() -> bool {
        Self::len() == 0
    }
}
