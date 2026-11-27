//! Modules with a result (DIG-06, part III §8.3): `module name(inputs) -> T { items result }`, its
//! result of type `T` (`E0301`), its placement in an expression, the value inputs it takes. A module-body
//! `let` that reads itself is `E0332`, or `E0353` when the cycle passes through a result that depends
//! directly on its input (§8.3.5).

use crate::refusal::forms::StructureForm;
use crate::msl::Span;
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, FileId, ModuleData, Resolution};

use super::check::{Checker, Context};
use super::{Callee, Ty};

impl Checker<'_> {
    /// The result of a module: `-> T` and the trailing expression have the type `T` (`E0301`).
    pub(super) fn module_result(&mut self, decl: DeclId, module: &ModuleData) {
        let data = self.design.decl_data(decl);
        let (file, name_span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
        match (module.ret, module.result) {
            (Some(ret), Some(result)) => match self.lower_type(ret) {
                Some(ty) => self.check_as(result, &ty, "E0301"),
                None => {
                    self.infer_event_position(result);
                }
            },
            (Some(_), None) => {
                let message = "a module declared with `-> T` ends its body with no result expression (§8 rule 3)".to_string();
                self.coded(file, "E0301", name_span, message, "no result");
            }
            (None, Some(result)) => {
                let span = self.design.expr_node(result).span;
                self.refuse(file, span, StructureForm::ModuleResult);
                self.infer(result);
            }
            (None, None) => {}
        }
    }

    /// The type a placement of `module` has: its result type, or the module itself.
    pub(super) fn placement_type(&mut self, module: DeclId) -> Option<Ty> {
        let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return None };
        match m.ret {
            Some(ret) => self.lower_type(ret),
            None => Some(Ty::module(module)),
        }
    }

    /// The name `instance` of a placement of `module`, read as a value: an instance of a module
    /// without a result has no value, `E0333` naming it (§8.2.14). One with a result is that value.
    pub(super) fn instance_as_value(&mut self, file: FileId, span: Span, instance: DeclId, module: DeclId) -> Option<Ty> {
        let has_result = matches!(&self.design.decl_data(module).kind, DeclKind::Module(m) if m.ret.is_some());
        if has_result {
            return Some(Ty::module(module));
        }
        let name = self.design.decl_data(instance).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("the instance `{name}` has no result: a module without a result is not a value (§8.2.14)");
        self.coded(file, "E0333", span, message, "an instance without a result used as a value");
        None
    }

    /// Whether the `let` name `binding` names an instance: its `let`'s value (through parentheses) is a placement of a module
    /// (§8.2.14), not a template (§8.4.1).
    pub(super) fn binds_an_instance(&self, binding: DeclId) -> bool {
        let Some(parent) = self.design.decl_data(binding).parent else { return false };
        let DeclKind::Let(data) = &self.design.decl_data(parent).kind else { return false };
        let mut value = data.value;
        while let ExprKind::Paren(inner) = &self.design.expr_node(value).kind {
            value = *inner;
        }
        matches!(self.callees.get(&value), Some(Callee::Module(_)))
    }

    /// `E0354` (§8.3.10): the name `binding` of a continuous result (a `let`, or a placement's
    /// result) read in a const context (a param default, a `where`, a `const` initializer), where no
    /// probe may be written. Reports and returns true when it is one.
    pub(super) fn continuous_result_in_const(&mut self, file: FileId, span: Span, binding: DeclId) -> bool {
        if self.context != Context::Const {
            return false;
        }
        let Some(init) = self.binding_init(binding) else { return false };
        if !self.continuous(init) {
            return false;
        }
        let name = self.design.decl_data(binding).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("the continuous result `{name}` is read in a const context, where no probe may be written (§8.3.10)");
        self.coded(file, "E0354", span, message, "a continuous result where probes are not allowed");
        self.poisoned.insert(binding);
        true
    }

    /// `E0354` (§8.3.10): the placement `expr` has an `Event` result and stands outside an event
    /// position. Returns false for any other expression.
    pub(super) fn event_result_out_of_place(&mut self, expr: ExprId) -> bool {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Call { class: CallClass::Instance { module: Some(module), .. }, .. } = &node.kind else { return false };
        let name = self.design.decl_data(*module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let file = self.file_of_expr(expr);
        let message = format!("the `Event` result of `{name}` is read outside an event position, where an event could not be written (§8.3.10)");
        self.coded(file, "E0354", node.span, message, "an `Event` result outside an event position");
        true
    }

    /// Whether `decl` is a value input: a port whose type is a value, not a nature.
    pub(super) fn is_value_input(&self, decl: DeclId) -> bool {
        matches!(&self.design.decl_data(decl).kind, DeclKind::Port(p) if self.design.nature_of(p.ty).is_none()) && !self.design.is_bundle_owner(decl)
    }

    /// The value inputs of `module`, in port order.
    fn value_inputs(&self, module: DeclId) -> Vec<DeclId> {
        self.design.decl_data(module).children.iter().copied().filter(|c| self.is_value_input(*c)).collect()
    }

    /// Whether `expr` reads the declaration `target`, directly or through a `let`.
    pub(super) fn reads_decl(&self, expr: ExprId, target: DeclId, followed: &mut Vec<ExprId>) -> bool {
        let node = self.design.expr_node(expr);
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(d)) = self.design.path_node(*path).segments.last().map(|s| s.res)
        {
            if d == target {
                return true;
            }
            if let Some(init) = self.binding_init(d)
                && !followed.contains(&init)
            {
                followed.push(init);
                let found = self.reads_decl(init, target, followed);
                followed.pop();
                return found;
            }
        }
        crate::pom::expr_children(&node.kind, self.design).into_iter().any(|c| self.reads_decl(c, target, followed))
    }

    // ---- a `let` that reads itself (§8.2.16, §8.3.5) -----------------------------------------

    /// Whether the result of `module` reads one of its value inputs (rather than only its own
    /// carriers): a direct dependence (§8.3.5).
    fn result_depends_directly(&self, module: DeclId) -> bool {
        let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return false };
        let Some(result) = m.result else { return false };
        self.value_inputs(module).into_iter().any(|input| self.reads_decl(result, input, &mut Vec::new()))
    }

    /// A read of the module-body `let` `binding` while its own value is typed (§8.2.16, §8.3.5):
    /// `E0332` outside a placement; inside one, `E0353` when every result on the cycle has a direct
    /// dependence (a core module's result always has one, §8.3.6), else the type of a feedback
    /// through an unknown: the declared result type of the placement the `let` is bound to.
    pub(super) fn self_read(&mut self, file: FileId, span: Span, binding: DeclId) -> Option<Ty> {
        let name = self.design.decl_data(binding).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        if self.placing.is_empty() {
            self.coded(file, "E0332", span, format!("the `let` `{name}` reads its own name (§8.2.16)"), "a `let` that reads itself");
        } else if self.placing.iter().all(|p| p.is_none_or(|m| self.result_depends_directly(m))) {
            let cycle = self.cycle_names(&name);
            let message = format!("the `let` `{name}` reads itself through results that all depend directly on their input: the cycle {cycle} has no unknown in it (§8.3.5)");
            self.coded(file, "E0353", span, message, "a cycle with no unknown");
        } else if let Some(ty) = self.feedback_type(binding) {
            return Some(ty);
        } else if self.placing.iter().all(|placed| placed.is_some_and(|module| !self.has_result(module))) {
            // through placements of modules with no result there is no instance value to feed back (§8.3.5): the `let`s read each
            // other's values (`let p = D(x) { k: q.k }; let q = D(x) { k: p.k };`); a node connection is no such read (`reach/fields.rs`)
            let cycle = self.cycle_names(&name);
            let message = format!("the `let` `{name}` reads its own name through other `let`s, {cycle}, and no module on the way has a result to feed back (§8.2.16)");
            self.coded(file, "E0332", span, message, "a `let` that reads itself");
        } else {
            self.refuse_with(file, span, StructureForm::FeedbackOfThroughInstanceWhoseValueIs, [name.to_string()]);
        }
        self.poisoned.insert(binding);
        None
    }

    /// Whether `module` declares a result (`-> T`), the value an instance of it feeds back (§8.3.5).
    fn has_result(&self, module: DeclId) -> bool {
        matches!(&self.design.decl_data(module).kind, DeclKind::Module(data) if data.ret.is_some())
    }

    /// The names of the cycle `let` -> the placed modules -> `let`, as `y -> twice -> y`.
    fn cycle_names(&self, binding: &str) -> String {
        let mut names = vec![binding.to_string()];
        names.extend(self.placing.iter().map(|p| p.map_or("a core module".to_string(), |m| self.design.decl_data(m).name.as_ref().map_or(String::new(), |n| n.text.clone()))));
        names.push(binding.to_string());
        names.join(" -> ")
    }

    /// The type of the `let` `binding` that feeds back through an unknown: the declared result type of
    /// the placement that is the whole of its value (parentheses aside). `None` for a value that only
    /// contains the placement, or a core module, whose result type waits for its arguments.
    fn feedback_type(&mut self, binding: DeclId) -> Option<Ty> {
        let mut value = self.lets_open.get(&binding).copied()?;
        while let ExprKind::Paren(inner) = &self.design.expr_node(value).kind {
            value = *inner;
        }
        let module = match self.callees.get(&value) {
            Some(Callee::Module(module)) => *module,
            _ => return None,
        };
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        self.lower_type(data.ret?)
    }
}
