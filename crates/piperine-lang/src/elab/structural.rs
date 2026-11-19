//! Params that fix structure are the keys of body variants (Part IV §11 rule 3, Part II §8.1.2).
//!
//! The kind of a branch with a potential contribution is constant during a solve, and when every
//! selector of it depends on params only it is fixed at elaboration: those params are
//! *structural*. A body therefore depends on the values of its key params, and one mechanism serves
//! it: **a body is lowered once per distinct value of its key params**.
//!
//! * `KeyParams` is the classification, over the reached modules with a body: a param read by a
//!   condition that depends on params only and encloses a contribution to a branch that has a
//!   potential contribution (*structural*), and, transitively, a param read by the value a
//!   placement gives a key param of its child (a fixpoint).
//! * A module with no key param has one body, its id the position of the module among the
//!   unkeyed modules in first-reach order, as before this feature: no existing body moves.
//! * A keyed module's variants take their ids on request after every unkeyed body
//!   (`BodyVariants`), are named `Name~p=v` (the `~` of function variants, `variants.rs`) and are
//!   lowered in id order with their key params bound to constants (`Scope::keys`): a condition
//!   that reads only key params and constants folds (`lower.rs`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use piperine_ir::{BodyId, Constant};

use super::consts::Bindings;
use super::body::ModulePlan;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, Design, ExprId, ExprKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::Types;

/// A body to lower: a module and the values of its key params, one text per key param in the
/// order of the module's params (the text of a `Constant` tells two values apart exactly).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BodyKey {
    pub(super) module: DeclId,
    pub(super) values: Vec<String>,
}

impl BodyKey {
    /// The key of the one body of a module with no key param.
    pub(super) fn unkeyed(module: DeclId) -> BodyKey {
        BodyKey { module, values: Vec::new() }
    }
}

/// The key params of each reached module with a body, in the order the module declares its params.
#[derive(Default)]
pub(super) struct KeyParams {
    by_module: BTreeMap<DeclId, Vec<DeclId>>,
}

impl KeyParams {
    /// The key params of `module`.
    pub(super) fn of(&self, module: DeclId) -> &[DeclId] {
        self.by_module.get(&module).map_or(&[], Vec::as_slice)
    }

    /// Whether `param` is a key param of `module`.
    pub(super) fn is_key(&self, module: DeclId, param: DeclId) -> bool {
        self.of(module).contains(&param)
    }

    /// Records `param` as a key of `module`; `true` when it was not one.
    fn add(&mut self, module: DeclId, param: DeclId, order: &[DeclId]) -> bool {
        let keys = self.by_module.entry(module).or_default();
        if keys.contains(&param) {
            return false;
        }
        keys.push(param);
        keys.sort_by_key(|k| order.iter().position(|p| p == k));
        true
    }
}

/// The variants of bodies requested so far: the id handed out to each, in request order, and the
/// ones not yet lowered. A variant is lowered after every unkeyed body, in id order.
#[derive(Default)]
pub(super) struct BodyVariants {
    ids: BTreeMap<BodyKey, BodyId>,
    pending: VecDeque<(BodyKey, Bindings)>,
    /// The id the next variant takes: one past the unkeyed bodies, then one past each request.
    next: u32,
}

impl BodyVariants {
    /// Variants take ids from `first` on.
    pub(super) fn starting_at(first: u32) -> BodyVariants {
        BodyVariants { next: first, ..BodyVariants::default() }
    }

    /// The next variant to lower, with the values its key params are bound to.
    pub(super) fn next_pending(&mut self) -> Option<(BodyKey, Bindings, BodyId)> {
        let (key, bindings) = self.pending.pop_front()?;
        let id = self.ids[&key];
        Some((key, bindings, id))
    }
}

/// The params an expression reads, when it reads nothing else but constants and operators on
/// them: what a condition that depends on params only is made of.
pub(super) struct ParamReads<'a, 'd> {
    design: &'d Design,
    types: &'a Types<'d>,
}

impl<'a, 'd> ParamReads<'a, 'd> {
    pub(super) fn new(design: &'d Design, types: &'a Types<'d>) -> Self {
        ParamReads { design, types }
    }

    /// The params `expr` reads; `None` when it reads a probe, a carrier, the environment, a local
    /// or calls anything but a method of the catalogue.
    pub(super) fn of(&self, expr: ExprId) -> Option<BTreeSet<DeclId>> {
        let mut params = BTreeSet::new();
        self.collect(expr, &mut params).then_some(params)
    }

    /// Adds the params `expr` reads to `params`; `false` when it reads anything else.
    fn collect(&self, expr: ExprId, params: &mut BTreeSet<DeclId>) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(_) => true,
            ExprKind::Paren(inner) | ExprKind::Cast { expr: inner, .. } | ExprKind::Unary { op: UnOp::Neg | UnOp::Not, expr: inner } => self.collect(*inner, params),
            ExprKind::Binary { lhs, rhs, .. } => self.collect(*lhs, params) && self.collect(*rhs, params),
            ExprKind::MethodCall { recv, args, .. } if self.types.method(expr).is_some() => self.collect(*recv, params) && args.iter().all(|a| self.collect(a.value, params)),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => match &self.design.decl_data(decl).kind {
                    DeclKind::Param(_) => {
                        params.insert(decl);
                        true
                    }
                    DeclKind::Const(_) | DeclKind::Variant(_) => true,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }
}

/// One contribution of a module's `analog` blocks as written: the two ends of its branch (`None`
/// is ground, the pair is sorted), its quantity, and the `if` and `match` conditions that enclose
/// it, outermost first.
struct WrittenContribution {
    ends: [Option<DeclId>; 2],
    quantity: crate::pom::QuantityRole,
    conditions: Vec<ExprId>,
}

/// The walk that lists the contributions of statements with the conditions around them.
struct ContributionWalk<'e, 'd, 't> {
    elaboration: &'e Elaboration<'d, 't>,
    conditions: Vec<ExprId>,
    found: Vec<WrittenContribution>,
}

impl ContributionWalk<'_, '_, '_> {
    /// The contributions among `stmts` and what they hold, in source order.
    fn statements(&mut self, stmts: &[StmtId]) {
        for stmt in stmts {
            match self.elaboration.design.stmt_node(*stmt).kind.clone() {
                StmtKind::Contribution { lhs, .. } => self.contribution(lhs),
                StmtKind::Expr { expr, .. } => self.expression(expr),
                StmtKind::Let { init: Some(init), .. } => self.expression(init),
                _ => {}
            }
        }
    }

    /// The contributions inside `expr`: in a block, an `if` or a `match` it is.
    fn expression(&mut self, expr: ExprId) {
        crate::stack::deep(|| match self.elaboration.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.expression(inner),
            ExprKind::Block(block) => self.statements(&block.stmts),
            ExprKind::If { cond, then, else_ } => {
                self.conditions.push(cond);
                self.statements(&then.stmts);
                if let Some(otherwise) = else_ {
                    self.expression(otherwise);
                }
                self.conditions.pop();
            }
            ExprKind::Match { scrutinee, arms } => {
                self.conditions.push(scrutinee);
                for arm in arms {
                    self.expression(arm.body);
                }
                self.conditions.pop();
            }
            _ => {}
        });
    }

    /// Records the contribution whose left side is `lhs`, when it names a probe of one or two nodes.
    fn contribution(&mut self, lhs: ExprId) {
        let design = self.elaboration.design;
        let ExprKind::Call { callee, args, .. } = &design.expr_node(lhs).kind else { return };
        let Some(quantity) = self.elaboration.quantity_role(*callee) else { return };
        let mut ends: Vec<Option<DeclId>> = Vec::new();
        for arg in args {
            let Some(end) = self.elaboration.terminal_decl(arg.value) else { return };
            ends.push(end);
        }
        let mut ends = match ends[..] {
            [a] => [a, None],
            [a, b] => [a, b],
            _ => return,
        };
        ends.sort();
        self.found.push(WrittenContribution { ends, quantity, conditions: self.conditions.clone() });
    }
}

impl Elaboration<'_, '_> {
    /// The node `arg` names as an end of a branch: `Some(None)` for ground, `Some(Some(decl))` for
    /// a port or a signal, `None` for anything else.
    pub(super) fn terminal_decl(&self, arg: ExprId) -> Option<Option<DeclId>> {
        let ExprKind::Path(path) = &self.design.expr_node(arg).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res)? {
            Resolution::Decl(decl) => matches!(self.design.decl_data(decl).kind, DeclKind::Port(_) | DeclKind::Signal(_)).then_some(Some(decl)),
            Resolution::Builtin(builtin) if builtin.info().kind == crate::pom::BuiltinKind::Node => Some(None),
            _ => None,
        }
    }

    /// The params of `module`, in declaration order.
    fn params_of(&self, module: DeclId) -> Vec<DeclId> {
        self.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Param(_))).collect()
    }

    /// The contributions `module` writes in its `analog` blocks to a branch that has a potential
    /// contribution among them, with the conditions that enclose each.
    fn potential_capable_contributions(&self, module: DeclId) -> Vec<WrittenContribution> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let mut walk = ContributionWalk { elaboration: self, conditions: Vec::new(), found: Vec::new() };
        for block in &data.behaviors {
            if let DeclKind::Behavior(behavior) = &self.design.decl_data(*block).kind
                && behavior.kind == crate::pom::BehaviorKind::Analog
            {
                walk.statements(&behavior.body);
            }
        }
        let found = walk.found;
        let potential: Vec<[Option<DeclId>; 2]> = found.iter().filter(|c| c.quantity == crate::pom::QuantityRole::Potential).map(|c| c.ends).collect();
        found.into_iter().filter(|c| potential.contains(&c.ends)).collect()
    }

    /// Classifies the key params of `modules` (the reached modules with a body): see the module
    /// documentation. Run once, before any body is lowered.
    pub(super) fn classify_key_params(&mut self, modules: &[DeclId]) {
        for module in modules {
            let order = self.params_of(*module);
            for contribution in self.potential_capable_contributions(*module) {
                for condition in &contribution.conditions {
                    for param in ParamReads::new(self.design, self.types).of(*condition).into_iter().flatten() {
                        self.keys.add(*module, param, &order);
                    }
                }
            }
        }
        // A param read by the value a placement gives a key param of its child is a key too.
        let mut changed = true;
        while changed {
            changed = false;
            for module in modules {
                let order = self.params_of(*module);
                for (child, param) in self.params_that_feed_key_params(*module) {
                    let _ = child;
                    changed |= self.keys.add(*module, param, &order);
                }
            }
        }
    }

    /// The params of `module` read by a value `module` gives to a key param of a module it
    /// places, as (the placed module, the param of `module`).
    fn params_that_feed_key_params(&self, module: DeclId) -> Vec<(DeclId, DeclId)> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let mut out = Vec::new();
        for call in &data.instances {
            let Some(crate::types::Callee::Module(child)) = self.types.callee(*call) else { continue };
            if self.result_callee(*call).is_some() {
                continue;
            }
            for field in self.placement_fields(*call, child) {
                let Resolution::Decl(given) = field.res else { continue };
                if !self.keys.is_key(child, given) {
                    continue;
                }
                for param in ParamReads::new(self.design, self.types).of(field.value).into_iter().flatten() {
                    out.push((child, param));
                }
            }
        }
        out
    }
}

impl Elaboration<'_, '_> {
    /// The key of the body `module` gets when its params hold `values`, and the id of that body:
    /// a module with no key param has its one body; a variant takes the next id the first time its
    /// values are asked for and is lowered by `lower_pending_bodies`. `None` when a key param has
    /// no value in `values`.
    pub(super) fn request_body(&mut self, module: DeclId, values: &Bindings) -> Option<(BodyKey, BodyId)> {
        let keys = self.keys.of(module).to_vec();
        if keys.is_empty() {
            return Some((BodyKey::unkeyed(module), self.body_ids.get(&module).copied()?));
        }
        let mut keyed = Bindings::new();
        let mut texts = Vec::new();
        for key in &keys {
            let value = values.get(*key)?;
            keyed.bind(*key, value);
            texts.push(value.to_string());
        }
        let key = BodyKey { module, values: texts };
        if let Some(id) = self.body_variants.ids.get(&key) {
            return Some((key.clone(), *id));
        }
        let id = BodyId(self.body_variants.next);
        self.body_variants.next += 1;
        self.body_variants.ids.insert(key.clone(), id);
        self.body_variants.pending.push_back((key.clone(), keyed));
        Some((key, id))
    }

    /// Lowers every variant requested and not yet lowered, in id order; a variant may request
    /// more, which follow.
    pub(super) fn lower_pending_bodies(&mut self) {
        while let Some((key, values, id)) = self.body_variants.next_pending() {
            self.lower_body(key.module, &values, key, id);
        }
    }

    /// The plan of the body that `module` gets when its params hold `values`, lowering the
    /// variant if nothing asked for it before (the top, or a binding of the host).
    pub(super) fn body_plan(&mut self, module: DeclId, values: &Bindings) -> Option<ModulePlan> {
        let (key, _) = self.request_body(module, values)?;
        self.lower_pending_bodies();
        self.plans.get(&key).cloned()
    }

    /// The name of the body of `key`: the module's, then `~param=value` for each key param.
    pub(super) fn body_name(&self, key: &BodyKey) -> String {
        let name = self.design.decl_data(key.module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if key.values.is_empty() {
            return name;
        }
        let pairs: Vec<String> = self.keys.of(key.module).iter().zip(&key.values).map(|(param, value)| format!("{}={value}", self.design.decl_data(*param).name.as_ref().map_or("", |n| n.text.as_str()))).collect();
        format!("{name}~{}", pairs.join(","))
    }
}


impl Elaboration<'_, '_> {
    /// The value of the condition `cond` when it reads only the key params `scope.keys` binds and
    /// constants; `None` for any other condition, which is lowered as written.
    pub(super) fn folded_condition(&mut self, scope: &Scope, cond: ExprId) -> Option<bool> {
        if scope.keys.is_empty() {
            return None;
        }
        let reads = ParamReads::new(self.design, self.types).of(cond)?;
        if !reads.iter().all(|param| scope.keys.get(*param).is_some()) {
            return None;
        }
        match self.consts.value(cond, &scope.keys) {
            Ok(Constant::Bool(holds)) => Some(holds),
            _ => None,
        }
    }
}
