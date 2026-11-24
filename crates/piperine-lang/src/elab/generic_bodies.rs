//! The bodies of a generic module (generics GN-13, GN-15, part III §8): a placement of a generic module folds the
//! arguments the checker recorded for it under the substitution of the body that places it, discharges the module's
//! `where` and length obligations for them, and asks for the body of that instance. Equal arguments share one body; a
//! chain of instances that grows without end is `E0341` at depth [`MOST_NESTED_MODULE_INSTANCES`] (D7).

use piperine_ir::Constant;

use super::elaborator::Elaboration;
use super::instances::Instance;
use super::lower::Scope;
use super::structural::BodyKey;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FileId, TypeId};
use crate::types::Ty;

/// How many instances of generic modules may place one another before the chain is cut (D7, as for functions).
const MOST_NESTED_MODULE_INSTANCES: usize = 1024;

impl Elaboration<'_, '_> {
    /// Whether `module` is a generic module: it has no body of its own, each instance is one.
    pub(super) fn is_generic_module(&self, module: DeclId) -> bool {
        let data = self.design.decl_data(module);
        matches!(data.kind, DeclKind::Module(_)) && data.generics_span.is_some()
    }

    /// The nature the node type `ty` is: the one it names, or the one the nature parameter it names takes in the
    /// instance being lowered.
    pub(super) fn nature_decl_of(&self, ty: TypeId) -> Option<DeclId> {
        self.design.nature_of(ty).or_else(|| match self.typed_written(ty)? {
            Ty::Node(nature) => Some(nature),
            _ => None,
        })
    }

    /// The instance the placement `call` of the generic module `module` makes, folded under the substitution of the body
    /// being lowered, its `where` and length obligations discharged and its params planned. `None` after a diagnostic.
    pub(super) fn module_instance(&mut self, call: ExprId, module: DeclId) -> Option<Instance> {
        let (file, span) = (self.design.decl_data(self.design.expr_node(call).owner).file, self.design.expr_node(call).span);
        if self.types.instantiation(call).is_none() {
            // never a silent loss: a placement whose instance nothing recorded is refused where it stands
            self.unsupported(file, span, "a placement of a generic module whose arguments the checker did not record");
            return None;
        }
        let instance = self.concrete_instance(call)?;
        self.admit_module_instance(module, instance, file, span)
    }

    /// The instance the placement `call` of the generic module `module` makes: the arguments of the template it goes through
    /// (`cell(c)`, `T(c)`, §8.4.3), else the ones recorded for the placement itself (`module_instance`). `None` after a diagnostic.
    pub(super) fn placement_instance(&mut self, scope: &Scope, call: ExprId, module: DeclId) -> Option<Instance> {
        let ExprKind::Call { class, .. } = self.design.expr_node(call).kind.clone() else { return None };
        let Ok(through) = self.template_through(scope, call, &class) else { return None };
        let Some(template) = through else { return self.module_instance(call, module) };
        let (file, span) = (self.design.decl_data(self.design.expr_node(call).owner).file, self.design.expr_node(call).span);
        let instance = Instance { item: module, arguments: template.arguments };
        let bindings = self.instance_bindings(&instance);
        self.discharge(&instance, &bindings)?;
        self.admit_module_instance(module, instance, file, span)
    }

    /// `instance` of `module` with its `where` discharged and its params planned.
    fn admit_module_instance(&mut self, module: DeclId, instance: Instance, file: FileId, span: Span) -> Option<Instance> {
        self.discharge_module_where(module, &instance, file, span)?;
        self.plan_instance_params(module, &instance);
        Some(instance)
    }

    /// The `where` of `module` evaluated for `instance`: false is `E0304` naming the instance.
    fn discharge_module_where(&mut self, module: DeclId, instance: &Instance, file: FileId, span: Span) -> Option<()> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Some(()) };
        let Some(condition) = data.where_ else { return Some(()) };
        let bindings = self.instance_bindings(instance);
        let outer = self.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
        let value = self.consts.value(condition, &bindings);
        self.consts.set_substitution(outer);
        match value {
            Ok(Constant::Bool(false)) => {
                let message = format!("the `where` of module `{}` is false for the instance", instance.name(self));
                self.report(file, Diagnostic::new("E0304", message, span, "a module `where` that is false"));
                None
            }
            Ok(_) => Some(()),
            Err(fault) => {
                self.const_fault(&fault, "a module `where` condition");
                None
            }
        }
    }

    /// Plans the params of `module` for `instance` once, their defaults folded under its arguments.
    pub(super) fn plan_instance_params(&mut self, module: DeclId, instance: &Instance) {
        if self.instance_param_plans.contains_key(instance) {
            return;
        }
        let outer = self.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
        let plans = self.plan_module_params(module);
        self.consts.set_substitution(outer);
        self.instance_param_plans.insert(instance.clone(), plans);
    }

    /// Records how deep the body `key` sits, one below the body that asks for it; past the bound it is `E0341`.
    pub(super) fn admit_instance_depth(&mut self, key: &BodyKey) -> Option<()> {
        let depth = self.lowering.as_ref().and_then(|asking| self.body_depths.get(asking)).map_or(0, |asking| asking + 1);
        self.body_depths.insert(key.clone(), depth);
        if depth > MOST_NESTED_MODULE_INSTANCES {
            let data = self.design.decl_data(key.module);
            let name = key.instance.as_ref().map_or_else(|| data.name.as_ref().map_or_else(String::new, |n| n.text.clone()), |instance| instance.item_name(self));
            let message = format!("instantiating module `{name}` never ends: the chain of instantiations is deeper than {MOST_NESTED_MODULE_INSTANCES} (D7)");
            self.report(data.file, Diagnostic::new("E0341", message, data.span, "an instantiation chain that does not end"));
            return None;
        }
        Some(())
    }
}
