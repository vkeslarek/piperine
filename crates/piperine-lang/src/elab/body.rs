//! One IR body per module the top reaches: its params (type, folded default, `where` lowered
//! over the body's own params), its nodes (ports first, then signals, with nature and initial
//! potential). A body is a function of its module alone: one variant per module in M1.

use piperine_ir::{BodyId, BodyNode, BodySpec, Carrier, Child, Constant, NatureId, NodeRole, Origin, Merge, Param, Real, Terminal, ValueType};

use super::carriers::{Frame, Structure};
use super::children::ChildPlan;
use super::consts::{Bindings, ConstFault};
use super::elaborator::Elaboration;
use super::instances::Instance;
use super::structural::BodyKey;
use super::lower::Scope;
use crate::refusal::forms::StructureForm;
use crate::pom::{DeclId, DeclKind, ExprId};
use crate::types::Ty;

/// A param of a module, as placement needs it.
#[derive(Clone)]
pub(super) struct ParamPlan {
    pub(super) decl: DeclId,
    pub(super) name: String,
    pub(super) default: Option<Constant>,
    /// A default that reads earlier params (`g: f64 = 1.0 / r`, §8.1.16): folded at each placement over the values the
    /// params before it take there, so `default` is `None`. Every param it reads is structural (`structural.rs`).
    pub(super) default_over_params: Option<ExprId>,
    /// The `where` condition as written, evaluated at each placement over the bound values.
    pub(super) where_: Option<ExprId>,
}

/// A node of a module: a port or a signal.
#[derive(Clone)]
pub(super) struct NodePlan {
    pub(super) name: String,
    pub(super) role: NodeRole,
    pub(super) nature: NatureId,
    pub(super) init: Option<Real>,
    pub(super) origin: Origin,
}

/// What placement reads of a lowered module: its nodes and carriers in body order, and
/// the placements it writes.
#[derive(Clone)]
pub(super) struct ModulePlan {
    pub(super) body: piperine_ir::BodyId,
    pub(super) nodes: Vec<NodePlan>,
    /// The branches the body merges: in every instance their two ends are one node.
    pub(super) merges: Vec<(Terminal, Terminal)>,
    /// Every carrier of the body, in its order: each is bound to one net per instance.
    pub(super) carriers: Vec<Carrier>,
    /// How many of the leading carriers are ports of the module itself.
    pub(super) port_carriers: usize,
    /// What the body says of each of its resolved carriers (`resolvers.rs`).
    pub(super) resolved: Vec<super::resolvers::ResolvedPlan>,
    pub(super) children: Vec<ChildPlan>,
    /// The nodes and carriers of children the body reads: placement binds each once the child is placed.
    pub(super) reaches: Vec<piperine_ir::Reach>,
}

impl ModulePlan {
    /// Whether carrier `carrier` of the body stands for a carrier of a child (`inst.name`): a resolved one is a drive of the
    /// child's net, with no net of its own.
    pub(super) fn is_reached(&self, carrier: u32) -> bool {
        self.reaches.iter().any(|reach| matches!(reach.item, piperine_ir::ReachedItem::Carrier { carrier: reached, .. } if reached == carrier))
    }
}

impl Elaboration<'_, '_> {
    /// What the body being lowered says of each resolved carrier: its nature, the `impl` its drives picked, whether it
    /// holds a drive of the net and whether it reads `others()`.
    fn resolved_plans(&self) -> Vec<super::resolvers::ResolvedPlan> {
        self.state.digital.structure
            .resolved_carriers
            .iter()
            .map(|(carrier, decl, nature)| super::resolvers::ResolvedPlan {
                carrier: *carrier,
                decl: *decl,
                nature: *nature,
                implementation: self.concrete_implementation(*decl, *nature),
                drives: self.state.digital.structure.committed.contains(carrier) || self.state.digital.structure.drives.iter().any(|drive| drive.carrier == *carrier) || self.state.digital.structure.carriers[*carrier as usize].init.is_some(),
                reads_others: self.state.digital.structure.others_read.contains(carrier),
            })
            .collect()
    }

    /// The plan of every param of the modules in `modules`: its name, its folded default and its
    /// `where`, which placement reads. A default that does not fold is reported once, here.
    pub(super) fn plan_params(&mut self, modules: &[DeclId]) {
        for module in modules {
            let plans = self.plan_module_params(*module);
            self.state.structure.param_plans.insert(*module, plans);
        }
    }

    /// The plan of every param of `module`, its defaults folded under the substitution set now (an instance of a generic
    /// module sets its own, `generic_bodies.rs`).
    pub(super) fn plan_module_params(&mut self, module: DeclId) -> Vec<ParamPlan> {
        let mut plans = Vec::new();
        for param in self.state.design.decl_data(module).children.clone() {
            let data = self.state.design.decl_data(param).clone();
            let DeclKind::Param(p) = &data.kind else { continue };
            let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let (default, default_over_params) = match p.default {
                Some(expr) => match self.fold_param_value(expr, &Bindings::new()) {
                    Ok(value) => (Some(value), None),
                    Err(ConstFault::UnboundParam { .. }) if self.reads_earlier_params_only(module, param, expr) => (None, Some(expr)),
                    Err(ConstFault::UnboundParam { .. }) => continue,
                    Err(fault) => {
                        self.const_fault(&fault, "a param default");
                        continue;
                    }
                },
                None => (self.absent_option(p), None),
            };
            plans.push(ParamPlan { decl: param, name, default, default_over_params, where_: p.where_ });
        }
        plans
    }

    /// Whether the default `expr` of the param `param` of `module` reads only params declared before `param` (§8.1.16); a
    /// read of `param` itself or of a later param is `E0321`, reported here.
    fn reads_earlier_params_only(&mut self, module: DeclId, param: DeclId, expr: ExprId) -> bool {
        let order = self.params_of(module);
        let own = order.iter().position(|p| *p == param);
        let mut read = Vec::new();
        self.collect_params(expr, &mut read);
        let Some(later) = read.into_iter().find(|p| order.iter().position(|q| q == p) >= own) else { return true };
        let name = self.state.design.decl_data(later).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let file = self.state.design.decl_data(param).file;
        let span = self.state.design.expr_node(expr).span;
        let message = format!("a param default is not an expression over params and constants declared before it: it reads `{name}`, which is not declared before this param (§8.1.16)");
        self.report(file, crate::msl::Diagnostic::new("E0321", message, span, "reads a param not declared before it"));
        false
    }

    /// The default `plan` gives a param at a placement whose earlier params hold `bound`: the folded constant, or the default
    /// that reads earlier params folded over them (§8.1.16). `Ok(None)` for a param with no default.
    pub(super) fn default_at(&mut self, plan: &ParamPlan, bound: &Bindings) -> Result<Option<Constant>, ConstFault> {
        match plan.default_over_params {
            Some(expr) => self.fold_param_value(expr, bound).map(Some),
            None => Ok(plan.default),
        }
    }

    /// The param plans of `module` as a placement of it sees them: those of its instance when it is generic.
    pub(super) fn plans_of(&self, module: DeclId, instance: Option<&Instance>) -> Option<&Vec<ParamPlan>> {
        match instance {
            Some(instance) => self.state.structure.instance_param_plans.get(instance),
            None => self.state.structure.param_plans.get(&module),
        }
    }

    /// The default of a param `declared` with none written: `None` for an `Option` param (§3.2.10, PI-03 AC 1, explicit-unknowns
    /// EU-06 AC 1), so an instance that binds nothing reads the absent case. A required param (§8.1.17, `ParamRequirement`, the
    /// rule the checker's templates follow) has no default to give (explicit-unknowns D4: no numeric, `bool` or enum `Default`).
    pub(super) fn absent_option(&mut self, declared: &crate::pom::ParamData) -> Option<Constant> {
        let written = self.typed_written(declared.ty)?;
        if crate::types::ParamRequirement::new(declared, Some(&written)).is_required() {
            return None;
        }
        let stored = self.stored_type(&written)?;
        self.default_constant(&written, &stored)
    }

    /// Lower the body `key` of the module `module`, whose key params hold `keys`, and remember
    /// its plan. `expected` is the id the body was promised (`body_ids`, `BodyVariants`).
    pub(super) fn lower_body(&mut self, module: DeclId, keys: &Bindings, key: BodyKey, expected: BodyId) {
        // an instance of a generic module is lowered under its own arguments (`generic_bodies.rs`)
        let outer = key.instance.as_ref().map(|instance| self.state.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments)));
        let enclosing = std::mem::replace(&mut self.state.structure.lowering, Some(key.clone()));
        self.lower_body_under_substitution(module, keys, key, expected);
        self.state.structure.lowering = enclosing;
        if let Some(outer) = outer {
            self.state.consts.set_substitution(outer);
        }
    }

    /// [`Elaboration::lower_body`] with the substitution of the body's instance set.
    fn lower_body_under_substitution(&mut self, module: DeclId, keys: &Bindings, key: BodyKey, expected: BodyId) {
        let data = self.state.design.decl_data(module).clone();
        let name = self.body_name(&key);
        let mut scope = Scope::new(data.file);
        scope.values.keys = keys.clone();
        self.state.digital.structure = Structure::default();
        self.state.structure.frames = vec![Frame::root(module)];
        self.state.structure.frames[0].substitution = self.state.consts.substitution().clone();
        self.state.structure.body_keys = keys.clone();
        self.state.consts.set_frame_params(keys.clone());
        self.expand_generation(module);
        self.index_children(module);
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        for (index, param) in params.iter().enumerate() {
            scope.values.params.insert(*param, index as u32);
        }
        let mut param_specs = Vec::new();
        for param in &params {
            if let Some(spec) = self.lower_param(&mut scope, module, *param) {
                param_specs.push(spec);
            }
        }
        self.lower_nodes(module);
        self.lower_nested_demands(&scope, module);
        let ammeters = self.lower_ammeters(&mut scope, module);
        scope.analog.merged = self.merged_branches(&scope, module);
        self.lower_carriers(0, module);
        self.lower_nested_carrier_demands(&scope, module);
        self.lower_definitions(&scope);
        self.hoist_placements(&scope);
        scope.analog.addend_lets = self.addend_only_ddt_lets(&scope, module);
        let mut analog = self.lower_analog(&mut scope, module);
        if !ammeters.is_empty() {
            analog.get_or_insert_with(Vec::new).splice(0..0, ammeters);
        }
        if let Some(hoisted) = self.lower_hoisted_analog(&mut scope) {
            analog.get_or_insert_with(Vec::new).extend(hoisted);
        }
        self.decide_branch_kinds(&mut scope);
        self.check_selector_rule(&scope);
        let (children, child_plans): (Vec<Child>, Vec<ChildPlan>) = self.lower_children(&mut scope, module).into_iter().unzip();
        self.lower_digital(&mut scope, module);
        self.judge_element_coverage(module, &name);
        self.judge_drive_overlaps(&name);
        self.judge_drive_loops(&name);
        let carriers = self.state.digital.structure.carriers.clone();
        let port_carriers = self.state.digital.structure.port_carriers;
        let resolved = self.resolved_plans();
        let reaches = self.state.digital.structure.reaches.clone();
        let merged = scope.analog.merged.clone();
        let (node_specs, node_plans): (Vec<BodyNode>, Vec<NodePlan>) = std::mem::take(&mut self.state.digital.structure.nodes).into_iter().unzip();
        let body = self.state.builder.body(BodySpec {
            name,
            origin: self.state.locator.decl(module),
            params: param_specs,
            nodes: node_specs,
            branches: scope.analog.branches,
            limits: scope.analog.limits,
            stims: scope.analog.stims,
            locals: scope.values.locals,
            analog,
            children,
        });
        debug_assert_eq!(body, expected, "bodies are built in the order of their ids");
        if let Some(instance) = &key.instance {
            self.state.builder.mark_body_instance(body, piperine_ir::GenericInstance { item: instance.item_name(self), arguments: instance.argument_names(self) });
        }
        self.finish_structure(body);
        for (plus, minus) in &merged {
            self.state.builder.merge(body, Merge { plus: *plus, minus: *minus });
        }
        self.state.structure.plans.insert(key, ModulePlan { body, nodes: node_plans, merges: merged, carriers, port_carriers, resolved, children: child_plans, reaches });
    }

    /// The param `param` of `module` as the body has it: its type, its `where` lowered over the
    /// body's own params, and whether it fixes the body's structure.
    fn lower_param(&mut self, scope: &mut Scope, module: DeclId, param: DeclId) -> Option<Param> {
        let data = self.state.design.decl_data(param).clone();
        let DeclKind::Param(p) = &data.kind else { return None };
        let plan = self.plans_of(module, self.state.structure.lowering.as_ref().and_then(|key| key.instance.as_ref()))?.iter().find(|plan| plan.decl == param)?.clone();
        // A param of module type holds its template's number (`templates.rs`).
        let ty = if self.is_module_param(param) {
            ValueType::UInt(32)
        } else if let Some(length) = self.template_array_length(param) {
            ValueType::Array(Box::new(ValueType::UInt(32)), length as u64)
        } else { self.lower_param_type(data.file, p.ty)? };
        let check = match p.where_ {
            Some(cond) => Some(self.lower_expr(scope, cond)?),
            None => None,
        };
        let structural = self.state.structure.keys.is_key(module, param);
        // a default that reads earlier params reads key params only, which the body fixes (§8.1.16)
        let default = match self.default_at(&plan, &self.state.structure.body_keys.clone()) {
            Ok(default) => default,
            Err(fault) => {
                self.const_fault(&fault, "a param default");
                return None;
            }
        };
        Some(Param { name: plan.name, ty, default, check, structural, origin: self.state.locator.decl(param) })
    }

    /// The type a param of the declared type `ty` holds: one value, an `Option` kept whole as `ValueType::Option`
    /// (its pin is `{ some, v }`, explicit-unknowns P5), unpacked into its halves where a read needs them.
    fn lower_param_type(&mut self, file: crate::pom::FileId, ty: crate::pom::TypeId) -> Option<ValueType> {
        let stored = self.typed_written(ty).and_then(|t| self.stored_type(&t));
        if stored.is_none() {
            let span = self.state.design.type_node(ty).span;
            self.refuse(file, span, StructureForm::ParamOfTypeThatHasNoValue);
        }
        stored
    }

    /// The values the params a frame's initial values may read take: the body's key params for the body's
    /// own module (frame 0, `body_keys`), the constants the placement bound for a hoisted module.
    pub(super) fn frame_bindings(&self, frame: usize) -> Bindings {
        if frame == 0 {
            return self.state.structure.body_keys.clone();
        }
        let mut bindings = Bindings::new();
        for (param, value) in &self.state.structure.frames[frame].params {
            self.bind_param(&mut bindings, *param, *value);
        }
        bindings
    }

    /// What the generation of the frame `from` unrolled into is parked in it, and that of the frame `to` becomes the active one
    /// (`Elaboration::generated`): each hoisted frame unrolls the generation of its own module (`hoist.rs`).
    pub(super) fn switch_generation(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        self.state.structure.frames[from].generated = std::mem::take(&mut self.state.structure.generated);
        self.state.structure.generated = std::mem::take(&mut self.state.structure.frames[to].generated);
    }

    /// Lowers code of `frame` from here on: `scope` reads its names and the types read its params. The state of the
    /// frame left comes back with `leave_frame`.
    pub(super) fn enter_frame(&mut self, scope: &mut Scope, frame: usize) -> FrameEntry {
        let params = self.frame_bindings(frame);
        let substitution = self.state.consts.set_substitution(self.state.structure.frames[frame].substitution.clone());
        let outer = std::mem::replace(&mut scope.values.frame, frame);
        self.switch_generation(outer, frame);
        FrameEntry { outer, params: self.state.consts.set_frame_params(params), substitution }
    }

    /// Back to the frame `entry` left.
    pub(super) fn leave_frame(&mut self, scope: &mut Scope, entry: FrameEntry) {
        self.switch_generation(scope.values.frame, entry.outer);
        scope.values.frame = entry.outer;
        self.state.consts.set_frame_params(entry.params);
        self.state.consts.set_substitution(entry.substitution);
    }

    /// The nodes of a module, ports first and then signals, each with its nature and initial value,
    /// added to the structure of the body and named in frame 0. A discrete carrier is not a node
    /// (`carriers.rs`).
    fn lower_nodes(&mut self, module: DeclId) {
        let mut children = self.state.design.decl_data(module).children.clone();
        // the nodes declared in generation are arrays, one element per iteration (`generation.rs`)
        children.extend(self.state.structure.generated.elaborated().filter(|item| matches!(self.state.design.decl_data(*item).kind, DeclKind::Signal(_))));
        // a bundle port or signal stands for its fields: each leaf is a node or a carrier, the bundle itself neither (`bundles.rs`)
        let children = self.state.design.module_items(&children);
        let node = |c: &DeclId| (self.state.types.carrier(*c).is_none() || self.state.types.node_array(*c).is_some()) && !self.state.design.is_bundle_owner(*c);
        let ports = children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Port(_)) && node(c));
        let signals = children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Signal(_)) && node(c));
        for (decl, role) in ports.map(|d| (d, NodeRole::Port)).chain(signals.map(|d| (d, NodeRole::Signal))).collect::<Vec<_>>() {
            if self.state.types.node_array(decl).is_some() {
                self.generated_items_complete(decl);
                if let Some((shape, first)) = self.add_node_array(decl, role, "", 0) {
                    self.state.structure.frames[0].nodes.insert(decl, first);
                    self.state.structure.frames[0].node_shapes.insert(decl, shape);
                }
            } else if let Some(terminal) = self.add_node(decl, role, "", 0) {
                self.state.structure.frames[0].nodes.insert(decl, terminal);
            }
        }
    }

    /// The node `decl` as a node of the body being lowered, named `prefix` and its own name (`lag.y`
    /// for a signal of a hoisted frame): added to the structure; its terminal. `None` after a diagnostic.
    pub(super) fn add_node(&mut self, decl: DeclId, role: NodeRole, prefix: &str, frame: usize) -> Option<Terminal> {
        let (spec, plan) = self.lower_node(decl, role, prefix, frame)?;
        self.state.digital.structure.nodes.push((spec, plan));
        Some(Terminal::Node(self.state.digital.structure.nodes.len() as u32 - 1))
    }

    fn lower_node(&mut self, decl: DeclId, role: NodeRole, prefix: &str, frame: usize) -> Option<(BodyNode, NodePlan)> {
        let data = self.state.design.decl_data(decl).clone();
        let name = format!("{prefix}{}", data.name.as_ref().map_or("", |n| n.text.as_str()));
        self.lower_node_named(decl, role, &name, frame)
    }

    /// The node `decl` named `name` in the body: one node, or one element of an array of nodes (`carrier_arrays.rs`).
    pub(super) fn lower_node_named(&mut self, decl: DeclId, role: NodeRole, name: &str, frame: usize) -> Option<(BodyNode, NodePlan)> {
        let data = self.state.design.decl_data(decl).clone();
        let name = name.to_string();
        let (ty, init) = match &data.kind {
            DeclKind::Port(p) => (p.ty, None),
            DeclKind::Signal(s) => (s.ty, s.init),
            _ => return None,
        };
        let nature = match self.state.consts.typed_node_array(decl).map(|t| t.innermost().clone()) {
            Some(Ty::Node(nature)) => self.nature(nature)?,
            _ => self.nature_decl_of(ty).and_then(|n| self.nature(n))?,
        };
        let init = match (init, self.state.types.node_array(decl).is_some()) {
            (_, true) => None,
            (init, false) => init,
        };
        let init = match init {
            Some(expr) => match self.state.consts.value(expr, &self.frame_bindings(frame)) {
                Ok(Constant::Real(value)) => Some(value),
                Ok(Constant::Bool(_)) => {
                    self.refuse(data.file, self.state.design.expr_node(expr).span, StructureForm::BoolInitialValue);
                    return None;
                }
                Ok(Constant::Int(_) | Constant::FloatBits(_) | Constant::Wide(_)) => {
                    self.refuse(data.file, self.state.design.expr_node(expr).span, StructureForm::IntegerOrNumberWidthInitialValue);
                    return None;
                }
                Ok(Constant::Aggregate(_)) => {
                    self.refuse(data.file, self.state.design.expr_node(expr).span, StructureForm::CompositeInitialValue);
                    return None;
                }
                Err(fault) => {
                    self.const_fault(&fault, "a node's initial value");
                    return None;
                }
            },
            None => None,
        };
        let origin = self.state.locator.decl(decl);
        let node = BodyNode { name: name.clone(), nature, role, init, origin: origin.clone() };
        Some((node, NodePlan { name, role, nature, init, origin }))
    }
}

/// The frame a lowering left when it entered another (`Elaboration::enter_frame`).
pub(super) struct FrameEntry {
    /// The frame `scope` read before.
    outer: usize,
    /// The module params the types read before.
    params: Bindings,
    /// The substitution the types read before.
    substitution: crate::types::Substitution,
}
