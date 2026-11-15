//! The forms the type checker accepts and M1 elaboration does not lower (ELAB-06), in one
//! table. Every row is `P0001` and names the form; a new row is one variant and one arm of
//! `Unlowered::name`, which has no wildcard.
//!
//! Not listed here because the type checker refuses them first, with its own `P0001`:
//! a `digital` block of a reachable module and an `extern` module (`types/`'s subset table).

use super::elaborator::Elaboration;
use crate::msl::Span;
use crate::pom::FileId;

/// One form M1 elaboration refuses.
pub(super) enum Unlowered<'a> {
    /// A contribution under an `if` or in a closure.
    ConditionalContribution,
    /// A contribution in a plain block nested in the analog block.
    NestedContribution,
    /// A `limit` under an `if`, in a function or in a closure.
    ConditionalLimit,
    /// An `ac_stim` under an `if`, in a function or in a closure.
    ConditionalStim,
    /// A `ddt` that is not a whole top-level addend of a flow contribution.
    DdtOffTop,
    /// A `ddt` with a tolerance argument.
    DdtTolerance,
    /// A placement that sets a name that is no param of the placed module (`m`, `dtemp`).
    PlacementSets(&'a str),
    /// A `digital` block.
    DigitalBlock,
    /// A second `analog` block of one module.
    SecondAnalogBlock,
}

impl Unlowered<'_> {
    /// The form as the diagnostic names it.
    fn name(&self) -> String {
        match self {
            Unlowered::ConditionalContribution => "a contribution under an `if` or in a closure".to_string(),
            Unlowered::NestedContribution => "a contribution in a nested block".to_string(),
            Unlowered::ConditionalLimit => "a `limit` under an `if` or in a closure".to_string(),
            Unlowered::ConditionalStim => "an `ac_stim` under an `if` or in a closure".to_string(),
            Unlowered::DdtOffTop => "a `ddt` that is not a whole addend of a flow contribution".to_string(),
            Unlowered::DdtTolerance => "a `ddt` with a tolerance".to_string(),
            Unlowered::PlacementSets(name) => format!("a placement that sets `{name}`"),
            Unlowered::DigitalBlock => "a `digital` block".to_string(),
            Unlowered::SecondAnalogBlock => "a second `analog` block for one module".to_string(),
        }
    }
}

impl Elaboration<'_, '_> {
    /// `P0001` at `span`, naming `form`.
    pub(super) fn refuse(&mut self, file: FileId, span: Span, form: Unlowered) {
        self.unsupported(file, span, &form.name());
    }
}
