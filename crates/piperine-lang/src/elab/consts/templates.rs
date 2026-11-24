//! Template literals folded by the constant evaluator (§8.4.1, GN-21): `Name { params }` met in a `const`, in the result of
//! a function or in a param default is a value of the elaboration, interned as a number (`elab/templates.rs`).

use piperine_ir::Constant;

use super::{Bindings, ConstEval, ConstFault};
use crate::elab::templates::{TemplateValue, Templates};
use crate::pom::{DeclId, DeclKind, ExprId, FieldInit, Resolution};

impl ConstEval<'_, '_> {
    /// The templates folded so far.
    pub(in crate::elab) fn templates(&self) -> &Templates {
        &self.templates
    }

    /// The number of `value`, interned in this evaluation.
    pub(in crate::elab) fn intern_template(&mut self, value: TemplateValue) -> Constant {
        self.templates.intern(value)
    }

    /// A template literal `Name { params }` met in a `const`, a function's result or a param default (§8.4.1): the module
    /// it names and the value of each param its block binds, interned. The literal is a value of the elaboration, so every
    /// param value folds here; a template folded this way and one the placement folds are the same number.
    pub(in crate::elab) fn template_literal(&mut self, module: DeclId, fields: &[FieldInit], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let mut folded = Vec::new();
        for field in fields {
            let Resolution::Decl(param) = field.res else { continue };
            folded.push((param, self.fold(field.value, bindings)?, field.value));
        }
        Ok(self.templates.intern(TemplateValue { module, fields: folded }))
    }
}

impl ConstEval<'_, '_> {
    /// The value the param `name` takes in the template `number` (§3.1.4, GN-23): what the template's block binds, else the
    /// default of the module's param, folded over the params of the template so far (a default may read an earlier param).
    pub(in crate::elab) fn template_param(&mut self, expr: ExprId, number: Constant, name: &str) -> Result<Constant, ConstFault> {
        let Some(template) = self.templates.get(number).cloned() else {
            return Err(ConstFault::Unsupported { expr, what: "a field read of a value that is not a template".to_string() });
        };
        let mut known = Bindings::new();
        for (param, value, _) in &template.fields {
            known.bind(*param, *value);
        }
        let params: Vec<DeclId> = self.design.decl_data(template.module).children.iter().copied().filter(|child| matches!(self.design.decl_data(*child).kind, DeclKind::Param(_))).collect();
        for param in params {
            let data = self.design.decl_data(param);
            let is_named = data.name.as_ref().is_some_and(|own| own.text == name);
            if known.get(param).is_none() {
                let DeclKind::Param(declared) = &data.kind else { continue };
                if let Some(default) = declared.default {
                    let value = self.fold(default, &known)?;
                    known.bind(param, value);
                }
            }
            if is_named {
                return known.get(param).ok_or(ConstFault::Unsupported { expr, what: format!("the param `{name}` of a template, which has no value") });
            }
        }
        Err(ConstFault::Unsupported { expr, what: format!("a param `{name}` the template's module does not have") })
    }
}
