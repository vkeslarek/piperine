//! POM constraint nodes — the pointwise/reduced quantity classification
//! (dv-core DVC-09). The block types themselves land with `Module::constraints`
//! (T9); this module currently holds the shared classification vocabulary the
//! elaborator's helper registry and the POM both speak.

/// Whether a constraint quantity exists at every evaluation point of an
/// analysis or only once the analysis has run to completion (design §C2 —
/// the class is a property of the *declaration*, read off
/// `headers/constraints.phdl`'s `@quantity(class, analysis)` marker, never
/// inferred from a use site).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum QuantityClass {
    /// Evaluable at every point (`ac_gain`, `V()`, `I()`) — lowers to a
    /// per-point margin kernel.
    Pointwise,
    /// Exists once per analysis (`ac_unity_gain_freq`, `ac_phase_margin`) —
    /// a host-side reduction over the analysis's points, never a kernel.
    Reduced,
}

impl std::fmt::Display for QuantityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointwise => write!(f, "pointwise"),
            Self::Reduced => write!(f, "reduced"),
        }
    }
}
