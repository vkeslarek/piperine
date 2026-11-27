//! Natures: each nature a reached module's ports and signals are made of becomes one IR nature,
//! in first-use order, so the ids do not depend on anything but the source.

use piperine_ir::{Constant, NatureId, Quantity, Real};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::refusal::forms::AnalogForm;
use crate::msl::Diagnostic;
use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprKind, FileId, QuantityRole};

impl Elaboration<'_, '_> {
    /// The natures of `modules`' ports and signals, added to the IR before any body.
    pub(super) fn lower_natures(&mut self, modules: &[DeclId]) {
        for module in modules {
            for child in self.design.decl_data(*module).children.clone() {
                let ty = match &self.design.decl_data(child).kind {
                    // A discrete carrier's nature (`Bit`) has no quantity: it is no IR nature.
                    _ if self.types.carrier(child).is_some() => continue,
                    DeclKind::Port(p) => p.ty,
                    DeclKind::Signal(s) => s.ty,
                    _ => continue,
                };
                if let Some(nature) = self.design.nature_of(ty) {
                    self.nature(nature);
                }
            }
        }
    }

    /// The IR nature of a `nature` declaration, added the first time it is asked for.
    pub(super) fn nature(&mut self, nature: DeclId) -> Option<NatureId> {
        if let Some(id) = self.natures.get(&nature) {
            return Some(*id);
        }
        let data = self.design.decl_data(nature);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (file, span) = (data.file, data.span);
        let potential = self.quantity(nature, QuantityRole::Potential);
        let flow = self.quantity(nature, QuantityRole::Flow);
        let Some(potential) = potential else {
            self.refuse_with(file, span, AnalogForm::NatureWithoutPotential, [name.to_string()]);
            return None;
        };
        self.check_access_names(nature, &potential, flow.as_ref());
        let id = self.ir.nature(&name, potential, flow);
        self.natures.insert(nature, id);
        Some(id)
    }

    /// The first quantity declaration of `role` a nature has.
    fn quantity_declaration(&self, nature: DeclId, role: QuantityRole) -> Option<DeclId> {
        let children = self.design.decl_data(nature).children.clone();
        children.into_iter().find(|c| matches!(&self.design.decl_data(*c).kind, DeclKind::Quantity(q) if q.role == role))
    }

    /// `E0205` (Part II §4 rule 10): the two `access` names of one nature are equal. Reported at the
    /// flow quantity's name, naming the access function and the nature.
    fn check_access_names(&mut self, nature: DeclId, potential: &Quantity, flow: Option<&Quantity>) {
        let Some(flow) = flow else { return };
        if potential.access.is_empty() || potential.access != flow.access {
            return;
        }
        let Some(declaration) = self.quantity_declaration(nature, QuantityRole::Flow) else { return };
        let data = self.design.decl_data(declaration);
        let (file, span) = (data.file, data.name.as_ref().map_or(data.span, |name| name.span));
        let nature_name = self.design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let message = format!("the `potential` and the `flow` of nature `{nature_name}` share the access name `{}`", flow.access);
        self.report(file, Diagnostic::new("E0205", message, span, "the access names of a nature must differ"));
    }

    /// `E0204` (Part II §4 rule 9): a quantity attribute that is not allowed, with the reason.
    fn quantity_attribute_fault(&mut self, file: FileId, span: Span, attribute: &str, reason: &str) {
        let message = format!("the quantity attribute `{attribute}` {reason}");
        self.report(file, Diagnostic::new("E0204", message, span, "invalid quantity attribute"));
    }

    /// The first quantity of `role` a nature declares: its name, access function, units and
    /// absolute tolerance. An attribute outside `access`, `units`, `abstol`, one of the wrong type
    /// and an `abstol` that is not positive are `E0204` (Part II §4 rule 9).
    fn quantity(&mut self, nature: DeclId, role: QuantityRole) -> Option<Quantity> {
        let quantity = self.quantity_declaration(nature, role)?;
        let data = self.design.decl_data(quantity).clone();
        let DeclKind::Quantity(q) = &data.kind else { return None };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut access = String::new();
        let mut units = String::new();
        let mut abstol = Real::new(0.0);
        for attr in &q.attrs {
            let attribute = attr.name.text.as_str();
            let value_span = self.design.expr_node(attr.value).span;
            match attribute {
                "access" => match &self.design.expr_node(attr.value).kind {
                    ExprKind::Path(path) => access = self.design.path_node(*path).segments.last().map_or_else(String::new, |s| s.name.text.clone()),
                    _ => self.quantity_attribute_fault(data.file, value_span, attribute, "must be an identifier"),
                },
                "units" => match &self.design.expr_node(attr.value).kind {
                    ExprKind::Lit(lit) if lit.str_value().is_some() => units = lit.str_value().unwrap_or_default(),
                    _ => self.quantity_attribute_fault(data.file, value_span, attribute, "must be a string literal"),
                },
                "abstol" => match self.consts.value(attr.value, &Bindings::new()) {
                    Ok(Constant::Real(value)) if value.get() > 0.0 => abstol = value,
                    Ok(Constant::Real(_)) => self.quantity_attribute_fault(data.file, value_span, attribute, "must be greater than zero"),
                    Ok(_) => self.quantity_attribute_fault(data.file, value_span, attribute, "must be a const `f64`"),
                    Err(fault) => self.const_fault(&fault, "an `abstol`"),
                },
                other => self.quantity_attribute_fault(data.file, attr.name.span, other, "is not one of `access`, `units`, `abstol`"),
            }
        }
        Some(Quantity { name, access, units, abstol })
    }
}
