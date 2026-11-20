//! The forms the type checker accepts and M1 elaboration does not lower (ELAB-06), in one
//! table. Every row is `P0001` and names the form; a new row is one variant and one arm of
//! `Unlowered::name`, which has no wildcard.
//!
//! Not listed here because the type checker refuses them first, with its own `P0001`:
//! an `extern` module (`types/`'s subset table). The digital forms the checker accepts are
//! lowered by the structure passes of msl-digital (`carriers.rs`, `drives.rs`, `events.rs`,
//! `hoist.rs`).

use super::elaborator::Elaboration;
use crate::msl::Span;
use crate::pom::FileId;

/// One form M1 elaboration refuses.
pub(super) enum Unlowered<'a> {
    /// A placement that sets a name that is no param of the placed module (`m`, `dtemp`).
    PlacementSets(&'a str),
}

impl Unlowered<'_> {
    /// The form as the diagnostic names it.
    fn name(&self) -> String {
        match self {
            Unlowered::PlacementSets(name) => format!("a placement that sets `{name}`"),
        }
    }
}

impl Elaboration<'_, '_> {
    /// `P0001` at `span`, naming `form`.
    pub(super) fn refuse(&mut self, file: FileId, span: Span, form: Unlowered) {
        self.unsupported(file, span, &form.name());
    }
}
