//! Template values: a param of module type holds a template `Name { params }` (§8.1.1, §8.4.4;
//! PI-04). The param is structural, so its value is a constant of the elaboration: the template
//! is folded to the module it names and the constants of its param block, interned, and the
//! param holds the template's number as a `Constant::Int`. Two equal templates are one number, so
//! they select one body variant (`structural.rs`); a placement through the param (`cell(c)`)
//! places the template's module with the template's values (`children.rs`).

use piperine_ir::Constant;

use super::consts::{Bindings, ConstFault};
use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution, StructClass};

/// A template folded: the module it names and the value each param of its block gives.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct TemplateValue {
    /// The module the template places.
    pub(super) module: DeclId,
    /// The params the block binds: the param, its folded value and the expression that gave it.
    pub(super) fields: Vec<(DeclId, Constant, ExprId)>,
}

/// The templates interned so far, by number.
#[derive(Debug, Default)]
pub(super) struct Templates {
    values: Vec<TemplateValue>,
}

impl Templates {
    /// The number of `value`: the one it was given before, else the next.
    fn intern(&mut self, value: TemplateValue) -> Constant {
        let number = match self.values.iter().position(|known| known.module == value.module && known.fields.iter().map(|f| (f.0, f.1)).eq(value.fields.iter().map(|f| (f.0, f.1)))) {
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
    /// Whether `param` is declared with a module type: its value is a template.
    pub(super) fn is_module_param(&self, param: DeclId) -> bool {
        matches!(&self.design.decl_data(param).kind, DeclKind::Param(p) if matches!(self.design.type_resolution(p.ty), Resolution::Decl(m) if matches!(self.design.decl_data(m).kind, DeclKind::Module(_))))
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
        Ok(self.templates.intern(TemplateValue { module, fields: folded }))
    }
}
