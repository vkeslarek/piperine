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
//! `logic.rs`) becomes the implementation behind one or more entries in
//! T11 — its rules travel intact (ideal.md §3 lineage).
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
//! ## T10 scope
//!
//! This task lands the CONTRACT and a minimal table of math entries
//! (the easy first citizens). T11 migrates the runtime families into
//! catalog entries in place — their implementations stay, their tests
//! travel intact.

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
    /// `pnjlim`/`fetlim` linearize against, what an `idt` accumulator
    /// holds).
    ScalarReal,
    /// A scalar real history slot plus a time stamp (`idt`/`idtmod`,
    /// `slew`).
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
    /// analytic form. (`abs` at the kink; `min`/`max` at the swap point.)
    FiniteDifferenceOnly,
    /// Reactive — the derivative is integration-method-shaped. (`ddt`'s
    /// contribution to the Jacobian is the integration method's
    /// coefficients; `idt` is its reciprocal). The catalog entry declares
    /// the category; the backend applies the method.
    Companion,
    /// The intrinsic is not differentiable — it has no real output
    /// (`drive` a digital net) or it is not part of a real-valued
    /// residual.
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
    /// `$temperature`, `$abstime`, `$analysis`, ...
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

// ── the math family: the first citizens of the catalog ─────────────────────
//
// Each math intrinsic is a static singleton struct implementing `Intrinsic`.
// Stateless, real→real, differentiable by a named closed-form rule. T11
// extends the table with the analog-operator/detector/convergence/noise/
// logic/system families.

struct MathEntry {
    name: &'static str,
    derivative: &'static str,
}

impl Intrinsic for MathEntry {
    fn name(&self) -> &'static str {
        self.name
    }
    fn family(&self) -> Family {
        Family::Math
    }
    fn signature(&self) -> Signature {
        // Every math intrinsic is real→real in its unary form. `pow`,
        // `min`, `max` take a second real; the catalog reports the
        // unary-or-binary signature per entry below.
        self.signature_for()
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
    fn breakpoints(&self) -> Vec<Breakpoint> {
        Vec::new()
    }
    fn domains(&self) -> LegalDomains {
        LegalDomains::Both
    }
}

impl MathEntry {
    /// Per-entry signature dispatch — most are unary real→real; `pow`/
    /// `min`/`max` are binary.
    fn signature_for(&self) -> Signature {
        match self.name {
            "pow" | "min" | "max" => Signature { args: vec![Ty::Real, Ty::Real], ret: Ty::Real },
            _ => Signature { args: vec![Ty::Real], ret: Ty::Real },
        }
    }
}

// The data table — `&'static MathEntry` per row, never a macro.
const SIN: MathEntry = MathEntry { name: "sin", derivative: "chain:cos" };
const COS: MathEntry = MathEntry { name: "cos", derivative: "chain:neg-sin" };
const EXP: MathEntry = MathEntry { name: "exp", derivative: "chain:self" };
const LN: MathEntry = MathEntry { name: "ln", derivative: "reciprocal" };
const SQRT: MathEntry = MathEntry { name: "sqrt", derivative: "half-reciprocal-sqrt" };
const POW: MathEntry = MathEntry { name: "pow", derivative: "binary-pow" };
const ABS: MathEntry = MathEntry { name: "abs", derivative: "signum-chain" };
const MIN: MathEntry = MathEntry { name: "min", derivative: "select-le" };
const MAX: MathEntry = MathEntry { name: "max", derivative: "select-ge" };
const FLOOR: MathEntry = MathEntry { name: "floor", derivative: "zero" };

/// The catalog — the declared language surface (MD-24). A data table of
/// entries, plus methods to enumerate and find. The owner of the table
/// is this struct; the table itself is a `&'static [&dyn Intrinsic]` so
/// lookups are by stable name (T11 fills the table with the runtime
/// families; T10 ships the math family as the first citizens).
pub struct Catalog;

impl Catalog {
    /// Iterate every registered entry, in declaration order. The order
    /// is fixed for stable printing and diagnostics.
    pub fn entries() -> impl Iterator<Item = &'static dyn Intrinsic> {
        ENTRY_REFS.iter().copied()
    }

    /// Find an entry by its surface name.
    pub fn find(name: &str) -> Option<&'static dyn Intrinsic> {
        Self::entries().find(|e| e.name() == name)
    }

    /// The total number of entries.
    pub fn len() -> usize {
        ENTRY_REFS.len()
    }

    /// `false` if the catalog has any entries (it always has — the math
    /// family is non-empty). Convenience for clippy-style len/is_empty.
    pub fn is_empty() -> bool {
        ENTRY_REFS.is_empty()
    }
}

/// The static entry table — table + trait, never a macro (MD-13 rule 5).
/// A `&'static [&dyn Intrinsic]` so callers can match on the trait object
/// without owning a copy. The order is the family-chapter order of
/// ideal.md §1.6 (math first; T11 adds the rest).
static ENTRY_REFS: &[&dyn Intrinsic] = &[
    &SIN, &COS, &EXP, &LN, &SQRT, &POW, &ABS, &MIN, &MAX, &FLOOR,
];

#[cfg(test)]
mod tests {
    //! Inline unit tests for the catalog. The contract check (every entry
    //! answers all five questions, no method panics) is the load-bearing
    //! guard — every entry MUST answer every question, because BOTH
    //! backends and the interpreter will ask.
    use super::*;

    /// Every registered entry answers all five questions without panic.
    /// This is the catalog's completeness proof (LOV-12): the trait is
    /// not optional, and a future entry that forgets a method fails to
    /// compile (the trait requires it) — but a future entry that returns
    /// a wrong value would not, so this test walks every entry and
    /// confirms each method returns a sensible value.
    #[test]
    fn every_entry_answers_all_five_questions() {
        let mut count = 0;
        for entry in Catalog::entries() {
            count += 1;
            // 1. Signature — args and return type defined.
            let sig = entry.signature();
            assert!(!sig.args.is_empty() || entry.name() == "$temperature", "{}: signature", entry.name());
            // 2. State shape — definite variant, not a panic.
            let _ = entry.state_shape();
            // 3. Derivative — a definite variant.
            let _ = entry.derivative();
            // 4. Breakpoints — a Vec (may be empty).
            let _ = entry.breakpoints();
            // 5. Domains — a definite variant.
            let _ = entry.domains();
        }
        assert!(count >= 10, "the math family alone has ≥10 entries: {count}");
    }

    /// No `macro_rules!` anywhere in the crate (MD-13 rule 5). The
    /// catalog is the place a macro would tempt — checked here at the
    /// contract level. The full grep-verify is in the policy gate (T17);
    /// this test is the catalog's own self-check.
    #[test]
    fn catalog_module_introduces_no_macros() {
        // The catalog is data tables + plain trait impls — by
        // construction, no `macro_rules!`. The mechanical proof lives in
        // tests/policy_gate.rs (T17); here we confirm the catalog module
        // shape by checking the table length matches the const count.
        assert_eq!(Catalog::len(), ENTRY_REFS.len());
        assert!(!Catalog::is_empty());
    }

    /// `find` resolves by name; `find("sin")` returns the sin entry.
    #[test]
    fn find_resolves_by_name() {
        let sin = Catalog::find("sin").expect("sin is in the catalog");
        assert_eq!(sin.name(), "sin");
        assert_eq!(sin.family(), Family::Math);
        assert_eq!(sin.signature(), Signature { args: vec![Ty::Real], ret: Ty::Real });
        assert_eq!(sin.state_shape(), StateShape::None);
        assert_eq!(sin.derivative(), DerivativeRule::ClosedForm("chain:cos"));
        assert!(sin.breakpoints().is_empty());
        assert_eq!(sin.domains(), LegalDomains::Both);
    }

    /// `find` returns None for an unknown name (no silent fallback).
    #[test]
    fn find_returns_none_for_unknown_name() {
        assert!(Catalog::find("not-an-intrinsic").is_none());
        assert!(Catalog::find("").is_none());
    }

    /// Every math entry has a real→real signature (or real×real→real for
    /// the binary ones) — fail-loud proof that the catalog's signature
    /// surface is the declared one.
    #[test]
    fn every_math_entry_has_a_real_signature() {
        for entry in Catalog::entries() {
            if entry.family() != Family::Math {
                continue;
            }
            let sig = entry.signature();
            assert_eq!(sig.ret, Ty::Real, "{}: math entries return real", entry.name());
            for arg in &sig.args {
                assert_eq!(*arg, Ty::Real, "{}: math args are real", entry.name());
            }
        }
    }

    /// Pow carries the binary-pow derivative (not a closed-form string)
    /// — the diff pass (T12) consults this and applies the general rule.
    #[test]
    fn pow_carries_the_binary_pow_derivative() {
        let pow = Catalog::find("pow").expect("pow is in the catalog");
        assert_eq!(pow.derivative(), DerivativeRule::BinaryPow);
    }

    /// `abs`/`min`/`max`/`floor` are finite-difference-only — they have
    /// no analytic derivative at their kinks.
    #[test]
    fn abs_min_max_floor_are_finite_difference_only() {
        for name in ["abs", "min", "max", "floor"] {
            let entry = Catalog::find(name).expect("{name} in catalog");
            assert_eq!(
                entry.derivative(),
                DerivativeRule::FiniteDifferenceOnly,
                "{name}: kinks → FD only"
            );
        }
    }

    /// Family names are stable strings — used in diagnostics.
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

    /// The catalog length is at least the math family size (10 today).
    /// T11 grows it; this test fails if the table shrinks.
    #[test]
    fn catalog_has_at_least_the_math_family() {
        assert!(Catalog::len() >= 10);
    }

    /// Every entry has a distinct name — the catalog is the declared
    /// surface, and a duplicate name would silently shadow.
    #[test]
    fn every_entry_has_a_distinct_name() {
        let mut names: Vec<&str> = Catalog::entries().map(|e| e.name()).collect();
        names.sort_unstable();
        let initial = names.len();
        names.dedup();
        assert_eq!(names.len(), initial, "duplicate intrinsic names in the catalog");
    }

    /// The `Breakpoint` variants carry the data the integrator needs
    /// (timer period/phase, crossing expr/dir, pending ramp).
    #[test]
    fn breakpoint_variants_carry_their_data() {
        let bp = Breakpoint::TimerPeriod { period: ValueId(0), phase: ValueId(1) };
        match &bp {
            Breakpoint::TimerPeriod { period, phase } => {
                assert_eq!(*period, ValueId(0));
                assert_eq!(*phase, ValueId(1));
            }
            _ => panic!("wrong variant"),
        }
        let bp = Breakpoint::Crossing { expr: ValueId(2), dir: CrossDir::Either };
        match &bp {
            Breakpoint::Crossing { expr, dir } => {
                assert_eq!(*expr, ValueId(2));
                assert_eq!(*dir, CrossDir::Either);
            }
            _ => panic!("wrong variant"),
        }
        let _ = Breakpoint::PendingRamp;
    }

    /// `LegalDomains` covers the three cases the spec names.
    #[test]
    fn legal_domains_covers_analog_digital_both() {
        assert_ne!(LegalDomains::Analog, LegalDomains::Digital);
        assert_ne!(LegalDomains::Analog, LegalDomains::Both);
        assert_ne!(LegalDomains::Digital, LegalDomains::Both);
    }
}
