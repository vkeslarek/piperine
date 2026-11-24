//! Template values: a param of module type holds a template `Name { params }` (§8.1.1, §8.4.4;
//! PI-04). The param is structural, so its value is a constant of the elaboration: the template
//! is folded to the module it names and the constants of its param block, interned, and the
//! param holds the template's number as a `Constant::Int`. Two equal templates are one number, so
//! they select one body variant (`structural.rs`); a placement through the param (`cell(c)`)
//! places the template's module with the template's values (`children.rs`).

use piperine_ir::Constant;

use super::consts::{Bindings, ConstFault};
use super::elaborator::Elaboration;
use super::instances::ConcreteValue;
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, Resolution, StructClass};
use crate::types::Callee;

/// A template folded: the module it names and the value each param of its block gives.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::elab) struct TemplateValue {
    /// The module the template places.
    pub(super) module: DeclId,
    /// The generic arguments of the module, in the order of its parameters (§8.4.3); empty for a module that is not generic.
    pub(super) arguments: Vec<ConcreteValue>,
    /// The params the block binds: the param, its folded value and the expression that gave it.
    pub(super) fields: Vec<(DeclId, Constant, ExprId)>,
}

/// The templates interned so far, by number.
#[derive(Debug, Default)]
pub(in crate::elab) struct Templates {
    values: Vec<TemplateValue>,
}

impl Templates {
    /// The number of `value`: the one it was given before, else the next.
    pub(in crate::elab) fn intern(&mut self, value: TemplateValue) -> Constant {
        let number = match self.values.iter().position(|known| known.module == value.module && known.arguments == value.arguments && known.fields.iter().map(|f| (f.0, f.1)).eq(value.fields.iter().map(|f| (f.0, f.1)))) {
            Some(number) => number,
            None => {
                self.values.push(value);
                self.values.len() - 1
            }
        };
        Constant::Int(number as i128)
    }

    /// The template `number` names, `None` for a number that was never handed out.
    pub(super) fn get(&self, number: Constant) -> Option<&TemplateValue> {
        match number {
            Constant::Int(n) => usize::try_from(n).ok().and_then(|n| self.values.get(n)),
            _ => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// Whether `param` is declared with a module type, or with a type parameter bounded by `Fn(..)`: its value is a template.
    pub(super) fn is_module_param(&self, param: DeclId) -> bool {
        matches!(&self.design.decl_data(param).kind, DeclKind::Param(p) if matches!(self.design.type_resolution(p.ty), Resolution::Decl(m) if matches!(self.design.decl_data(m).kind, DeclKind::Module(_)) || crate::types::GenericSignature::is_callable(self.design, m)))
    }

    /// The module the placement `call` places: the one the checker names, else (a call of a callable param, `cell(a)` with
    /// `cell: C` and `C: Fn(..)`) the module of the template the param holds in `keys`. `None` for a call that places nothing, or a
    /// param no template is bound to.
    pub(super) fn placed_module(&self, call: ExprId, keys: &Bindings) -> Option<DeclId> {
        if let Some(Callee::Module(module)) = self.types.callee(call) {
            return Some(module);
        }
        let ExprKind::Call { class: CallClass::Instance { module: None, via: Some(via) }, .. } = &self.design.expr_node(call).kind else { return None };
        if !self.is_module_param(*via) {
            return None;
        }
        keys.get(*via).and_then(|number| self.consts.templates().get(number)).map(|template| template.module)
    }

    /// The value `expr` gives a param: a template folded and interned (`Constant::Int` of its
    /// number), any other expression as `ConstEval::value` folds it over `bindings`.
    pub(super) fn fold_param_value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let ExprKind::Struct { fields, class: StructClass::Template(module), .. } = self.design.expr_node(expr).kind.clone() else {
            // a value of `T` bound to a param of `Option<T>` is `Some` of it (§3.2.3)
            if self.types.implicit_some(expr) {
                return self.implicit_some_constant(expr, bindings);
            }
            // a composite folds to a constant of the IR's arena (`consts/imported.rs`)
            if self.typed(expr).is_some_and(|ty| ty.is_composite() || matches!(ty, crate::types::Ty::Enum(_))) {
                return self.composite_constant(expr, bindings);
            }
            return self.consts.value(expr, bindings);
        };
        let mut folded = Vec::new();
        for field in &fields {
            let Resolution::Decl(param) = field.res else { continue };
            folded.push((param, self.fold_param_value(field.value, bindings)?, field.value));
        }
        let arguments = self.consts.template_arguments(expr, bindings)?;
        Ok(self.consts.intern_template(TemplateValue { module, arguments, fields: folded }))
    }
}
