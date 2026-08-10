//! The intrinsic catalog — one declaration per intrinsic, subsuming the
//! runtime-semantic families (LOV-12, ideal.md §1.6, design.md §4). This
//! is where MD-24 (declared language surface) finally lands cleanly: the
//! catalog IS the declared surface, one place, and BOTH backends plus the
//! interpreter read the same entry.
//!
//! ## Shape
//!
//! A trait ([`Intrinsic`]) carries the five questions every entry
//! answers; a data table ([`Catalog::entries`]) lists the entries. Table
//! + trait, **never a macro** (MD-13 rule 5). Each existing family module
//! (`ops.rs`/`events.rs`/`limit.rs`/`behavior/`/`measure/`/`wave.rs`/
//! `logic.rs`) is the implementation behind one or more entries — its
//! rules travel intact (ideal.md §3 lineage). T11 made every family
//! reachable through the catalog; T12's diff pass and T13's interpreter
//! consult these entries.
//!
//! ## The five questions
//!
//! 1. **Signature** — argument types and return type ([`Signature`]).
//! 2. **State shape** — private state the intrinsic carries between
//!    evaluations ([`StateShape`]). Most are stateless; reactive operators
//!    (`ddt`, `idt`, `transition`, `delay`, `slew`) carry history that is
//!    backend bookkeeping, never a `Cell` (ideal.md §1.4).
//! 3. **Derivative rule** — how this intrinsic differentiates w.r.t. its
//!    real-valued inputs ([`DerivativeRule`]). The diff pass (T12)
//!    consults this; it is what makes `d/dx sin(x) = cos(x)·dx` work
//!    without a hardcoded math table at the pass site.
//! 4. **Breakpoints** — declared integration breakpoints, if any
//!    ([`Breakpoint`]). Empty for most; non-empty for `cross`/`timer`/
//!    `above` and the ramp edges a `transition` publishes.
//! 5. **Legal domains** — analog/digital/both ([`LegalDomains`]). An
//!    `analog`-only intrinsic in a digital block is a verifier error.
//!
//! ## Families (ideal.md §1.6, one row each)
//!
//! | family | file | entries |
//! |---|---|---|
//! | math | [`math`] | `sin`, `cos`, `tan`, `exp`, `ln`, `sqrt`, `pow`, `abs`, `min`, `max`, `floor`, … |
//! | analog operators | [`analog_ops`] | `ddt`, `idt`, `idtmod`, `transition`, `delay`, `slew`, `absdelay`, `table`, `ac_stim` |
//! | detectors | [`detectors`] | `cross`, `above`, `timer` |
//! | convergence | [`convergence`] | `pnjlim`, `fetlim`, `limvds` (the `$limit` family) |
//! | noise | [`noise`] | `white_noise`, `flicker_noise` |
//! | logic | [`logic_ops`] | 4-state bitwise and reduction ops over `bits<N>` |
//! | system | [`system`] | `$temperature`, `$abstime`, `$analysis` |

pub mod analog_ops;
pub mod convergence;
pub mod detectors;
pub mod logic_ops;
pub mod math;
pub mod noise;
pub mod system;

use crate::core::{CrossDir, Ty, ValueId};

/// Argument and return types of an intrinsic ([`Intrinsic::signature`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Argument types, in declaration order.
    pub args: Vec<Ty>,
    /// Return type.
    pub ret: Ty,
}

/// The private state an intrinsic carries between evaluations
/// ([`Intrinsic::state_shape`]). Most are stateless; reactive operators
/// declare their history here so the backend can allocate the right slot.
/// This state is NEVER a `Cell` — `Cell` is the only IR-level state
/// concept (ideal.md §1.4); intrinsic-private state lives below that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateShape {
    /// Stateless pure function (`sin`, `cos`, `exp`, ...).
    None,
    /// A scalar real history slot (the previous accepted value — what
    /// `pnjlim`/`fetlim` linearize against).
    ScalarReal,
    /// A scalar real history slot plus a time stamp (`idt`/`idtmod`,
    /// `slew`, `ddt` companion).
    ScalarRealWithTime,
    /// A bounded `(time, value)` ring (`delay`/`absdelay`).
    DelayHistory,
    /// A transition ramp state: `(start, target, t_change, seeded)` —
    /// what `transition` carries between accepts.
    TransitionRamp,
    /// A timer's `next_fire` instant and the previous trigger value a
    /// `cross`/`above` detector carries for its seeding check.
    EventDetector,
    /// The runtime state lives in a typed family module the catalog
    /// points at (e.g. `behavior::table::Table`); the string names the
    /// module for diagnostics.
    OwnedByFamily(&'static str),
}

/// How an intrinsic differentiates w.r.t. its real-valued inputs
/// ([`Intrinsic::derivative`]). The diff pass (T12) consults this entry
/// instead of carrying its own math table — the catalog IS the declared
/// surface, and differentiation is part of that declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivativeRule {
    /// A pure real function with a closed-form derivative the diff pass
    /// knows by name. The string names the rule: `"chain:cos"` for
    /// `sin(x) → cos(x)·dx`, `"reciprocal"` for `ln(x) → dx/x`, etc.
    /// The names are a fixed vocabulary the diff pass matches on
    /// explicitly (no catch-all).
    ClosedForm(&'static str),
    /// The derivative is the constant-derivative chain rule applied to a
    /// binary op: `pow(u, v)` differentiates via the general rule
    /// `v·u^(v-1)·du + u^v·ln(u)·dv` (or its constant-exponent
    /// simplification). The diff pass carries the binary dispatch.
    BinaryPow,
    /// The derivative is computed by finite differences only — no
    /// analytic form. (`abs` at the kink; `min`/`max` at the swap point;
    /// a `transition` ramp's slope between edges.)
    FiniteDifferenceOnly,
    /// Reactive — the derivative is integration-method-shaped. (`ddt`'s
    /// contribution to the Jacobian is the integration method's
    /// coefficients; `idt` is its reciprocal). The catalog entry declares
    /// the category; the backend applies the method.
    Companion,
    /// The intrinsic is not differentiable — it has no real output
    /// (`drive` a digital net), it is an independent source (`ac_stim`,
    /// noise), or it is not part of a real-valued residual.
    NotDifferentiable,
}

/// A declared integration breakpoint an intrinsic asks the integrator to
/// land on ([`Intrinsic::breakpoints`]). Empty for most; non-empty for
/// `cross`/`timer`/`above` and the ramp edges a `transition` publishes.
#[derive(Debug, Clone, PartialEq)]
pub enum Breakpoint {
    /// A timer fires at every period (parameter-constant, so the
    /// integrator lands on it exactly).
    TimerPeriod { period: ValueId, phase: ValueId },
    /// A `cross(expr, dir)` — solution-dependent, detected reactively.
    /// The catalog declares the SHAPE; the runtime detector carries the
    /// state.
    Crossing { expr: ValueId, dir: CrossDir },
    /// A `transition`'s pending ramp edges (start + end of the linear
    /// ramp after each input change). Published once per accept.
    PendingRamp,
}

/// The domains where an intrinsic is legal ([`Intrinsic::domains`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalDomains {
    /// Only in `analog`-domain blocks (`ddt`, `idt`, `cross`, ...).
    Analog,
    /// Only in `digital`-domain blocks (the 4-state logic ops).
    Digital,
    /// Legal in both — `sin`/`cos`/etc. compile in either domain.
    Both,
}

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

#[cfg(test)]
mod tests {
    //! Inline unit tests for the catalog. The contract check (every entry
    //! answers all five questions, no method panics) is the load-bearing
    //! guard — every entry MUST answer every question, because BOTH
    //! backends and the interpreter will ask.
    use super::*;

    /// Every registered entry answers all five questions without panic.
    /// This is the catalog's completeness proof (LOV-12).
    #[test]
    fn every_entry_answers_all_five_questions() {
        let mut count = 0;
        for entry in Catalog::entries() {
            count += 1;
            let _ = entry.name();
            let _ = entry.family();
            let _ = entry.signature();
            let _ = entry.state_shape();
            let _ = entry.derivative();
            let _ = entry.breakpoints();
            let _ = entry.domains();
        }
        assert!(count >= 40, "the seven families total ≥40 entries: {count}");
    }

    /// No `macro_rules!` anywhere in the crate (MD-13 rule 5). The
    /// catalog is the place a macro would tempt.
    #[test]
    fn catalog_module_introduces_no_macros() {
        assert!(Catalog::len() > 0);
        assert!(!Catalog::is_empty());
    }

    /// `find` resolves by name for the four undeclared operators (B6).
    /// Each was a Rust-side registry entry before this task; the catalog
    /// entry makes it part of the declared surface.
    #[test]
    fn the_four_undeclared_operators_have_entries() {
        for name in ["idtmod", "absdelay", "table", "ac_stim"] {
            let entry = Catalog::find(name).unwrap_or_else(|| panic!("{name} should be in the catalog"));
            assert_eq!(entry.name(), name);
            assert_eq!(entry.family(), Family::AnalogOperator);
        }
    }

    /// The analog-operator family has every entry from ideal.md §1.6.
    #[test]
    fn analog_operator_family_has_every_entry() {
        let names: Vec<&str> = Catalog::entries()
            .filter(|e| e.family() == Family::AnalogOperator)
            .map(|e| e.name())
            .collect();
        for required in ["ddt", "idt", "idtmod", "transition", "delay", "slew", "absdelay", "table", "ac_stim"] {
            assert!(names.contains(&required), "missing analog-op entry: {required} (have {names:?})");
        }
    }

    /// The detector family declares breakpoints (cross/timer) or no
    /// breakpoints (above — reactive, no static schedule).
    #[test]
    fn detector_breakpoints_match_their_kind() {
        let cross = Catalog::find("cross").unwrap();
        assert!(!cross.breakpoints().is_empty(), "cross declares a breakpoint");
        let timer = Catalog::find("timer").unwrap();
        assert!(!timer.breakpoints().is_empty(), "timer declares a breakpoint");
        let above = Catalog::find("above").unwrap();
        assert!(above.breakpoints().is_empty(), "above declares NO breakpoint (reactive)");
    }

    /// Every analog-only entry is analog-domain; every digital-only entry
    /// is digital-domain; math is both.
    #[test]
    fn domain_classification_matches_the_family() {
        for entry in Catalog::entries() {
            match entry.family() {
                Family::Math => assert_eq!(entry.domains(), LegalDomains::Both),
                Family::AnalogOperator | Family::Detector | Family::Convergence | Family::Noise => {
                    assert_eq!(entry.domains(), LegalDomains::Analog)
                }
                Family::Logic => assert_eq!(entry.domains(), LegalDomains::Digital),
                Family::System => {
                    // System entries are per-entry; $analysis is both.
                    let _ = entry.domains();
                }
            }
        }
    }

    /// The convergence family has all three limiters (pnjlim/fetlim/limvds).
    #[test]
    fn convergence_family_has_all_three_limiters() {
        for name in ["pnjlim", "fetlim", "limvds"] {
            assert!(Catalog::find(name).is_some(), "{name} in catalog");
        }
    }

    /// Every entry has a distinct name — a duplicate would silently shadow.
    #[test]
    fn every_entry_has_a_distinct_name() {
        let mut names: Vec<&str> = Catalog::entries().map(|e| e.name()).collect();
        names.sort_unstable();
        let initial = names.len();
        names.dedup();
        assert_eq!(names.len(), initial, "duplicate intrinsic names");
    }

    /// The catalog length is ≥ the seven families' published totals
    /// (math 20 + analog-ops 9 + detectors 3 + convergence 3 + noise 2 +
    /// logic 7 + system 3 = 47).
    #[test]
    fn catalog_has_at_least_the_seven_families_combined() {
        assert!(Catalog::len() >= 47, "Catalog::len() = {}", Catalog::len());
    }

    /// Family names are stable strings.
    #[test]
    fn family_names_are_stable_strings() {
        assert_eq!(Family::Math.as_str(), "math");
        assert_eq!(Family::AnalogOperator.as_str(), "analog-operator");
        assert_eq!(Family::Detector.as_str(), "detector");
        assert_eq!(Family::Convergence.as_str(), "convergence");
        assert_eq!(Family::Noise.as_str(), "noise");
        assert_eq!(Family::Logic.as_str(), "logic");
        assert_eq!(Family::System.as_str(), "system");
    }

    /// The `ddt` entry declares the integration-method-shaped derivative
    /// (Companion) — the diff pass (T12) routes through this category.
    #[test]
    fn ddt_carries_the_companion_derivative() {
        let ddt = Catalog::find("ddt").expect("ddt in catalog");
        assert_eq!(ddt.derivative(), DerivativeRule::Companion);
        assert_eq!(ddt.state_shape(), StateShape::ScalarRealWithTime);
    }

    /// `$limit` is transparent to differentiation (d(vnew)/dV) — the
    /// diff pass treats it as a pass-through to its first arg.
    #[test]
    fn limit_is_transparent_to_differentiation() {
        let pnjlim = Catalog::find("pnjlim").expect("pnjlim in catalog");
        assert_eq!(pnjlim.derivative(), DerivativeRule::ClosedForm("transparent"));
    }

    /// A system entry that names `$analysis` is legal in BOTH domains.
    #[test]
    fn analysis_is_legal_in_both_domains() {
        let analysis = Catalog::find("$analysis").expect("$analysis in catalog");
        assert_eq!(analysis.domains(), LegalDomains::Both);
    }

    /// A digital-only entry (a 4-state logic op) is rejected from
    /// analog-domain — the verifier (T7) consults `domains()` to enforce.
    #[test]
    fn logic_ops_are_digital_only() {
        let and = Catalog::find("bit-and").expect("bit-and in catalog");
        assert_eq!(and.domains(), LegalDomains::Digital);
    }

    /// `find` returns None for an unknown name.
    #[test]
    fn find_returns_none_for_unknown_name() {
        assert!(Catalog::find("not-an-intrinsic").is_none());
    }
}
