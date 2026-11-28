//! The placements a module writes: each one a `Child` of its body, the tree the device author
//! wrote. A child names its module's body, the terminals and carriers of the parent it connects,
//! and the param values the placement gives: a constant where the value folds alone, else the
//! expression over the parent's params (`ChildValue::Expr`, beside the child, contract D4), which
//! placement evaluates per instance into the flat table (DIG-09 AC 11).

use super::StructureLowering;
use std::collections::BTreeSet;

use piperine_ir::{Child, ChildValue, Constant, NodeRole, Terminal, ValueType};

use super::bundles::PortBindings;
use crate::elab::consts::{Bindings, ConstFault};
use super::generation::Iteration;
use crate::elab::functions::instances::Instance;
use crate::elab::elaborator::Scope;
use super::templates::TemplateValue;
use crate::refusal::forms::StructureForm;
use crate::msl::Diagnostic;
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, Resolution, StructClass};
use crate::types::Ty;

/// A placement, as instance placement needs it.
#[derive(Clone)]
pub(in crate::elab) struct ChildPlan {
    pub(in crate::elab) name: String,
    pub(in crate::elab) module: DeclId,
    /// The arguments of the instance of a generic module the child places.
    pub(in crate::elab) instance: Option<Instance>,
    /// The body the child places: the variant of the module its key params select.
    pub(in crate::elab) body: piperine_ir::BodyId,
    pub(in crate::elab) terminals: Vec<Terminal>,
    /// What each carrier port of the child's body is bound to, in port order: a carrier of the
    /// parent's body.
    pub(in crate::elab) carriers: Vec<Option<u32>>,
    /// The param values written, as (index in the child's params, the value when it folds alone,
    /// the value's expression): one that reads the parent's params is evaluated per instance.
    pub(in crate::elab) given: Vec<(u32, Option<Constant>, ExprId)>,
    /// The call expression of the placement.
    pub(in crate::elab) call: ExprId,
    /// The loop variables of the generation the placement is written in, each the iteration's constant: what a value of `given`
    /// that reads the parent's params reads besides them (`k: m + (j as f64)`).
    pub(in crate::elab) unrolled: Bindings,
}

impl StructureLowering<'_, '_, '_> {
    /// Gives every authored child the node slots of its body that are no port, now that every body is
    /// built (`Ir::check` asks for one slot per node of the child's body): each names the child's own
    /// node, `Terminal::Own(k)` with `k` the node's index in the child's body, so the authored tree
    /// shows the child's signals and never ground where a node exists (S4, AS-16). A node the child
    /// reaches into a grandchild (`NodeRole::Reached`) is the child's own view of it, so its slot
    /// names it too.
    pub(in crate::elab) fn pad_child_nodes(&mut self) {
        let nodes: std::collections::BTreeMap<piperine_ir::BodyId, Vec<NodeRole>> = self.state.structure.plans.values().map(|plan| (plan.body, plan.nodes.iter().map(|n| n.role).collect())).collect();
        let slots: Vec<(piperine_ir::BodyId, u32, Vec<Terminal>)> = self
            .state.structure.plans
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
            let built = self.state.builder.set_child_nodes(body, child, nodes);
            self.built(built);
        }
    }

    /// The placements of `module` in source order, each as the `Child` of its body and the plan
    /// placement reads. A placement of a module the type checker did not reach is skipped: it
    /// was refused already.
    pub(super) fn lower_children(&mut self, scope: &mut Scope, module: DeclId) -> Vec<(Child, ChildPlan)> {
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return Vec::new() };
        let calls = data.instances.clone();
        let mut ordinals: Vec<(DeclId, u32)> = Vec::new();
        let mut out = Vec::new();
        for call in calls {
            let Some(child_module) = self.placed_module(call, &scope.values.keys) else { continue };
            // A module with a result or a value input is hoisted into this body, not placed (`hoist.rs`).
            if self.hoisted_callee(call).is_some() {
                continue;
            }
            // a placement in generation is one child per iteration, named by its ordinals (`sec[1]`); an arm not elaborated has none
            let owner = self.state.design.expr_node(call).owner;
            let iterations = match self.state.structure.generated.of(owner) {
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
                    scope.values.unrolled.bind(*variable, Constant::Int(*value));
                }
                scope.digital.ordinals = iteration.ordinals.clone();
                let lowered = self.lower_child(scope, call, child_module, ordinal, out.len() as u32);
                scope.digital.ordinals.clear();
                for (variable, _) in &iteration.values {
                    scope.values.unrolled.unbind(*variable);
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
        let ExprKind::Call { callee, params, .. } = &self.state.design.expr_node(call).kind else { return Vec::new() };
        // `(Cell { cs: 2.0 })(x)`: the template written in place, through parentheses
        match &self.state.design.expr_node(self.peel_parens(*callee)).kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module => fields.clone(),
            _ => params.clone().unwrap_or_default(),
        }
    }

    /// The values the key params of `module` take at the placement written with `fields`: the value
    /// written, evaluated over the key params of the placing body (`scope.keys`), else the default.
    /// `None` after a diagnostic (`E0319` for a key param with neither).
    pub(super) fn child_key_values(&mut self, scope: &Scope, (module, instance): (DeclId, Option<&Instance>), fields: &[FieldInit], file: FileId, span: crate::msl::Span) -> Option<Bindings> {
        let mut values = Bindings::new();
        for key in self.state.structure.keys.of(module).to_vec() {
            let written = match fields.iter().find(|f| f.res == Resolution::Decl(key)) {
                Some(field) => match self.fold_param_value(field.value, &scope.values.keys.overlaid(&scope.values.unrolled)) {
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
                let name = self.state.design.decl_data(key).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.report(file, Diagnostic::new("E0319", format!("the placement leaves the required param `{name}` unbound"), span, "required param not bound"));
                None
            }
        }
    }

    /// Whether the placement `call` is written through an expression that is no name (`(if c { .. } else { .. })(x)`): a choice of
    /// templates, whose head folds at elaboration (§8.4.6).
    pub(super) fn has_an_expression_head(&self, call: ExprId) -> bool {
        let ExprKind::Call { callee, class: CallClass::Instance { module: Some(_), via: None }, .. } = &self.state.design.expr_node(call).kind else { return false };
        matches!(self.state.design.expr_node(self.peel_parens(*callee)).kind, ExprKind::If { .. } | ExprKind::Match { .. } | ExprKind::Block(_) | ExprKind::Index { .. })
    }

    /// The template a placement through a template value places (§8.4.4, §8.4.9): the value the param of module type holds in the
    /// body being lowered (`cell(c)`, PI-04: the default or the parent's binding), or the value the head of the placement folds to
    /// over the key params (`C1(c)`, `nch(l, w)(d, g, s, b)`). `Ok(None)` when `class` is a placement of a module by name or of a
    /// template written in place; `Err` after the diagnostic of a head that does not fold.
    pub(super) fn template_through(&mut self, scope: &Scope, call: ExprId, class: &CallClass) -> Result<Option<TemplateValue>, ()> {
        let CallClass::Instance { module: placed, via } = class else { return Ok(None) };
        // a placement in the body of a hoisted module reads the params its own placement bound, not the keys of the body that holds it
        let keys = if scope.values.frame == 0 { scope.values.keys.clone() } else { self.frame_bindings(scope.values.frame) };
        match via {
            Some(via) if placed.is_some_and(|placed| *via == placed) => return Ok(None),
            Some(via) if self.is_module_param(*via) => return Ok(keys.get(*via).and_then(|number| self.state.consts.templates().get(number)).cloned()),
            Some(_) => {}
            // a choice of templates written as the head folds as any head does; a module named by its path places by name
            None if !self.has_an_expression_head(call) => return Ok(None),
            None => {}
        }
        let ExprKind::Call { callee, .. } = &self.state.design.expr_node(call).kind else { return Ok(None) };
        let folded = self.fold_param_value(*callee, &keys.overlaid(&scope.values.unrolled));
        match folded {
            Ok(number) => Ok(self.state.consts.templates().get(number).cloned()),
            Err(fault) => {
                self.const_fault(&fault, "the head of a placement through a template");
                Err(())
            }
        }
    }

    /// The values the key params of `module` take at the placement `call`: the template's when the placement goes through one,
    /// else the ones its block writes (`child_key_values`). `None` after a diagnostic.
    pub(super) fn placement_key_values(&mut self, scope: &Scope, call: ExprId, module: DeclId, span: crate::msl::Span) -> Option<Bindings> {
        let ExprKind::Call { class, .. } = self.state.design.expr_node(call).kind.clone() else { return None };
        let instance = if self.is_generic_module(module) { Some(self.placement_instance(scope, call, module)?) } else { None };
        match self.template_through(scope, call, &class) {
            Err(()) => None,
            Ok(Some(template)) => self.template_values(&template, instance.as_ref(), scope.values.file, span).map(|(_, keys)| keys),
            Ok(None) => {
                let fields = self.placement_fields(call, module);
                self.child_key_values(scope, (module, instance.as_ref()), &fields, scope.values.file, span)
            }
        }
    }

    /// What a placement through a template gives its module: the constant of each param the block
    /// binds, as `lower_child` keeps them, and the values of the module's key params.
    fn template_values(&mut self, template: &TemplateValue, instance: Option<&Instance>, file: FileId, span: crate::msl::Span) -> Option<(Vec<(u32, Option<Constant>, ExprId)>, Bindings)> {
        let params: Vec<DeclId> = self.state.design.decl_data(template.module).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        let constants = match self.template_constants(template) {
            Ok(constants) => constants,
            Err(fault) => {
                self.const_fault(&fault, "a param value of a template");
                return None;
            }
        };
        let given: Vec<(u32, Option<Constant>, ExprId)> = constants.iter().filter_map(|(param, value, expr)| Some((params.iter().position(|p| p == param)? as u32, Some(*value), *expr))).collect();
        let mut keys = Bindings::new();
        for key in self.state.structure.keys.of(template.module).to_vec() {
            let written = constants.iter().find(|(param, _, _)| *param == key).map(|(_, value, _)| *value);
            let value = self.key_value((template.module, instance), key, written, &keys, (file, span))?;
            self.bind_param(&mut keys, key, value);
        }
        Some((given, keys))
    }

    fn lower_child(&mut self, scope: &Scope, call: ExprId, module: DeclId, ordinal: u32, index: u32) -> Option<(Child, ChildPlan)> {
        let node = self.state.design.expr_node(call).clone();
        let file = self.state.design.decl_data(node.owner).file;
        let ExprKind::Call { args, class, .. } = &node.kind else { return None };
        let placed = match class {
            CallClass::Instance { module: Some(m), .. } => *m == module,
            CallClass::Instance { module: None, via: Some(via) } => self.is_module_param(*via),
            _ => false,
        };
        if !placed {
            self.refuse(file, node.span, StructureForm::PlacementThroughTemplateValue);
            return None;
        }
        let Ok(through) = self.template_through(scope, call, class) else { return None };
        let fields = if through.is_some() { Vec::new() } else { self.placement_fields(call, module) };
        let module_name = self.state.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let name = self.placement_name(call, &module_name, ordinal);
        let mut ok = true;
        let mut bound = PortBindings { terminals: Vec::new(), carriers: Vec::new(), windows: Vec::new() };
        let ports: Vec<DeclId> = self.state.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Port(_))).collect();
        for (port, arg) in ports.iter().zip(args) {
            // a bundle port is given each field in turn (`bundles.rs`)
            if self.state.design.is_bundle_owner(*port) {
                ok &= self.bind_bundle_port(scope, &name, *port, arg.value, &mut bound);
                continue;
            }
            let (terminals, carriers, windows) = (&mut bound.terminals, &mut bound.carriers, &mut bound.windows);
            if self.state.types.node_array(*port).is_some() {
                match self.digital().node_array_terminals(scope, arg.value) {
                    Some(nodes) => terminals.extend(nodes),
                    None => ok = false,
                }
                continue;
            }
            // a port of an array of length 0 has no carrier in the child, so it binds nothing (§1.2.3)
            if self.state.types.carrier_array(*port).is_some_and(Ty::has_fixed_zero_length) {
                continue;
            }
            if self.state.types.carrier(*port).is_some() {
                match self.lower_carrier_argument(scope, &name, *port, arg.value) {
                    Some((carrier, window)) => {
                        if matches!(&self.state.design.decl_data(*port).kind, DeclKind::Port(data) if data.dir == Some(crate::pom::Dir::Out)) {
                            self.state.digital.structure.handed.push((carrier, window));
                            let span = self.state.design.expr_node(arg.value).span;
                            let reach = crate::elab::digital::drive_overlaps::ElementReach::of_window(window);
                            self.state.digital.structure.drive_sites.push(crate::elab::digital::drive_overlaps::DriveSite { carrier, reach, fields: Vec::new(), way: crate::elab::digital::drive_overlaps::DriveWay::Handed, file: scope.values.file, span });
                        }
                        carriers.push(Some(carrier));
                        windows.push(window);
                    }
                    None => ok = false,
                }
                continue;
            }
            // a reached node (`c0.sn`) is bound once its child is placed; placement places that child first (`place.rs`)
            match self.analog().lower_terminal(scope, arg.value) {
                Some(terminal) => terminals.push(terminal),
                None => ok = false,
            }
        }
        let PortBindings { terminals, carriers, windows } = bound;
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
            shapes_agree &= self.digital().check_array_argument(scope, *port, arg.value, &key_values);
        }
        if !shapes_agree {
            return None;
        }
        let (_, body) = self.request_body(module, instance.as_ref(), &key_values)?;
        let params = given.iter().filter_map(|(index, value, _)| value.map(|v| (*index, v))).collect();
        let origin = self.state.locator.origin(file, node.span);
        // The slots of the child's own nodes are added once every body is built (`pad_child_nodes`).
        let child = Child { name: name.clone(), body, nodes: terminals.clone(), params, origin };
        // The authored child shows the carriers it connects (ir-rust "Left for msl-digital").
        if !carriers.is_empty() {
            self.state.digital.structure.child_carriers.push((index, carriers.clone()));
        }
        // the part of an array of carriers each port is given (`elements.rs`)
        if windows.iter().any(Option::is_some) {
            self.state.digital.structure.child_windows.push((index, windows));
        }
        Some((child, ChildPlan { name, module, instance, body, terminals, carriers, given, call, unrolled: scope.values.unrolled.clone() }))
    }

    /// The carrier of the parent an argument at the carrier port `port` of the placement
    /// `instance` is bound to: the carrier it names, or for an expression at an `in` port the
    /// carrier of its anonymous definition (§8.2.9). `None` after a diagnostic.
    pub(super) fn lower_carrier_argument(&mut self, scope: &Scope, instance: &str, port: DeclId, arg: ExprId) -> Option<(u32, Option<piperine_ir::CarrierWindow>)> {
        let node = self.state.design.expr_node(arg).clone();
        // a carrier declared inside generation, named inside its loops, is the iteration's element (§8.2.23)
        let generated = !self.digital().generated_carrier_levels(scope, arg).is_empty();
        // a field of a bundle of this module (`column.wl`) is a carrier of its own (`pom/bundles.rs`); a field of an element of an
        // array of bundles (`lanes[k].a`) is the window of that element of its leaf
        if let Some(place) = self.bundle_leaf(scope, arg)
            && let Some(index) = self.state.structure.frames.get(scope.values.frame).and_then(|f| f.carriers.get(&place.decl)).copied()
        {
            if place.elements.is_empty() {
                return Some((index, None));
            }
            return self.digital().carrier_window(scope, arg).flatten();
        }
        if !generated
            && let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(index) = self.state.structure.frames.get(scope.values.frame).and_then(|f| f.carriers.get(&decl))
        {
            return Some((*index, None));
        }
        // an element `x[k]` or a sub-array `g[r]` of an array of carriers: the carrier and its window
        if (generated || matches!(node.kind, ExprKind::Index { .. }))
            && let Some(found) = self.digital().carrier_window(scope, arg)
        {
            return found;
        }
        match &self.state.design.decl_data(port).kind {
            DeclKind::Port(p) if p.dir == Some(crate::pom::Dir::In) => self.digital().anonymous_definition(scope, instance, port, arg).map(|carrier| (carrier, None)),
            // The checker refuses an expression at an `out` port first (`E0314`).
            _ => {
                self.refuse(scope.values.file, node.span, StructureForm::ExpressionAtOutCarrierPort);
                None
            }
        }
    }

    /// The name of the `let` that binds the placement, else `<module>#<k>` (§21 rule 3).
    fn placement_name(&self, call: ExprId, module: &str, ordinal: u32) -> String {
        let owner = self.state.design.expr_node(call).owner;
        if let DeclKind::Let(l) = &self.state.design.decl_data(owner).kind
            && l.value == call
            && let crate::pom::PatKind::Ident { name, sub: None, .. } = &self.state.design.pat_node(l.pat).kind
        {
            return name.text.clone();
        }
        format!("{module}#{ordinal}")
    }

    /// The param values the placement `child` writes, each folded when it reads no param of the
    /// parent; one that does is lowered over the parent's params and kept beside the child as a
    /// `ChildValue::Expr`. `None` after a diagnostic.
    fn lower_given(&mut self, scope: &Scope, file: FileId, module: DeclId, child: u32, fields: &[FieldInit]) -> Option<Vec<(u32, Option<Constant>, ExprId)>> {
        let params: Vec<DeclId> = self.state.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        let mut ok = true;
        for field in fields {
            let index = match field.res {
                Resolution::Decl(d) => params.iter().position(|p| *p == d),
                _ => None,
            };
            let Some(index) = index else {
                self.refuse_with(file, field.span, StructureForm::PlacementThatSets, [field.name.text.to_string()]);
                ok = false;
                continue;
            };
            if !seen.insert(index) {
                self.report(file, Diagnostic::new("E0320", format!("the param `{}` is given twice", field.name.text), field.span, "param given twice"));
                ok = false;
                continue;
            }
            let bindings = if self.is_template_param(params[index]) { scope.values.keys.clone() } else { Bindings::new() }.overlaid(&scope.values.unrolled);
            match self.fold_param_value(field.value, &bindings) {
                Ok(value) => out.push((index as u32, Some(value), field.value)),
                // A value over the parent's params (`{ r: r_top }`, `{ l }`): the authored child
                // shows the expression, the instance table the constant of each instance.
                Err(ConstFault::UnboundParam { .. }) => {
                    let mut sub = Scope::sub_scope(scope);
                    match self.lower_param_expression(&mut sub, field.value) {
                        Some(value) if sub.values.locals.is_empty() => {
                            self.state.digital.structure.child_params.push((child, index as u32, ChildValue::Expr(value)));
                            out.push((index as u32, None, field.value));
                        }
                        Some(_) => {
                            self.elaboration.refuse(file, self.state.design.expr_node(field.value).span, StructureForm::LetInParamValue);
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
        let payload = if self.state.types.implicit_some(expr) {
            Some(expr)
        } else if self.values().is_some_call(expr) {
            match &self.state.design.expr_node(expr).kind {
                ExprKind::Call { args, .. } => args.first().map(|arg| arg.value),
                _ => None,
            }
        } else {
            None
        };
        // an `Option` param of the parent passed on (`{ rs: rs }`): its two halves, packed again
        if payload.is_none() && matches!(self.typed(expr).as_ref(), Some(crate::types::Ty::Option(_))) {
            let pair = self.values().lower_option(scope, expr)?;
            let origin = self.expr_origin(expr);
            let held = self.type_of(pair.val);
            return Some(self.emit(piperine_ir::Expr::OptionPack { is: pair.is, value: pair.val }, ValueType::Option(Box::new(held)), origin));
        }
        let Some(payload) = payload else { return self.values().lower_expr(scope, expr) };
        let value = self.values().lower_expr(scope, payload)?;
        let origin = self.expr_origin(expr);
        let is = self.emit(piperine_ir::Expr::Bool(true), ValueType::Bool, origin.clone());
        let held = self.type_of(value);
        Some(self.emit(piperine_ir::Expr::OptionPack { is, value }, ValueType::Option(Box::new(held)), origin))
    }
}
