//! Natures: each nature a reached module's ports and signals are made of becomes one IR nature,
//! in first-use order, so the ids do not depend on anything but the source.

use piperine_ir::{Constant, NatureId, Quantity, Real};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind, ExprKind, QuantityRole};

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
            self.unsupported(file, span, &format!("nature `{name}` without a potential"));
            return None;
        };
        let id = self.ir.nature(&name, potential, flow);
        self.natures.insert(nature, id);
        Some(id)
    }

    /// The first quantity of `role` a nature declares: its name, access function, units and
    /// absolute tolerance.
    fn quantity(&mut self, nature: DeclId, role: QuantityRole) -> Option<Quantity> {
        let children = self.design.decl_data(nature).children.clone();
        let quantity = children.into_iter().find(|c| matches!(&self.design.decl_data(*c).kind, DeclKind::Quantity(q) if q.role == role))?;
        let data = self.design.decl_data(quantity).clone();
        let DeclKind::Quantity(q) = &data.kind else { return None };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut access = String::new();
        let mut units = String::new();
        let mut abstol = Real::new(0.0);
        for attr in &q.attrs {
            match attr.name.text.as_str() {
                "access" => {
                    if let ExprKind::Path(path) = &self.design.expr_node(attr.value).kind {
                        access = self.design.path_node(*path).segments.last().map_or_else(String::new, |s| s.name.text.clone());
                    }
                }
                "units" => {
                    if let ExprKind::Lit(lit) = &self.design.expr_node(attr.value).kind {
                        units = lit.str_value().unwrap_or_default();
                    }
                }
                "abstol" => match self.consts.value(attr.value, &Bindings::new()) {
                    Ok(Constant::Real(value)) => abstol = value,
                    Ok(Constant::Bool(_)) => self.unsupported(data.file, self.design.expr_node(attr.value).span, "a `bool` abstol"),
                    Ok(Constant::Int(_)) => self.unsupported(data.file, self.design.expr_node(attr.value).span, "an integer abstol"),
                    Err(fault) => self.const_fault(&fault, "an `abstol`"),
                },
                _ => {}
            }
        }
        Some(Quantity { name, access, units, abstol })
    }
}
