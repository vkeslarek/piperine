//! The placements a module writes: each one a `Child` of its body, the tree the device author
//! wrote. A child names its module's body, the terminals and carriers of the parent it connects,
//! and the param values the placement gives: a constant where the value folds alone, else the
//! expression over the parent's params (`ChildValue::Expr`, beside the child, contract D4), which
//! placement evaluates per instance into the flat table (DIG-09 AC 11).

use std::collections::BTreeSet;

use piperine_ir::{Child, ChildValue, Constant, NodeRole, Terminal, ValueType};

use super::consts::{Bindings, ConstFault};
use super::elaborator::Elaboration;
use super::generation::Iteration;
use super::instances::Instance;
use super::lower::Scope;
use super::templates::TemplateValue;
use super::unlowered::Unlowered;
use crate::msl::Diagnostic;
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, Resolution, StructClass};
use crate::types::Ty;

/// A placement, as instance placement needs it.
#[derive(Clone)]
pub(super) struct ChildPlan {
    pub(super) name: String,
    pub(super) module: DeclId,
    /// The arguments of the instance of a generic module the child places.
    pub(super) instance: Option<Instance>,
    /// The body the child places: the variant of the module its key params select.
    pub(super) body: piperine_ir::BodyId,
    pub(super) terminals: Vec<Terminal>,
    /// What each carrier port of the child's body is bound to, in port order: a carrier of the
    /// parent's body.
    pub(super) carriers: Vec<Option<u32>>,
    /// The param values written, as (index in the child's params, the value when it folds alone,
    /// the value's expression): one that reads the parent's params is evaluated per instance.
    pub(super) given: Vec<(u32, Option<Constant>, ExprId)>,
    /// The call expression of the placement.
    pub(super) call: ExprId,
}

impl Elaboration<'_, '_> {
    /// Gives every authored child the node slots of its body that are no port, now that every body is
    /// built (`Ir::check` asks for one slot per node of the child's body): each names the child's own
    /// node, `Terminal::Own(k)` with `k` the node's index in the child's body, so the authored tree
    /// shows the child's signals and never ground where a node exists (S4, AS-16). A node the child
    /// reaches into a grandchild (`NodeRole::Reached`) is the child's own view of it, so its slot
    /// names it too.
    pub(super) fn pad_child_nodes(&mut self) {
        let nodes: std::collections::BTreeMap<piperine_ir::BodyId, Vec<NodeRole>> = self.plans.values().map(|plan| (plan.body, plan.nodes.iter().map(|n| n.role).collect())).collect();
        let slots: Vec<(piperine_ir::BodyId, u32, Vec<Terminal>)> = self
            .plans
            .values()
            .flat_map(|plan| {
                plan.children.iter().enumerate().filter_map(|(index, child)| {
                    let roles = nodes.get(&child.body)?;
                    let mut slots = child.terminals.clone();
                    for (slot, role) in roles.iter().enumerate().skip(slots.len()) {
                        slots.push(if *role == NodeRole::Port { Terminal::Ground } else { Terminal::Own(slot as u32) });
                    }
                    Some((plan.body, index as u32, slots))
                })
            })
            .collect();
        for (body, child, nodes) in slots {
            self.ir.set_child_nodes(body, child, nodes);
        }
    }

    /// The placements of `module` in source order, each as the `Child` of its body and the plan
    /// placement reads. A placement of a module the type checker did not reach is skipped: it
    /// was refused already.
    pub(super) fn lower_children(&mut self, scope: &mut Scope, module: DeclId) -> Vec<(Child, ChildPlan)> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let calls = data.instances.clone();
        let mut ordinals: Vec<(DeclId, u32)> = Vec::new();
        let mut out = Vec::new();
        for call in calls {
            let Some(child_module) = self.placed_module(call, &scope.keys) else { continue };
            // A module with a result or a value input is hoisted into this body, not placed (`hoist.rs`).
            if self.hoisted_callee(call).is_some() {
                continue;
            }
            // a placement in generation is one child per iteration, named by its ordinals (`sec[1]`); an arm not elaborated has none
            let owner = self.design.expr_node(call).owner;
            let iterations = match self.generated.of(owner) {
                Some(iterations) => iterations.clone(),
                None => vec![Iteration::default()],
            };
            for iteration in &iterations {
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
                for (variable, value) in &iteration.values {
                    scope.unrolled.bind(*variable, Constant::Int(*value));
                }
                scope.ordinals = iteration.ordinals.clone();
                let lowered = self.lower_child(scope, call, child_module, ordinal, out.len() as u32);
                scope.ordinals.clear();
                for (variable, _) in &iteration.values {
                    scope.unrolled.unbind(*variable);
                }
                if let Some((mut child, mut plan)) = lowered {
                    let suffix = iteration.subscripts();
                    child.name.push_str(&suffix);
                    plan.name.push_str(&suffix);
                    out.push((child, plan));
                }
            }
        }
        out
    }

    /// The `{ name: value }` fields the placement `call` of `module` writes, in source order: the
    /// fields of the struct template it calls, else the block after the call.
    pub(super) fn placement_fields(&self, call: ExprId, module: DeclId) -> Vec<FieldInit> {
        let ExprKind::Call { callee, params, .. } = &self.design.expr_node(call).kind else { return Vec::new() };
        match &self.design.expr_node(*callee).kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module => fields.clone(),
            _ => params.clone().unwrap_or_default(),
        }
    }

    /// The values the key params of `module` take at the placement written with `fields`: the value
    /// written, evaluated over the key params of the placing body (`scope.keys`), else the default.
    /// `None` after a diagnostic (`E0319` for a key param with neither).
    pub(super) fn child_key_values(&mut self, scope: &Scope, (module, instance): (DeclId, Option<&Instance>), fields: &[FieldInit], file: FileId, span: crate::msl::Span) -> Option<Bindings> {
        let mut values = Bindings::new();
        for key in self.keys.of(module).to_vec() {
            let written = match fields.iter().find(|f| f.res == Resolution::Decl(key)) {
                Some(field) => match self.fold_param_value(field.value, &scope.keys.overlaid(&scope.unrolled)) {
                    Ok(value) => Some(value),
                    Err(fault) => {
                        self.const_fault(&fault, "the value of a param that fixes structure");
                        return None;
                    }
                },
                None => None,
            };
            let value = self.key_value((module, instance), key, written, &values, (file, span))?;
            self.bind_param(&mut values, key, value);
        }
        Some(values)
    }

    /// The value key param `key` of `module` takes at a placement: `written`, else the default, folded over the key params
    /// before it (`earlier`) when it reads them (§8.1.16). `None` after `E0319` for a key param with neither, or after the
    /// diagnostic of a default that does not fold.
    fn key_value(&mut self, (module, instance): (DeclId, Option<&Instance>), key: DeclId, written: Option<Constant>, earlier: &Bindings, (file, span): (FileId, crate::msl::Span)) -> Option<Constant> {
        // an instance of a generic module has the plans of its own arguments (`generic_bodies.rs`)
        if let Some(instance) = instance {
            self.plan_instance_params(module, instance);
        }
        let plan = self.plans_of(module, instance).and_then(|plans| plans.iter().find(|p| p.decl == key)).cloned();
        let default = match (written, plan) {
            (None, Some(plan)) => match self.default_at(&plan, earlier) {
                Ok(default) => default,
                Err(fault) => {
                    self.const_fault(&fault, "a param default");
                    return None;
                }
            },
            _ => None,
        };
        match written.or(default) {
            Some(value) => Some(value),
            None => {
                let name = self.design.decl_data(key).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.report(file, Diagnostic::new("E0319", format!("the placement leaves the required param `{name}` unbound"), span, "required param not bound"));
                None
            }
        }
    }

    /// The template a placement through a template value places (§8.4.4, §8.4.9): the value the param of module type holds in the
    /// body being lowered (`cell(c)`, PI-04: the default or the parent's binding), or the value the head of the placement folds to
    /// over the key params (`C1(c)`, `nch(l, w)(d, g, s, b)`). `Ok(None)` when `class` is a placement of a module by name or of a
    /// template written in place; `Err` after the diagnostic of a head that does not fold.
    pub(super) fn template_through(&mut self, scope: &Scope, call: ExprId, class: &CallClass) -> Result<Option<TemplateValue>, ()> {
        let CallClass::Instance { module: placed, via: Some(via) } = class else { return Ok(None) };
        if placed.is_some_and(|placed| *via == placed) {
            return Ok(None);
        }
        if self.is_module_param(*via) {
            return Ok(scope.keys.get(*via).and_then(|number| self.consts.templates().get(number)).cloned());
        }
        let ExprKind::Call { callee, .. } = &self.design.expr_node(call).kind else { return Ok(None) };
        let folded = self.fold_param_value(*callee, &scope.keys.overlaid(&scope.unrolled));
        match folded {
            Ok(number) => Ok(self.consts.templates().get(number).cloned()),
            Err(fault) => {
                self.const_fault(&fault, "the head of a placement through a template");
                Err(())
            }
        }
    }

    /// The values the key params of `module` take at the placement `call`: the template's when the placement goes through one,
    /// else the ones its block writes (`child_key_values`). `None` after a diagnostic.
    pub(super) fn placement_key_values(&mut self, scope: &Scope, call: ExprId, module: DeclId, span: crate::msl::Span) -> Option<Bindings> {
        let ExprKind::Call { class, .. } = self.design.expr_node(call).kind.clone() else { return None };
        let instance = if self.is_generic_module(module) { Some(self.placement_instance(scope, call, module)?) } else { None };
        match self.template_through(scope, call, &class) {
            Err(()) => None,
            Ok(Some(template)) => self.template_values(&template, instance.as_ref(), scope.file, span).map(|(_, keys)| keys),
            Ok(None) => {
                let fields = self.placement_fields(call, module);
                self.child_key_values(scope, (module, instance.as_ref()), &fields, scope.file, span)
            }
        }
    }

    /// What a placement through a template gives its module: the constant of each param the block
    /// binds, as `lower_child` keeps them, and the values of the module's key params.
    fn template_values(&mut self, template: &TemplateValue, instance: Option<&Instance>, file: FileId, span: crate::msl::Span) -> Option<(Vec<(u32, Option<Constant>, ExprId)>, Bindings)> {
        let params: Vec<DeclId> = self.design.decl_data(template.module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        let given: Vec<(u32, Option<Constant>, ExprId)> = template.fields.iter().filter_map(|(param, value, expr)| Some((params.iter().position(|p| p == param)? as u32, Some(*value), *expr))).collect();
        let mut keys = Bindings::new();
        for key in self.keys.of(template.module).to_vec() {
            let written = template.fields.iter().find(|(param, _, _)| *param == key).map(|(_, value, _)| *value);
            let value = self.key_value((template.module, instance), key, written, &keys, (file, span))?;
            self.bind_param(&mut keys, key, value);
        }
        Some((given, keys))
    }

    fn lower_child(&mut self, scope: &Scope, call: ExprId, module: DeclId, ordinal: u32, index: u32) -> Option<(Child, ChildPlan)> {
        let node = self.design.expr_node(call).clone();
        let file = self.design.decl_data(node.owner).file;
        let ExprKind::Call { args, class, .. } = &node.kind else { return None };
        let placed = match class {
            CallClass::Instance { module: Some(m), .. } => *m == module,
            CallClass::Instance { module: None, via: Some(via) } => self.is_module_param(*via),
            _ => false,
        };
        if !placed {
            self.unsupported(file, node.span, "a placement through a template value");
            return None;
        }
        let Ok(through) = self.template_through(scope, call, class) else { return None };
        let fields = if through.is_some() { Vec::new() } else { self.placement_fields(call, module) };
        let module_name = self.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let name = self.placement_name(call, &module_name, ordinal);
        let mut ok = true;
        let mut terminals = Vec::new();
        let mut carriers = Vec::new();
        let mut windows = Vec::new();
        let ports: Vec<DeclId> = self.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_))).collect();
        for (port, arg) in ports.iter().zip(args) {
            if self.types.node_array(*port).is_some() {
                match self.node_array_terminals(scope, arg.value) {
                    Some(nodes) => terminals.extend(nodes),
                    None => ok = false,
                }
                continue;
            }
            // a port of an array of length 0 has no carrier in the child, so it binds nothing (§1.2.3)
            if self.types.carrier_array(*port).is_some_and(Ty::has_fixed_zero_length) {
                continue;
            }
            if self.types.carrier(*port).is_some() {
                match self.lower_carrier_argument(scope, &name, *port, arg.value) {
                    Some((carrier, window)) => {
                        if matches!(&self.design.decl_data(*port).kind, DeclKind::Port(data) if data.dir == Some(crate::pom::Dir::Out)) {
                            self.structure.handed.push((carrier, window));
                            let span = self.design.expr_node(arg.value).span;
                            let reach = super::drive_overlaps::ElementReach::of_window(window);
                            self.structure.drive_sites.push(super::drive_overlaps::DriveSite { carrier, reach, fields: Vec::new(), way: super::drive_overlaps::DriveWay::Handed, file: scope.file, span });
                        }
                        carriers.push(Some(carrier));
                        windows.push(window);
                    }
                    None => ok = false,
                }
                continue;
            }
            match self.lower_terminal(scope, arg.value) {
                // A reached node is bound after its child is placed, which a sibling placed before
                // it could not read.
                Some(Terminal::Node(at)) if self.structure.nodes[at as usize].1.role == piperine_ir::NodeRole::Reached => {
                    self.unsupported(scope.file, self.design.expr_node(arg.value).span, "a node of a child connected to another placement");
                    ok = false;
                }
                Some(terminal) => terminals.push(terminal),
                None => ok = false,
            }
        }
        let instance = if self.is_generic_module(module) { Some(self.placement_instance(scope, call, module)?) } else { None };
        let templated = match &through {
            Some(template) => match self.template_values(template, instance.as_ref(), file, node.span) {
                Some(values) => Some(values),
                None => return None,
            },
            None => None,
        };
        let given = match &templated {
            Some((given, _)) => Some(given.clone()),
            None => self.lower_given(scope, file, module, index, &fields),
        };
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
        let key_values = match templated {
            Some((_, keys)) => keys,
            None => self.child_key_values(scope, (module, instance.as_ref()), &fields, file, node.span)?,
        };
        let mut shapes_agree = true;
        for (port, arg) in ports.iter().zip(args) {
            shapes_agree &= self.check_array_argument(scope, *port, arg.value, &key_values);
        }
        if !shapes_agree {
            return None;
        }
        let (_, body) = self.request_body(module, instance.as_ref(), &key_values)?;
        let params = given.iter().filter_map(|(index, value, _)| value.map(|v| (*index, v))).collect();
        let origin = self.locator.origin(file, node.span);
        // The slots of the child's own nodes are added once every body is built (`pad_child_nodes`).
        let child = Child { name: name.clone(), body, nodes: terminals.clone(), params, origin };
        // The authored child shows the carriers it connects (ir-rust "Left for msl-digital").
        if !carriers.is_empty() {
            self.structure.child_carriers.push((index, carriers.clone()));
        }
        // the part of an array of carriers each port is given (`elements.rs`)
        if windows.iter().any(Option::is_some) {
            self.structure.child_windows.push((index, windows));
        }
        Some((child, ChildPlan { name, module, instance, body, terminals, carriers, given, call }))
    }

    /// The carrier of the parent an argument at the carrier port `port` of the placement
    /// `instance` is bound to: the carrier it names, or for an expression at an `in` port the
    /// carrier of its anonymous definition (§8.2.9). `None` after a diagnostic.
    pub(super) fn lower_carrier_argument(&mut self, scope: &Scope, instance: &str, port: DeclId, arg: ExprId) -> Option<(u32, Option<piperine_ir::CarrierWindow>)> {
        let node = self.design.expr_node(arg).clone();
        // a carrier declared inside generation, named inside its loops, is the iteration's element (§8.2.23)
        let generated = !self.generated_carrier_levels(scope, arg).is_empty();
        if !generated
            && let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(index) = self.frames.get(scope.frame).and_then(|f| f.carriers.get(&decl))
        {
            return Some((*index, None));
        }
        // an element `x[k]` or a sub-array `g[r]` of an array of carriers: the carrier and its window
        if (generated || matches!(node.kind, ExprKind::Index { .. }))
            && let Some(found) = self.carrier_window(scope, arg)
        {
            return found;
        }
        match &self.design.decl_data(port).kind {
            DeclKind::Port(p) if p.dir == Some(crate::pom::Dir::In) => self.anonymous_definition(scope, instance, port, arg).map(|carrier| (carrier, None)),
            // The checker refuses an expression at an `out` port first (`E0314`).
            _ => {
                self.unsupported(scope.file, node.span, "an expression at an `out` carrier port");
                None
            }
        }
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

    /// The param values the placement `child` writes, each folded when it reads no param of the
    /// parent; one that does is lowered over the parent's params and kept beside the child as a
    /// `ChildValue::Expr`. `None` after a diagnostic.
    fn lower_given(&mut self, scope: &Scope, file: FileId, module: DeclId, child: u32, fields: &[FieldInit]) -> Option<Vec<(u32, Option<Constant>, ExprId)>> {
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
                self.refuse(file, field.span, Unlowered::PlacementSets(&field.name.text));
                ok = false;
                continue;
            };
            if !seen.insert(index) {
                self.report(file, Diagnostic::new("E0320", format!("the param `{}` is given twice", field.name.text), field.span, "param given twice"));
                ok = false;
                continue;
            }
            let bindings = if self.is_module_param(params[index]) { scope.keys.clone() } else { Bindings::new() }.overlaid(&scope.unrolled);
            match self.fold_param_value(field.value, &bindings) {
                Ok(value) => out.push((index as u32, Some(value), field.value)),
                // A value over the parent's params (`{ r: r_top }`, `{ l }`): the authored child
                // shows the expression, the instance table the constant of each instance.
                Err(ConstFault::UnboundParam { .. }) => {
                    let mut sub = Self::sub_scope(scope);
                    match self.lower_param_expression(&mut sub, field.value) {
                        Some(value) if sub.locals.is_empty() => {
                            self.structure.child_params.push((child, index as u32, ChildValue::Expr(value)));
                            out.push((index as u32, None, field.value));
                        }
                        Some(_) => {
                            self.unsupported(file, self.design.expr_node(field.value).span, "a `let` in a param value");
                            ok = false;
                        }
                        None => ok = false,
                    }
                }
                Err(fault) => {
                    self.const_fault(&fault, "a param value");
                    ok = false;
                }
            }
        }
        ok.then_some(out)
    }

    /// The value `expr` gives a param of the child when it reads the parent's params (§3.2.3): the expression itself, and for an
    /// `Option` param a value of `T`, or `Some(value)` written out, as `Some` of it (`OptionPack`). `None` after a diagnostic.
    fn lower_param_expression(&mut self, scope: &mut Scope, expr: ExprId) -> Option<piperine_ir::ExprId> {
        let payload = if self.types.implicit_some(expr) {
            Some(expr)
        } else if self.is_some_call(expr) {
            match &self.design.expr_node(expr).kind {
                ExprKind::Call { args, .. } => args.first().map(|arg| arg.value),
                _ => None,
            }
        } else {
            None
        };
        // an `Option` param of the parent passed on (`{ rs: rs }`): its two halves, packed again
        if payload.is_none() && matches!(self.typed(expr).as_ref(), Some(crate::types::Ty::Option(_))) {
            let pair = self.lower_option(scope, expr)?;
            let origin = self.expr_origin(expr);
            let held = self.type_of(pair.val);
            return Some(self.emit(piperine_ir::Expr::OptionPack { is: pair.is, value: pair.val }, ValueType::Option(Box::new(held)), origin));
        }
        let Some(payload) = payload else { return self.lower_expr(scope, expr) };
        let value = self.lower_expr(scope, payload)?;
        let origin = self.expr_origin(expr);
        let is = self.emit(piperine_ir::Expr::Bool(true), ValueType::Bool, origin.clone());
        let held = self.type_of(value);
        Some(self.emit(piperine_ir::Expr::OptionPack { is, value }, ValueType::Option(Box::new(held)), origin))
    }
}
