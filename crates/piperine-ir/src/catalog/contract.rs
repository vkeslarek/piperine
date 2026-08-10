//! The five questions every catalog entry answers, and the [`Intrinsic`]
//! trait that asks them (LOV-12, `mod.rs`'s doc comment).

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
