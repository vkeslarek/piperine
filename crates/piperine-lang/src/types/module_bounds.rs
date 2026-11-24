//! `Fn(P)` bounds on modules (§8.4.5, GN-23): a module fits `C: Fn(P1, .., Pn)` when its ports have the types and directions
//! `P`, in order, names not counting; a module with a result fits as well, the bound says nothing about a result. A module
//! that does not is `E0360` naming the first port that differs.

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, FileId, TypeId};

use super::check::Checker;
use super::Ty;

/// The first port at which a module differs from the inputs of an `Fn(P)` bound.
pub(super) struct PortDifference {
    /// What differs, as the message of `E0360` says it.
    pub(super) what: String,
}

impl Checker<'_> {
    /// The first port of `module` that differs from the inputs `inputs` of an `Fn(P)` bound (§8.4.5): a port of another type,
    /// a bound input with no port, or a port the bound has no input for. `None` when the module fits.
    pub(super) fn first_differing_port(&mut self, module: DeclId, inputs: &[TypeId]) -> Option<PortDifference> {
        let wanted: Vec<Option<Ty>> = inputs.iter().map(|input| self.lower_type(*input)).collect();
        let found = self.module_port_types(module)?;
        let names: Vec<String> = self.design.decl_data(module).children.iter().filter_map(|child| match &self.design.decl_data(*child).kind {
            DeclKind::Port(_) => Some(self.design.decl_data(*child).name.as_ref().map_or_else(String::new, |name| name.text.clone())),
            _ => None,
        }).collect();
        for (position, (name, have)) in names.iter().zip(&found).enumerate() {
            match wanted.get(position) {
                // a bound input that does not lower was refused where it is written
                Some(None) => return None,
                Some(Some(want)) if want == have => {}
                Some(Some(want)) => return Some(PortDifference { what: format!("its port `{name}` has type `{}`, and `Fn` wants `{}` at position {}", have.name(self.design), want.name(self.design), position + 1) }),
                None => return Some(PortDifference { what: format!("its port `{name}` has no input in `Fn` at position {}", position + 1) }),
            }
        }
        let missing = wanted.get(found.len())?.as_ref()?;
        Some(PortDifference { what: format!("it has no port at position {}, and `Fn` wants `{}`", found.len() + 1, missing.name(self.design)) })
    }

    /// `E0360` for the module `module` against the bound `Fn(inputs)` of `parameter`, `false` after it. A module that fits is `true`.
    pub(super) fn module_fits_callable(&mut self, module: DeclId, parameter: &str, inputs: &[TypeId], file: FileId, span: Span) -> bool {
        let Some(difference) = self.first_differing_port(module, inputs) else { return true };
        let name = self.design.decl_data(module).name.as_ref().map_or_else(String::new, |name| name.text.clone());
        let message = format!("module `{name}` does not satisfy the bound `Fn` of `{parameter}`: {} (§8.4.5)", difference.what);
        self.coded(file, "E0360", span, message, "a module whose ports differ from the bound");
        false
    }
}
