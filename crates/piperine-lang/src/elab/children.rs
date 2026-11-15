//! The placements a module writes: each one a `Child` of its body, the tree the device author
//! wrote. A child names its module's body, the terminals of the parent it connects, and the
//! param values the placement gives (constants; a value that reads the parent's params is M2).

use std::collections::BTreeSet;

use piperine_ir::{Child, Constant, Terminal};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, Resolution, StructClass};
use crate::types::Callee;

/// A placement, as instance placement needs it.
#[derive(Clone)]
pub(super) struct ChildPlan {
    pub(super) name: String,
    pub(super) module: DeclId,
    pub(super) terminals: Vec<Terminal>,
    /// The param values written, as (index in the child's params, value, the value's expression).
    pub(super) given: Vec<(u32, Constant, ExprId)>,
    /// The call expression of the placement.
    pub(super) call: ExprId,
}

impl Elaboration<'_, '_> {
    /// The placements of `module` in source order, each as the `Child` of its body and the plan
    /// placement reads. A placement of a module the type checker did not reach is skipped: it
    /// was refused already.
    pub(super) fn lower_children(&mut self, scope: &Scope, module: DeclId) -> Vec<(Child, ChildPlan)> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let calls = data.instances.clone();
        let mut ordinals: Vec<(DeclId, u32)> = Vec::new();
        let mut out = Vec::new();
        for call in calls {
            let Some(Callee::Module(child_module)) = self.types.callee(call) else { continue };
            let ordinal = match ordinals.iter_mut().find(|(m, _)| *m == child_module) {
                Some((_, k)) => {
                    *k += 1;
                    *k
                }
                None => {
                    ordinals.push((child_module, 0));
                    0
                }
            };
            if let Some(child) = self.lower_child(scope, call, child_module, ordinal) {
                out.push(child);
            }
        }
        out
    }

    fn lower_child(&mut self, scope: &Scope, call: ExprId, module: DeclId, ordinal: u32) -> Option<(Child, ChildPlan)> {
        let node = self.design.expr_node(call).clone();
        let file = self.design.decl_data(node.owner).file;
        let ExprKind::Call { callee, args, params, class } = &node.kind else { return None };
        if !matches!(class, CallClass::Instance { module: Some(m), .. } if *m == module) {
            self.unsupported(file, node.span, "a placement through a template value");
            return None;
        }
        let fields: Vec<FieldInit> = match &self.design.expr_node(*callee).kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module => fields.clone(),
            _ => params.clone().unwrap_or_default(),
        };
        let module_name = self.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let name = self.placement_name(call, &module_name, ordinal);
        let mut ok = true;
        let mut terminals = Vec::new();
        for arg in args {
            match self.lower_terminal(scope, arg.value) {
                Some(terminal) => terminals.push(terminal),
                None => ok = false,
            }
        }
        let given = self.lower_given(file, module, &fields);
        let given = match given {
            Some(given) => given,
            None => {
                ok = false;
                Vec::new()
            }
        };
        if !ok {
            return None;
        }
        let body = self.body_ids.get(&module).copied()?;
        let params = given.iter().map(|(index, value, _)| (*index, *value)).collect();
        let origin = self.locator.origin(file, node.span);
        // SPEC_DEVIATION: `Child::nodes` must bind one terminal per node of the child's body
        // (`Ir::check`), and `Terminal` has no "the child's own node". A signal of the child is
        // not connected to anything of the parent, so its slot is padded with `Ground`.
        // Reason: the authored tree is a view; the instance table (`InstanceSpec::nodes`) is the
        // one backends bind from and holds the child's own node there. No M1 device has a signal.
        let own_signals = self.design.decl_data(module).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::Signal(_))).count();
        let mut slots = terminals.clone();
        slots.extend(std::iter::repeat_n(Terminal::Ground, own_signals));
        let child = Child { name: name.clone(), body, nodes: slots, params, origin };
        Some((child, ChildPlan { name, module, terminals, given, call }))
    }

    /// The name of the `let` that binds the placement, else `<module>#<k>` (§21 rule 3).
    fn placement_name(&self, call: ExprId, module: &str, ordinal: u32) -> String {
        let owner = self.design.expr_node(call).owner;
        if let DeclKind::Let(l) = &self.design.decl_data(owner).kind
            && l.value == call
            && let crate::pom::PatKind::Ident { name, sub: None, .. } = &self.design.pat_node(l.pat).kind
        {
            return name.text.clone();
        }
        format!("{module}#{ordinal}")
    }

    /// The param values a placement writes, each folded; `None` after a diagnostic.
    fn lower_given(&mut self, file: FileId, module: DeclId, fields: &[FieldInit]) -> Option<Vec<(u32, Constant, ExprId)>> {
        let params: Vec<DeclId> = self.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        let mut ok = true;
        for field in fields {
            let index = match field.res {
                Resolution::Decl(d) => params.iter().position(|p| *p == d),
                _ => None,
            };
            let Some(index) = index else {
                self.unsupported(file, field.span, &format!("a placement that sets `{}`", field.name.text));
                ok = false;
                continue;
            };
            if !seen.insert(index) {
                self.report(file, Diagnostic::new("E0320", format!("the param `{}` is given twice", field.name.text), field.span, "param given twice"));
                ok = false;
                continue;
            }
            match self.consts.value(field.value, &Bindings::new()) {
                Ok(value) => out.push((index as u32, value, field.value)),
                Err(fault) => {
                    self.const_fault(&fault, "a param value");
                    ok = false;
                }
            }
        }
        ok.then_some(out)
    }
}
