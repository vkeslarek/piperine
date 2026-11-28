//! `Fn(P)` bounds on modules (§8.4.5, GN-23): a module fits `C: Fn(P1, .., Pn)` when its ports have the types and directions
//! `P`, in order, names not counting; a module with a result fits as well, the bound says nothing about a result. A module
//! that does not is `E0360` naming the first port that differs.

use crate::msl::Span;
use crate::pom::{BundleView, DeclId, DeclKind, FileId, TypeId};

use super::GenericsSolver;
use crate::types::{Nominal, Substitution, Ty};

/// The first port at which a module differs from the inputs of an `Fn(P)` bound.
pub(super) struct PortDifference {
    /// What differs, as the message of `E0360` says it.
    pub(super) what: String,
}

impl GenericsSolver<'_, '_> {
    /// The first port of `module` that differs from the inputs `inputs` of an `Fn(P)` bound (§8.4.5): a port of another type,
    /// a bound input with no port, or a port the bound has no input for. `None` when the module fits. The ports of a generic
    /// module are read under the arguments its template names (`S<Electrical>`, §8.4.3).
    pub(super) fn first_differing_port(&mut self, template: &Nominal, inputs: &[TypeId]) -> Option<PortDifference> {
        let module = template.decl;
        let wanted: Vec<Option<Ty>> = inputs.iter().map(|input| self.bound_input_type(*input)).collect();
        let mut arguments = Substitution::new();
        for (parameter, argument) in self.generic_signature(module).parameters().iter().zip(&template.arguments) {
            arguments.bind(parameter.decl, argument.clone());
        }
        let found: Vec<Ty> = self.module_port_types(module)?.iter().map(|port| arguments.apply(port)).collect();
        let ports: Vec<DeclId> = self.state.design.decl_data(module).children.iter().copied().filter(|child| matches!(self.state.design.decl_data(*child).kind, DeclKind::Port(_))).collect();
        let names: Vec<String> = self.state.design.decl_data(module).children.iter().filter_map(|child| match &self.state.design.decl_data(*child).kind {
            DeclKind::Port(_) => Some(self.state.design.decl_data(*child).name.as_ref().map_or_else(String::new, |name| name.text.clone())),
            _ => None,
        }).collect();
        for (position, (name, have)) in names.iter().zip(&found).enumerate() {
            match wanted.get(position) {
                // a bound input that does not lower was refused where it is written
                Some(None) => return None,
                Some(Some(want)) if want == have => {
                    // a bundle is placed in the view the bound writes: the template's own view of its port must be that one (§7 rules 3 to 5)
                    if let Some(difference) = self.view_difference(ports[position], name, inputs[position]) {
                        return Some(difference);
                    }
                }
                Some(Some(want)) => return Some(PortDifference { what: format!("its port `{name}` has type `{}`, and `Fn` wants `{}` at position {}", have.name(self.state.design), want.name(self.state.design), position + 1) }),
                None => return Some(PortDifference { what: format!("its port `{name}` has no input in `Fn` at position {}", position + 1) }),
            }
        }
        let missing = wanted.get(found.len())?.as_ref()?;
        Some(PortDifference { what: format!("it has no port at position {}, and `Fn` wants `{}`", found.len() + 1, missing.name(self.state.design)) })
    }

    /// The difference between the view the bundle port `port` of a module takes its bundle in and the view the bound input `input`
    /// writes (`Fn(Pipe)` a producer, `Fn(flip Pipe)` a consumer): placing the module through the bound would otherwise drive an input
    /// or leave an output with no driver. `None` for a port that is no bundle, or that has the bound's view.
    fn view_difference(&self, port: DeclId, name: &str, input: TypeId) -> Option<PortDifference> {
        if !self.state.design.is_bundle_owner(port) {
            return None;
        }
        let DeclKind::Port(data) = &self.state.design.decl_data(port).kind else { return None };
        let (have, want) = (BundleView::of_port(self.state.design, data.dir, data.ty), BundleView::of_port(self.state.design, None, input));
        (have != want).then(|| PortDifference { what: format!("its port `{name}` takes the bundle as {}, and `Fn` wants it as {} (§7 rules 3 to 5)", have.article(), want.article()) })
    }

    /// `E0360` for the template's module against the bound `Fn(inputs)` of `parameter`, `false` after it. A module that fits is `true`.
    pub(super) fn module_fits_callable(&mut self, template: &Nominal, parameter: &str, inputs: &[TypeId], file: FileId, span: Span) -> bool {
        let Some(difference) = self.first_differing_port(template, inputs) else { return true };
        let name = self.state.design.decl_data(template.decl).name.as_ref().map_or_else(String::new, |name| name.text.clone());
        let message = format!("module `{name}` does not satisfy the bound `Fn` of `{parameter}`: {} (§8.4.5)", difference.what);
        self.coded(file, "E0360", span, message, "a module whose ports differ from the bound");
        false
    }
}
