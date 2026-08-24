//! §7.4.4's own two discipline-resolution modes for undeclared
//! mixed/continuous-signal nets — "the selection of these discipline
//! resolution modes shall be vendor-specific" (§7.4.4), so Piperine
//! exposes it as an [`super::ElabContext`] seam (like `schemas`) rather
//! than a language keyword: no VAMS Annex A production picks the mode,
//! so it cannot be surfaced as new grammar (C7).

/// One of the two Annex F discipline-resolution modes (T60, LRMW2-56).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisciplineResolutionMode {
    /// §7.4.4.1 (the LRM's own stated default): both continuous and
    /// discrete disciplines propagate up the hierarchy; `connect ...
    /// resolveto` rules resolve discrete/discrete conflicts along the
    /// way, and continuous wins only where continuous and discrete
    /// actually meet.
    #[default]
    Basic,
    /// §7.4.4.2: continuous disciplines propagate up *and back down*;
    /// discrete disciplines never propagate up, and `resolveto` rules are
    /// ignored for ordinary (non-coercion) conflict resolution — "The
    /// specified `resolveto` connect statements are ignored in this mode
    /// unless coercion ... is used."
    Detail,
}
