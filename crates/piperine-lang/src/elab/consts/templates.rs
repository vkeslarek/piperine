//! Template literals folded by the constant evaluator (§8.4.1, GN-21): `Name { params }` met in a `const`, in the result of
//! a function or in a param default is a value of the elaboration, interned as a number (`elab/templates.rs`).

use piperine_ir::Constant;

use super::{Bindings, ConstEval, ConstFault};
use crate::elab::templates::{TemplateValue, Templates};
use crate::pom::{DeclId, FieldInit, Resolution};

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
