//! The forms the type checker accepts and M1 elaboration does not lower (ELAB-06), in one
//! table. Every row is `P0001` and names the form; a new row is one variant and one arm of
//! `Unlowered::name`, which has no wildcard.
//!
//! Not listed here because the type checker refuses them first, with its own `P0001`:
//! an `extern` module (`types/`'s subset table). The digital forms the checker accepts are
//! lowered by the structure passes of msl-digital (`carriers.rs`, `drives.rs`, `events.rs`,
//! `hoist.rs`); a value input of a module without a result stays refused here.

use super::elaborator::Elaboration;
use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, FileId};

/// One form M1 elaboration refuses.
pub(super) enum Unlowered<'a> {
    /// A placement that sets a name that is no param of the placed module (`m`, `dtemp`).
    PlacementSets(&'a str),
    /// A port of a value type, the expression the caller wrote.
    ValueInput,
}

impl Unlowered<'_> {
    /// The form as the diagnostic names it.
    fn name(&self) -> String {
        match self {
            Unlowered::PlacementSets(name) => format!("a placement that sets `{name}`"),
            Unlowered::ValueInput => "a value input".to_string(),
        }
    }
}

impl Elaboration<'_, '_> {
    /// `P0001` at `span`, naming `form`.
    pub(super) fn refuse(&mut self, file: FileId, span: Span, form: Unlowered) {
        self.unsupported(file, span, &form.name());
    }
}

impl Elaboration<'_, '_> {
    /// `P0001` for each form a module that gets a body holds and elaboration has no lowering for
    /// yet (a value input of a module without a result): a body built without it would run as a
    /// different circuit. `true` when anything was refused.
    pub(super) fn refuse_digital_forms(&mut self, modules: &[DeclId]) -> bool {
        let before = self.diagnostics.len();
        for module in modules {
            let data = self.design.decl_data(*module).clone();
            if !matches!(data.kind, DeclKind::Module(_)) {
                continue;
            }
            for child in data.children.iter().copied() {
                let item = self.design.decl_data(child).clone();
                if let DeclKind::Port(p) = &item.kind
                    && self.types.carrier(child).is_none()
                    && self.design.nature_of(p.ty).is_none()
                {
                    self.refuse(item.file, item.span, Unlowered::ValueInput);
                }
            }
        }
        self.diagnostics.len() > before
    }
}
