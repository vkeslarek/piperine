//! The forms the type checker accepts and M1 elaboration does not lower (ELAB-06), in one
//! table. Every row is `P0001` and names the form; a new row is one variant and one arm of
//! `Unlowered::name`, which has no wildcard.
//!
//! Not listed here because the type checker refuses them first, with its own `P0001`:
//! an `extern` module (`types/`'s subset table). The digital forms the checker accepts (a
//! module with a result) is refused here until the structure passes of msl-digital lower it; a
//! carrier, a definition and a `digital` block are lowered (`carriers.rs`, `drives.rs`,
//! `events.rs`).

use super::elaborator::Elaboration;
use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, FileId};

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
    /// A second `analog` block of one module.
    SecondAnalogBlock,
    /// A module with a result.
    ResultModule,
    /// A port of a value type, the expression the caller wrote.
    ValueInput,
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
            Unlowered::SecondAnalogBlock => "a second `analog` block for one module".to_string(),
            Unlowered::ResultModule => "a module with a result".to_string(),
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
    /// `P0001` for each form a reached module holds that elaboration has no lowering for yet (a
    /// module with a result, a value input): a body built without it would run as a different
    /// circuit. `true` when anything was refused.
    pub(super) fn refuse_digital_forms(&mut self, modules: &[DeclId]) -> bool {
        let before = self.diagnostics.len();
        for module in modules {
            let data = self.design.decl_data(*module).clone();
            let DeclKind::Module(m) = &data.kind else { continue };
            if m.ret.is_some() {
                self.refuse(data.file, data.span, Unlowered::ResultModule);
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
