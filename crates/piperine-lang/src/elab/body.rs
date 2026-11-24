//! One IR body per module the top reaches: its params (type, folded default, `where` lowered
//! over the body's own params), its nodes (ports first, then signals, with nature and initial
//! potential). A body is a function of its module alone: one variant per module in M1.

use piperine_ir::{BodyId, BodyNode, BodySpec, Carrier, Child, Constant, NatureId, NodeRole, Origin, Merge, Param, Real, Terminal, ValueType};

use super::carriers::{Frame, Structure};
use super::children::ChildPlan;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::structural::BodyKey;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId};
use crate::types::Ty;

/// A param of a module, as placement needs it.
#[derive(Clone)]
pub(super) struct ParamPlan {
    pub(super) decl: DeclId,
    pub(super) name: String,
    pub(super) default: Option<Constant>,
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
    pub(super) children: Vec<ChildPlan>,
    /// The nodes and carriers of children the body reads: placement binds each once the child is placed.
    pub(super) reaches: Vec<piperine_ir::Reach>,
}

impl Elaboration<'_, '_> {
    /// The plan of every param of the modules in `modules`: its name, its folded default and its
    /// `where`, which placement reads. A default that does not fold is reported once, here.
    pub(super) fn plan_params(&mut self, modules: &[DeclId]) {
        for module in modules {
            let mut plans = Vec::new();
            for param in self.design.decl_data(*module).children.clone() {
                let data = self.design.decl_data(param).clone();
                let DeclKind::Param(p) = &data.kind else { continue };
                let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
                let default = match p.default {
                    Some(expr) => match self.fold_param_value(expr, &Bindings::new()) {
                        Ok(value) => Some(value),
                        Err(fault) => {
                            self.const_fault(&fault, "a param default");
                            continue;
                        }
                    },
                    None => self.absent_option(p.ty),
                };
                plans.push(ParamPlan { decl: param, name, default, where_: p.where_ });
            }
            self.param_plans.insert(*module, plans);
        }
    }

    /// The default of an `Option` param written with none: `None` (§3.2.10, PI-03 AC 1, explicit-unknowns
    /// EU-06 AC 1), so an instance that binds nothing reads the absent case. Any other type has no
    /// default to give (explicit-unknowns D4: no numeric, `bool` or enum `Default`), so the param is required.
    fn absent_option(&mut self, ty: crate::pom::TypeId) -> Option<Constant> {
        let written = self.typed_written(ty)?;
        if !matches!(written, crate::types::Ty::Option(_)) {
            return None;
        }
        let stored = self.stored_type(&written)?;
        self.default_constant(&written, &stored)
    }

    /// Lower the body `key` of the module `module`, whose key params hold `keys`, and remember
    /// its plan. `expected` is the id the body was promised (`body_ids`, `BodyVariants`).
    pub(super) fn lower_body(&mut self, module: DeclId, keys: &Bindings, key: BodyKey, expected: BodyId) {
        let data = self.design.decl_data(module).clone();
        let name = self.body_name(&key);
        let mut scope = Scope::new(data.file);
        scope.keys = keys.clone();
        self.structure = Structure::default();
        self.frames = vec![Frame::root(module)];
        self.body_keys = keys.clone();
        self.consts.set_frame_params(keys.clone());
        self.index_children(module);
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        for (index, param) in params.iter().enumerate() {
            scope.params.insert(*param, index as u32);
        }
        let mut param_specs = Vec::new();
        for param in &params {
            if let Some(spec) = self.lower_param(&mut scope, module, *param) {
                param_specs.push(spec);
            }
        }
        self.lower_nodes(module);
        let ammeters = self.lower_ammeters(&mut scope, module);
        scope.merged = self.merged_branches(&scope, module);
        self.lower_carriers(0, module);
        self.lower_definitions(&scope, module);
        self.hoist_placements(&scope);
        scope.addend_lets = self.addend_only_ddt_lets(&scope, module);
        let mut analog = self.lower_analog(&mut scope, module);
        if !ammeters.is_empty() {
            analog.get_or_insert_with(Vec::new).splice(0..0, ammeters);
        }
        if let Some(hoisted) = self.lower_hoisted_analog(&mut scope) {
            analog.get_or_insert_with(Vec::new).extend(hoisted);
        }
        self.decide_branch_kinds(&mut scope);
        self.check_selector_rule(&scope);
        let (children, child_plans): (Vec<Child>, Vec<ChildPlan>) = self.lower_children(&scope, module).into_iter().unzip();
        self.lower_digital(&scope, module);
        let carriers = self.structure.carriers.clone();
        let reaches = self.structure.reaches.clone();
        let merged = scope.merged.clone();
        let (node_specs, node_plans): (Vec<BodyNode>, Vec<NodePlan>) = std::mem::take(&mut self.structure.nodes).into_iter().unzip();
        let body = self.ir.body(BodySpec {
            name,
            origin: self.locator.decl(module),
            params: param_specs,
            nodes: node_specs,
            branches: scope.branches,
            limits: scope.limits,
            stims: scope.stims,
            locals: scope.locals,
            analog,
            children,
        });
        debug_assert_eq!(body, expected, "bodies are built in the order of their ids");
        self.finish_structure(body);
        for (plus, minus) in &merged {
            self.ir.merge(body, Merge { plus: *plus, minus: *minus });
        }
        self.plans.insert(key, ModulePlan { body, nodes: node_plans, merges: merged, carriers, children: child_plans, reaches });
    }

    /// The param `param` of `module` as the body has it: its type, its `where` lowered over the
    /// body's own params, and whether it fixes the body's structure.
    fn lower_param(&mut self, scope: &mut Scope, module: DeclId, param: DeclId) -> Option<Param> {
        let data = self.design.decl_data(param).clone();
        let DeclKind::Param(p) = &data.kind else { return None };
        let plan = self.param_plans.get(&module)?.iter().find(|plan| plan.decl == param)?.clone();
        // A param of module type holds its template's number (`templates.rs`).
        let ty = if self.is_module_param(param) { ValueType::UInt(32) } else { self.lower_param_type(data.file, p.ty)? };
        let check = match p.where_ {
            Some(cond) => Some(self.lower_expr(scope, cond)?),
            None => None,
        };
        let structural = self.keys.is_key(module, param);
        Some(Param { name: plan.name, ty, default: plan.default, check, structural, origin: self.locator.decl(param) })
    }

    /// The type a param of the declared type `ty` holds: one value, an `Option` kept whole as `ValueType::Option`
    /// (its pin is `{ some, v }`, explicit-unknowns P5), unpacked into its halves where a read needs them.
    fn lower_param_type(&mut self, file: crate::pom::FileId, ty: crate::pom::TypeId) -> Option<ValueType> {
        let stored = self.typed_written(ty).and_then(|t| self.stored_type(&t));
        if stored.is_none() {
            let span = self.design.type_node(ty).span;
            self.unsupported(file, span, "a param of a type that has no value in the IR");
        }
        stored
    }

    /// The values the params a frame's initial values may read take: the body's key params for the body's
    /// own module (frame 0, `body_keys`), the constants the placement bound for a hoisted module.
    pub(super) fn frame_bindings(&self, frame: usize) -> Bindings {
        if frame == 0 {
            return self.body_keys.clone();
        }
        let mut bindings = Bindings::new();
        for (param, value) in &self.frames[frame].params {
            self.bind_param(&mut bindings, *param, *value);
        }
        bindings
    }

    /// Lowers code of `frame` from here on: `scope` reads its names and the types read its params. The state of the
    /// frame left comes back with `leave_frame`.
    pub(super) fn enter_frame(&mut self, scope: &mut Scope, frame: usize) -> FrameEntry {
        let params = self.frame_bindings(frame);
        FrameEntry { outer: std::mem::replace(&mut scope.frame, frame), params: self.consts.set_frame_params(params) }
    }

    /// Back to the frame `entry` left.
    pub(super) fn leave_frame(&mut self, scope: &mut Scope, entry: FrameEntry) {
        scope.frame = entry.outer;
        self.consts.set_frame_params(entry.params);
    }

    /// The nodes of a module, ports first and then signals, each with its nature and initial value,
    /// added to the structure of the body and named in frame 0. A discrete carrier is not a node
    /// (`carriers.rs`).
    fn lower_nodes(&mut self, module: DeclId) {
        let children = self.design.decl_data(module).children.clone();
        let node = |c: &DeclId| self.types.carrier(*c).is_none() || self.types.node_array(*c).is_some();
        let ports = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_)) && node(c));
        let signals = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Signal(_)) && node(c));
        for (decl, role) in ports.map(|d| (d, NodeRole::Port)).chain(signals.map(|d| (d, NodeRole::Signal))).collect::<Vec<_>>() {
            if self.types.node_array(decl).is_some() {
                if let Some((shape, first)) = self.add_node_array(decl, role, "", 0) {
                    self.frames[0].nodes.insert(decl, first);
                    self.frames[0].node_shapes.insert(decl, shape);
                }
            } else if let Some(terminal) = self.add_node(decl, role, "", 0) {
                self.frames[0].nodes.insert(decl, terminal);
            }
        }
    }

    /// The node `decl` as a node of the body being lowered, named `prefix` and its own name (`lag.y`
    /// for a signal of a hoisted frame): added to the structure; its terminal. `None` after a diagnostic.
    pub(super) fn add_node(&mut self, decl: DeclId, role: NodeRole, prefix: &str, frame: usize) -> Option<Terminal> {
        let (spec, plan) = self.lower_node(decl, role, prefix, frame)?;
        self.structure.nodes.push((spec, plan));
        Some(Terminal::Node(self.structure.nodes.len() as u32 - 1))
    }

    fn lower_node(&mut self, decl: DeclId, role: NodeRole, prefix: &str, frame: usize) -> Option<(BodyNode, NodePlan)> {
        let data = self.design.decl_data(decl).clone();
        let name = format!("{prefix}{}", data.name.as_ref().map_or("", |n| n.text.as_str()));
        self.lower_node_named(decl, role, &name, frame)
    }

    /// The node `decl` named `name` in the body: one node, or one element of an array of nodes (`carrier_arrays.rs`).
    pub(super) fn lower_node_named(&mut self, decl: DeclId, role: NodeRole, name: &str, frame: usize) -> Option<(BodyNode, NodePlan)> {
        let data = self.design.decl_data(decl).clone();
        let name = name.to_string();
        let (ty, init) = match &data.kind {
            DeclKind::Port(p) => (p.ty, None),
            DeclKind::Signal(s) => (s.ty, s.init),
            _ => return None,
        };
        let nature = match self.types.node_array(decl).map(|t| t.innermost().clone()) {
            Some(Ty::Node(nature)) => self.nature(nature)?,
            _ => self.design.nature_of(ty).and_then(|n| self.nature(n))?,
        };
        let init = match (init, self.types.node_array(decl).is_some()) {
            (_, true) => None,
            (init, false) => init,
        };
        let init = match init {
            Some(expr) => match self.consts.value(expr, &self.frame_bindings(frame)) {
                Ok(Constant::Real(value)) => Some(value),
                Ok(Constant::Bool(_)) => {
                    self.unsupported(data.file, self.design.expr_node(expr).span, "a `bool` initial value");
                    return None;
                }
                Ok(Constant::Int(_) | Constant::FloatBits(_) | Constant::Wide(_)) => {
                    self.unsupported(data.file, self.design.expr_node(expr).span, "an integer or number-width initial value");
                    return None;
                }
                Ok(Constant::Aggregate(_)) => {
                    self.unsupported(data.file, self.design.expr_node(expr).span, "a composite initial value");
                    return None;
                }
                Err(fault) => {
                    self.const_fault(&fault, "a node's initial value");
                    return None;
                }
            },
            None => None,
        };
        let origin = self.locator.decl(decl);
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
}
