//! The forms the type checker accepts and M1 elaboration does not lower (ELAB-06), in one
//! table. Every row is `P0001` and names the form; a new row is one variant and one arm of
//! `Unlowered::name`, which has no wildcard.
//!
//! Not listed here because the type checker refuses them first, with its own `P0001`:
//! an `extern` module (`types/`'s subset table). The digital forms the checker accepts (a
//! carrier, a definition, a `digital` block, a module with a result) are refused here until
//! the structure passes of msl-digital lower them.

use super::elaborator::Elaboration;
use crate::msl::Span;
use crate::pom::{BehaviorKind, DeclId, DeclKind, FileId};

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
    /// A discrete port or signal.
    Carrier,
    /// A definition `place = expr;`.
    Definition,
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
            Unlowered::DigitalBlock => "a `digital` block".to_string(),
            Unlowered::SecondAnalogBlock => "a second `analog` block for one module".to_string(),
            Unlowered::Carrier => "a discrete carrier".to_string(),
            Unlowered::Definition => "a definition `=`".to_string(),
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
    /// `P0001` for each digital form a reached module holds: elaboration has no lowering for a
    /// carrier, a definition, a `digital` block or a result yet, and a body built without them
    /// would run as a different circuit. `true` when anything was refused.
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
                match &item.kind {
                    DeclKind::Port(_) | DeclKind::Signal(_) if self.types.carrier(child).is_some() => self.refuse(item.file, item.span, Unlowered::Carrier),
                    DeclKind::Port(p) if self.design.nature_of(p.ty).is_none() => self.refuse(item.file, item.span, Unlowered::ValueInput),
                    DeclKind::Definition(_) => self.refuse(item.file, item.span, Unlowered::Definition),
                    _ => {}
                }
            }
            for behavior in m.behaviors.iter().copied() {
                let b = self.design.decl_data(behavior).clone();
                if matches!(&b.kind, DeclKind::Behavior(bd) if bd.kind == BehaviorKind::Digital) {
                    self.refuse(b.file, b.span, Unlowered::DigitalBlock);
                }
            }
        }
        self.diagnostics.len() > before
    }
}
