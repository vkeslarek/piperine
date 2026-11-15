//! One IR body per module the top reaches: its params (type, folded default, `where` lowered
//! over the body's own params), its nodes (ports first, then signals, with nature and initial
//! potential). A body is a function of its module alone: one variant per module in M1.

use piperine_ir::{BodyNode, BodySpec, Child, Constant, NatureId, NodeRole, Origin, Param, Real, ValueType};

use super::children::ChildPlan;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, FileId, Resolution, TypeId};

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

/// What placement reads of a lowered module: its params and nodes in body order, and the
/// placements it writes.
#[derive(Clone)]
pub(super) struct ModulePlan {
    pub(super) body: piperine_ir::BodyId,
    pub(super) params: Vec<ParamPlan>,
    pub(super) nodes: Vec<NodePlan>,
    pub(super) children: Vec<ChildPlan>,
}

impl Elaboration<'_, '_> {
    /// Lower the module `module` to a body and remember its plan.
    pub(super) fn lower_body(&mut self, module: DeclId) {
        let data = self.design.decl_data(module).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut scope = Scope::new(data.file);
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        for (index, param) in params.iter().enumerate() {
            scope.params.insert(*param, index as u32);
        }
        let mut param_specs = Vec::new();
        let mut param_plans = Vec::new();
        for param in &params {
            if let Some((spec, plan)) = self.lower_param(&mut scope, *param) {
                param_specs.push(spec);
                param_plans.push(plan);
            }
        }
        let nodes = self.lower_nodes(&mut scope, module);
        let (node_specs, node_plans): (Vec<BodyNode>, Vec<NodePlan>) = nodes.into_iter().unzip();
        let (children, child_plans): (Vec<Child>, Vec<ChildPlan>) = self.lower_children(&scope, module).into_iter().unzip();
        let expected = self.body_ids.get(&module).copied();
        let body = self.ir.body(BodySpec {
            name,
            origin: self.locator.decl(module),
            params: param_specs,
            nodes: node_specs,
            branches: Vec::new(),
            limits: scope.limits,
            stims: scope.stims,
            locals: scope.locals,
            analog: None,
            children,
        });
        debug_assert_eq!(Some(body), expected, "bodies are built in first-reach order");
        self.plans.insert(module, ModulePlan { body, params: param_plans, nodes: node_plans, children: child_plans });
    }

    /// The IR type of a param's declared type.
    fn param_type(&mut self, file: FileId, ty: TypeId) -> Option<ValueType> {
        let node = self.design.type_node(ty).clone();
        match self.design.type_resolution(ty) {
            Resolution::Builtin(id) if id.info().name == "fN" && id.width() == Some(64) => Some(ValueType::F64),
            Resolution::Builtin(id) if id.info().name == "bool" => Some(ValueType::Bool),
            _ => {
                self.unsupported(file, node.span, "a param type other than `f64` and `bool`");
                None
            }
        }
    }

    fn lower_param(&mut self, scope: &mut Scope, param: DeclId) -> Option<(Param, ParamPlan)> {
        let data = self.design.decl_data(param).clone();
        let DeclKind::Param(p) = &data.kind else { return None };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let ty = self.param_type(data.file, p.ty)?;
        let default = match p.default {
            Some(expr) => match self.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(value),
                Err(fault) => {
                    self.const_fault(&fault, "a param default");
                    return None;
                }
            },
            None => None,
        };
        let check = match p.where_ {
            Some(cond) => Some(self.lower_expr(scope, cond)?),
            None => None,
        };
        let spec = Param { name: name.clone(), ty, default, check, structural: false, origin: self.locator.decl(param) };
        Some((spec, ParamPlan { decl: param, name, default, where_: p.where_ }))
    }

    /// The nodes of a module: ports first, then signals, each with its nature and initial value.
    fn lower_nodes(&mut self, scope: &mut Scope, module: DeclId) -> Vec<(BodyNode, NodePlan)> {
        let children = self.design.decl_data(module).children.clone();
        let ports = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_)));
        let signals = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Signal(_)));
        let mut out = Vec::new();
        for (decl, role) in ports.map(|d| (d, NodeRole::Port)).chain(signals.map(|d| (d, NodeRole::Signal))).collect::<Vec<_>>() {
            if let Some(node) = self.lower_node(decl, role) {
                scope.nodes.insert(decl, out.len() as u32);
                out.push(node);
            }
        }
        out
    }

    fn lower_node(&mut self, decl: DeclId, role: NodeRole) -> Option<(BodyNode, NodePlan)> {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (ty, init) = match &data.kind {
            DeclKind::Port(p) => (p.ty, None),
            DeclKind::Signal(s) => (s.ty, s.init),
            _ => return None,
        };
        let nature = self.design.nature_of(ty).and_then(|n| self.nature(n))?;
        let init = match init {
            Some(expr) => match self.consts.value(expr, &Bindings::new()) {
                Ok(Constant::Real(value)) => Some(value),
                Ok(Constant::Bool(_)) => {
                    self.unsupported(data.file, self.design.expr_node(expr).span, "a `bool` initial value");
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
