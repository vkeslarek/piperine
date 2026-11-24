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
use super::instances::Instance;
use super::body::ModulePlan;
use super::elaborator::Elaboration;
use super::lower::Scope;
use piperine_ir::Terminal;
use crate::pom::{Arm, FileId, DeclId, DeclKind, Design, ExprId, ExprKind, PatId, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::Types;

/// A body to lower: a module and the values of its key params, one text per key param in the
/// order of the module's params (the text of a `Constant` tells two values apart exactly).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BodyKey {
    pub(super) module: DeclId,
    /// The generic arguments of the instance of a generic module the body lowers; `None` for a module as written.
    pub(super) instance: Option<Instance>,
    pub(super) values: Vec<String>,
}

impl BodyKey {
    /// The key of the one body of a module with no key param.
    pub(super) fn unkeyed(module: DeclId) -> BodyKey {
        BodyKey { module, instance: None, values: Vec::new() }
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
/// them: what a condition that depends on params only is made of. An expression written in a
/// hoisted frame reads through the placements that lead to it (`within`): a value input is the
/// caller's argument, so it reads what that argument reads.
pub(super) struct ParamReads<'a, 'd> {
    design: &'d Design,
    types: &'a Types<'d>,
    /// The placements, outermost first, between the body's own module and the frame the expression
    /// is written in; empty for an expression of the body's own module.
    placements: &'a [ExprId],
    /// Whether a read went through a value input of a placement.
    through_input: std::cell::Cell<bool>,
}

impl<'a, 'd> ParamReads<'a, 'd> {
    /// A reader of expressions written in the body's own module.
    pub(super) fn new(design: &'d Design, types: &'a Types<'d>) -> Self {
        ParamReads::within(design, types, &[])
    }

    /// A reader of expressions written in the frame that `placements` lead to.
    pub(super) fn within(design: &'d Design, types: &'a Types<'d>, placements: &'a [ExprId]) -> Self {
        ParamReads { design, types, placements, through_input: std::cell::Cell::new(false) }
    }

    /// Whether the type `ty` names a nature parameter of a generic module: a node of the nature a placement gives.
    fn is_nature_parameter_type(&self, ty: crate::pom::TypeId) -> bool {
        self.types.is_nature_parameter_type(self.design, ty)
    }

    /// Whether `decl` is a port of a value type: a value input.
    pub(super) fn is_value_input(&self, decl: DeclId) -> bool {
        matches!(&self.design.decl_data(decl).kind, DeclKind::Port(p) if self.types.carrier(decl).is_none() && self.design.nature_of(p.ty).is_none() && !self.is_nature_parameter_type(p.ty))
    }

    /// Whether the last `of` read went through a value input of a placement.
    pub(super) fn reads_an_input(&self) -> bool {
        self.through_input.get()
    }

    /// The params of the body's own module `expr` reads; `None` when it reads a probe, a carrier,
    /// the environment, a local or calls anything but a method of the catalogue.
    pub(super) fn of(&self, expr: ExprId) -> Option<BTreeSet<DeclId>> {
        let mut params = BTreeSet::new();
        self.collect(expr, self.placements.len(), &mut params).then_some(params)
    }

    /// Adds the params `expr` reads to `params`; `false` when it reads anything else. `depth` is how
    /// many of the placements the expression is written beneath.
    fn collect(&self, expr: ExprId, depth: usize, params: &mut BTreeSet<DeclId>) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(_) => true,
            // the bounds of a generation range, or of the range an array length is
            ExprKind::Range { lo, hi, .. } => lo.iter().chain(hi.iter()).all(|bound| self.collect(*bound, depth, params)),
            ExprKind::Paren(inner) | ExprKind::Cast { expr: inner, .. } | ExprKind::Unary { op: UnOp::Neg | UnOp::Not, expr: inner } => self.collect(*inner, depth, params),
            ExprKind::Binary { lhs, rhs, .. } => self.collect(*lhs, depth, params) && self.collect(*rhs, depth, params),
            ExprKind::MethodCall { recv, args, .. } if self.types.method(expr).is_some() => self.collect(*recv, depth, params) && args.iter().all(|a| self.collect(a.value, depth, params)),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => match &self.design.decl_data(decl).kind {
                    // a param of a placed callee is a constant of its frame, not a read of the body
                    DeclKind::Param(_) => {
                        if depth == 0 {
                            params.insert(decl);
                        }
                        true
                    }
                    DeclKind::Const(_) | DeclKind::Variant(_) => true,
                    DeclKind::Port(_) => self.collect_input(decl, depth, params),
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    /// Adds the params the caller's argument for the value input `input` reads; `false` when
    /// `input` is no value input of the placement the expression is beneath.
    fn collect_input(&self, input: DeclId, depth: usize, params: &mut BTreeSet<DeclId>) -> bool {
        if depth == 0 || !self.is_value_input(input) {
            return false;
        }
        let call = self.placements[depth - 1];
        let Some(crate::types::Callee::Module(callee)) = self.types.callee(call) else { return false };
        let ports = self.design.decl_data(callee).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::Port(_)));
        let Some(position) = ports.into_iter().position(|c| *c == input) else { return false };
        let ExprKind::Call { args, .. } = &self.design.expr_node(call).kind else { return false };
        let Some(argument) = args.get(position) else { return false };
        self.through_input.set(true);
        self.collect(argument.value, depth - 1, params)
    }
}

/// One contribution of a module's `analog` blocks as written: the two ends of its branch (`None`
/// is ground, the pair is sorted), its quantity, and the `if` and `match` conditions that enclose
/// it, outermost first.
pub(super) struct WrittenContribution {
    pub(super) ends: [Option<DeclId>; 2],
    pub(super) quantity: crate::pom::QuantityRole,
    pub(super) conditions: Vec<ExprId>,
    /// The right side of the contribution.
    pub(super) rhs: ExprId,
    /// The placements, outermost first, of the hoisted frame the contribution is written in (the
    /// conditions and the right side are read there); empty for the module's own.
    pub(super) placements: Vec<ExprId>,
}

/// The walk that lists the contributions of statements with the conditions around them.
struct ContributionWalk<'e, 'd, 't> {
    elaboration: &'e mut Elaboration<'d, 't>,
    /// The key params the walk folds conditions over; none folds nothing.
    keys: Bindings,
    conditions: Vec<ExprId>,
    found: Vec<WrittenContribution>,
    /// The placements the walk is beneath: empty in the module's own `analog` blocks.
    placements: Vec<ExprId>,
}

impl ContributionWalk<'_, '_, '_> {
    /// The contributions of the `analog` blocks of `module`, then of every module it hoists, each
    /// beneath its placement.
    fn module(&mut self, module: DeclId) {
        let DeclKind::Module(data) = &self.elaboration.design.decl_data(module).kind else { return };
        let (blocks, calls) = (data.behaviors.clone(), data.instances.clone());
        for block in blocks {
            if let DeclKind::Behavior(behavior) = &self.elaboration.design.decl_data(block).kind.clone()
                && behavior.kind == crate::pom::BehaviorKind::Analog
            {
                self.statements(&behavior.body);
            }
        }
        for call in calls {
            let Some(callee) = self.elaboration.hoisted_callee(call) else { continue };
            // a module that places itself is reported when it is hoisted (`E0341`), not walked forever
            if self.placements.iter().any(|p| self.elaboration.hoisted_callee(*p) == Some(callee)) {
                continue;
            }
            self.placements.push(call);
            self.module(callee);
            self.placements.pop();
        }
    }

    /// The contributions among `stmts` and what they hold, in source order.
    fn statements(&mut self, stmts: &[StmtId]) {
        for stmt in stmts {
            match self.elaboration.design.stmt_node(*stmt).kind.clone() {
                StmtKind::Contribution { lhs, rhs } => self.contribution(lhs, rhs),
                StmtKind::Expr { expr, .. } => self.expression(expr),
                StmtKind::Let { init: Some(init), .. } => self.expression(init),
                _ => {}
            }
        }
    }

    /// The contributions inside `expr`: in a block, an `if` or a `match` it is.
    fn expression(&mut self, expr: ExprId) {
        crate::stack::Stack::deep(|| match self.elaboration.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.expression(inner),
            ExprKind::Block(block) => self.statements(&block.stmts),
            // A condition over the key params folds: only the arm it selects is written (`lower.rs`).
            ExprKind::If { cond, then, else_ } => match self.elaboration.folded_condition(&self.keys, &self.placements, cond) {
                Some(true) => self.statements(&then.stmts),
                Some(false) => {
                    if let Some(otherwise) = else_ {
                        self.expression(otherwise);
                    }
                }
                None => {
                    self.conditions.push(cond);
                    self.statements(&then.stmts);
                    if let Some(otherwise) = else_ {
                        self.expression(otherwise);
                    }
                    self.conditions.pop();
                }
            },
            // A scrutinee over the key params folds like an `if` condition (§11 rule 3): only the
            // arm it selects is written.
            ExprKind::Match { scrutinee, arms } => match self.elaboration.folded_arm(&self.keys, &self.placements, scrutinee, &arms) {
                Some(selected) => self.expression(arms[selected].body),
                None => {
                    self.conditions.push(scrutinee);
                    for arm in arms {
                        self.expression(arm.body);
                    }
                    self.conditions.pop();
                }
            },
            // A loop that contributes is unrolled (`unroll.rs`): its contributions are written once,
            // whatever the trip count, and the kind of a branch does not depend on it.
            ExprKind::For { body, .. } | ExprKind::While { body, .. } | ExprKind::Loop { body } => self.statements(&body.stmts),
            _ => {}
        });
    }

    /// Records the contribution whose left side is `lhs`, when it names a probe of one or two nodes.
    fn contribution(&mut self, lhs: ExprId, rhs: ExprId) {
        let design = self.elaboration.design;
        let ExprKind::Call { callee, args, .. } = &design.expr_node(lhs).kind else { return };
        let Some(quantity) = self.elaboration.quantity_role(*callee) else { return };
        let mut ends: Vec<Option<DeclId>> = Vec::new();
        for arg in args {
            let Some(end) = self.elaboration.terminal_in(arg.value, &self.placements) else { return };
            ends.push(end);
        }
        let mut ends = match ends[..] {
            [a] => [a, None],
            [a, b] => [a, b],
            _ => return,
        };
        ends.sort();
        self.found.push(WrittenContribution { ends, quantity, conditions: self.conditions.clone(), rhs, placements: self.placements.clone() });
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

    /// The contributions `module` writes in its `analog` blocks and `analog` blocks of the modules it
    /// hoists (a module with a value input or a result), with the conditions that enclose each; a
    /// condition over the key params `keys` folds and is not one of them.
    pub(super) fn written_contributions(&mut self, module: DeclId, keys: &Bindings) -> Vec<WrittenContribution> {
        let mut walk = ContributionWalk { elaboration: self, keys: keys.clone(), conditions: Vec::new(), found: Vec::new(), placements: Vec::new() };
        walk.module(module);
        walk.found
    }

    /// The node `arg` names as an end of a branch written in the frame `placements` lead to, read
    /// as the body's own module has it: a node port of a placed callee is the node its argument
    /// names, one level up. `None` for a node the callee owns, or anything but a node.
    pub(super) fn terminal_in(&self, arg: ExprId, placements: &[ExprId]) -> Option<Option<DeclId>> {
        let Some(decl) = self.terminal_decl(arg)? else { return Some(None) };
        let [outer @ .., call] = placements else { return Some(Some(decl)) };
        let Some(crate::types::Callee::Module(callee)) = self.types.callee(*call) else { return None };
        let ports = self.design.decl_data(callee).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::Port(_)));
        let position = ports.into_iter().position(|c| *c == decl)?;
        let ExprKind::Call { args, .. } = &self.design.expr_node(*call).kind else { return None };
        self.terminal_in(args.get(position)?.value, outer)
    }

    /// The contributions of `module` to a branch that has a potential contribution among them.
    fn potential_capable_contributions(&mut self, module: DeclId) -> Vec<WrittenContribution> {
        let found = self.written_contributions(module, &Bindings::new());
        let potential: Vec<[Option<DeclId>; 2]> = found.iter().filter(|c| c.quantity == crate::pom::QuantityRole::Potential).map(|c| c.ends).collect();
        found.into_iter().filter(|c| potential.contains(&c.ends)).collect()
    }

    /// Classifies the key params of `modules` (the reached modules with a body): see the module
    /// documentation. Run once, before any body is lowered.
    pub(super) fn classify_key_params(&mut self, modules: &[DeclId]) {
        // A param of module type is structural (§8.4.6): its template selects what is placed.
        for module in modules {
            let order = self.params_of(*module);
            for param in order.clone() {
                if self.is_module_param(param) {
                    self.keys.add(*module, param, &order);
                }
            }
        }
        for module in modules {
            let order = self.params_of(*module);
            for contribution in self.potential_capable_contributions(*module) {
                for condition in &contribution.conditions {
                    for param in ParamReads::within(self.design, self.types, &contribution.placements).of(*condition).into_iter().flatten() {
                        self.keys.add(*module, param, &order);
                    }
                }
            }
        }
        // A param read by the initial value of a node or a carrier is structural too: the body folds it.
        for module in modules {
            let order = self.params_of(*module);
            for param in self.params_read_by_initial_values(*module) {
                self.keys.add(*module, param, &order);
            }
        }
        // A param read by the length of an array of carriers or nodes is structural too (§8.1.2, QC2).
        for module in modules {
            let order = self.params_of(*module);
            for param in self.params_read_by_array_lengths(*module) {
                self.keys.add(*module, param, &order);
            }
        }
        // A param read by the range or the condition of a generation is structural too (§8.2.7, GN-20).
        for module in modules {
            let order = self.params_of(*module);
            for param in self.params_read_by_generation(*module) {
                self.keys.add(*module, param, &order);
            }
        }
        // A param read by a const generic argument of a placement is structural too (`Taps::<{ count }>`, GN-20).
        for module in modules {
            let order = self.params_of(*module);
            for param in self.params_read_by_generic_arguments(*module) {
                self.keys.add(*module, param, &order);
            }
        }
        // A param of a child that the parent reads through `inst.name` is fixed by the placement.
        for (module, param) in self.params_read_through_reaches() {
            if modules.contains(&module) {
                let order = self.params_of(module);
                self.keys.add(module, param, &order);
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

    /// The params of `module` that the length of one of its arrays of carriers or nodes reads (`signal s: [Bit; n]`).
    fn params_read_by_array_lengths(&self, module: DeclId) -> Vec<DeclId> {
        let mut found = Vec::new();
        for child in &self.design.decl_data(module).children {
            let Some(mut at) = self.types.carrier_array(*child).or_else(|| self.types.node_array(*child)) else { continue };
            while let crate::types::Ty::Array(element, length) = at {
                if let crate::types::Length::Generic(crate::types::ConstTerm::Expr(expr)) = length {
                    found.extend(ParamReads::new(self.design, self.types).of(*expr).into_iter().flatten());
                }
                at = element;
            }
        }
        found
    }

    /// The params of `module` that the range of a generation `for`, or the condition of a generation `if`, reads.
    fn params_read_by_generation(&self, module: DeclId) -> Vec<DeclId> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let mut found = Vec::new();
        // the generation of the body, and of the `digital` blocks, which are written beside the module
        let mut pending: Vec<DeclId> = self.design.decl_data(module).children.iter().chain(&data.behaviors).copied().collect();
        while let Some(decl) = pending.pop() {
            let item = self.design.decl_data(decl);
            match &item.kind {
                DeclKind::Generation(crate::pom::GenerationData::For { iter, .. }) => self.collect_params(*iter, &mut found),
                DeclKind::Generation(crate::pom::GenerationData::If { cond, .. }) => self.collect_params(*cond, &mut found),
                _ => {}
            }
            if matches!(item.kind, DeclKind::Generation(_) | DeclKind::Behavior(_)) {
                pending.extend(item.children.iter().copied());
            }
        }
        found
    }

    /// Adds every param `expr` reads, whatever else it reads, to `found`.
    fn collect_params(&self, expr: ExprId, found: &mut Vec<DeclId>) {
        let node = self.design.expr_node(expr);
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res)
            && matches!(self.design.decl_data(decl).kind, DeclKind::Param(_))
        {
            found.push(decl);
        }
        for child in crate::pom::expr_children(&node.kind, self.design) {
            self.collect_params(child, found);
        }
    }

    /// The params of `module` that a const generic argument of one of its placements reads (`Taps::<{ count }>`).
    fn params_read_by_generic_arguments(&self, module: DeclId) -> Vec<DeclId> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let mut found = Vec::new();
        for call in &data.instances {
            let Some(instantiation) = self.types.instantiation(*call) else { continue };
            for argument in &instantiation.arguments {
                if let crate::types::GenericValue::Const(crate::types::ConstTerm::Expr(expr)) = argument {
                    found.extend(ParamReads::new(self.design, self.types).of(*expr).into_iter().flatten());
                }
            }
        }
        found
    }

    /// The params of `module` that the initial value of one of its nodes or carriers reads (`signal a:
    /// Electrical = v0`, a port's default).
    fn params_read_by_initial_values(&self, module: DeclId) -> Vec<DeclId> {
        let mut found = Vec::new();
        for child in &self.design.decl_data(module).children {
            let init = match &self.design.decl_data(*child).kind {
                DeclKind::Signal(s) => s.init,
                DeclKind::Port(p) if self.types.carrier(*child).is_some() => p.default,
                _ => None,
            };
            if let Some(params) = init.and_then(|expr| ParamReads::new(self.design, self.types).of(expr)) {
                found.extend(params);
            }
        }
        found
    }

    /// The params of `module` read by a value `module` gives to a key param of a module it
    /// places, as (the placed module, the param of `module`).
    fn params_that_feed_key_params(&self, module: DeclId) -> Vec<(DeclId, DeclId)> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let mut out = Vec::new();
        for call in &data.instances {
            let Some(crate::types::Callee::Module(child)) = self.types.callee(*call) else { continue };
            if self.hoisted_callee(*call).is_some() {
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
    pub(super) fn request_body(&mut self, module: DeclId, instance: Option<&Instance>, values: &Bindings) -> Option<(BodyKey, BodyId)> {
        let keys = self.keys.of(module).to_vec();
        if keys.is_empty() && instance.is_none() {
            return Some((BodyKey::unkeyed(module), self.body_ids.get(&module).copied()?));
        }
        let mut keyed = Bindings::new();
        for key in &keys {
            self.bind_param(&mut keyed, *key, values.get(*key)?);
        }
        let key = self.body_key(module, instance, values)?;
        if let Some(id) = self.body_variants.ids.get(&key) {
            return Some((key.clone(), *id));
        }
        self.admit_instance_depth(&key)?;
        let id = BodyId(self.body_variants.next);
        self.body_variants.next += 1;
        self.body_variants.ids.insert(key.clone(), id);
        self.body_variants.pending.push_back((key.clone(), keyed));
        Some((key, id))
    }

    /// The key of the body `module` gets when its params hold `values`. `None` when a key param has no value in `values`.
    pub(super) fn body_key(&self, module: DeclId, instance: Option<&Instance>, values: &Bindings) -> Option<BodyKey> {
        let texts = self.keys.of(module).iter().map(|key| values.get(*key).map(|value| value.to_string())).collect::<Option<Vec<_>>>()?;
        Some(BodyKey { module, instance: instance.cloned(), values: texts })
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
    pub(super) fn body_plan(&mut self, module: DeclId, instance: Option<&Instance>, values: &Bindings) -> Option<ModulePlan> {
        let (key, _) = self.request_body(module, instance, values)?;
        self.lower_pending_bodies();
        self.plans.get(&key).cloned()
    }

    /// The name of the body of `key`: the module's, then `~param=value` for each key param.
    pub(super) fn body_name(&self, key: &BodyKey) -> String {
        let name = match &key.instance {
            Some(instance) => instance.name(self),
            None => self.design.decl_data(key.module).name.as_ref().map_or_else(String::new, |n| n.text.clone()),
        };
        if key.values.is_empty() {
            return name;
        }
        let pairs: Vec<String> = self.keys.of(key.module).iter().zip(&key.values).map(|(param, value)| format!("{}={value}", self.design.decl_data(*param).name.as_ref().map_or("", |n| n.text.as_str()))).collect();
        format!("{name}~{}", pairs.join(","))
    }
}


impl Elaboration<'_, '_> {
    /// The value of the condition `cond`, written in the frame `placements` lead to, when it reads
    /// only the key params `keys` binds and constants (through the value inputs of the placements:
    /// the caller's arguments, which read key params only); `None` for any other condition, which is
    /// lowered as written.
    pub(super) fn folded_condition(&mut self, keys: &Bindings, placements: &[ExprId], cond: ExprId) -> Option<bool> {
        let bindings = self.fold_bindings(keys, placements, cond)?;
        match self.consts.value(cond, &bindings) {
            Ok(Constant::Bool(holds)) => Some(holds),
            _ => None,
        }
    }

    /// What `expr`, written in the frame `placements` lead to, is evaluated over for a fold: `keys`
    /// at the body's own module, and beneath a placement the callee's params and the values of its
    /// value inputs, each the caller's argument evaluated one level up. `None` when `expr` is not a
    /// value of the key params alone, or (beneath a placement) reads no value input, which
    /// the frame lowers as written as it always did.
    fn fold_bindings(&mut self, keys: &Bindings, placements: &[ExprId], expr: ExprId) -> Option<Bindings> {
        let reads = ParamReads::within(self.design, self.types, placements);
        let params = reads.of(expr)?;
        if keys.is_empty() || !params.iter().all(|param| keys.get(*param).is_some()) || (!placements.is_empty() && !reads.reads_an_input()) {
            return None;
        }
        let mut level = keys.clone();
        for call in placements {
            level = self.placement_bindings(*call, &level)?;
        }
        Some(level)
    }

    /// The values the value inputs and params of the callee of the placement `call` take, the
    /// arguments read over `caller`, the bindings of the frame the placement is written in.
    fn placement_bindings(&mut self, call: ExprId, caller: &Bindings) -> Option<Bindings> {
        let Some(crate::types::Callee::Module(callee)) = self.types.callee(call) else { return None };
        let ExprKind::Call { args, .. } = self.design.expr_node(call).kind.clone() else { return None };
        let ports: Vec<DeclId> = self.design.decl_data(callee).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_))).collect();
        let mut bound = Bindings::new();
        for (position, port) in ports.iter().enumerate() {
            if self.is_value_input(*port) {
                let value = self.consts.value(args.get(position)?.value, caller).ok()?;
                bound.bind(*port, value);
            }
        }
        let fields = self.placement_fields(call, callee);
        for param in self.params_of(callee) {
            let DeclKind::Param(data) = &self.design.decl_data(param).kind else { continue };
            let written = fields.iter().find(|f| f.res == Resolution::Decl(param)).map(|f| f.value);
            let value = self.consts.value(written.or(data.default)?, &Bindings::new()).ok()?;
            bound.bind(param, value);
        }
        Some(bound)
    }
}

impl Elaboration<'_, '_> {
    /// The position of the arm a `match` on `scrutinee` selects when the scrutinee reads only the
    /// key params `keys` binds and constants (§11 rule 3: a selector over params only is fixed at
    /// elaboration, an `if` and a `match` alike). `None` when the scrutinee is not such a value,
    /// an arm has a guard or a pattern that binds, or no arm matches: the `match` is then lowered
    /// as written, a run-time selector.
    pub(super) fn folded_arm(&mut self, keys: &Bindings, placements: &[ExprId], scrutinee: ExprId, arms: &[Arm]) -> Option<usize> {
        let keys = &self.fold_bindings(keys, placements, scrutinee)?;
        let subject = self.consts.value(scrutinee, keys).ok()?;
        for (position, arm) in arms.iter().enumerate() {
            if arm.guard.is_some() {
                return None;
            }
            if self.constant_pattern_holds(keys, arm.pat, subject)? {
                return Some(position);
            }
        }
        None
    }

    /// Whether the pattern `pat` accepts the constant `subject`; `None` for a pattern that binds
    /// or destructures, which no fold can decide.
    pub(super) fn constant_pattern_holds(&mut self, keys: &Bindings, pat: PatId, subject: Constant) -> Option<bool> {
        match self.design.pat_node(pat).kind.clone() {
            PatKind::Wild => Some(true),
            PatKind::Lit(expr) => Some(self.consts.value(expr, keys).ok()? == subject),
            PatKind::Path(path) => {
                let res = self.design.path_node(path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                Some(self.named_constant(res)? == subject)
            }
            PatKind::Ident { res, sub: None, .. } => Some(self.named_constant(res)? == subject),
            PatKind::Or(alternatives) => {
                let mut holds = false;
                for alternative in alternatives {
                    holds |= self.constant_pattern_holds(keys, alternative, subject)?;
                }
                Some(holds)
            }
            PatKind::Range { lo, hi, inclusive } => {
                let above = match lo {
                    Some(lo) => Self::at_least(subject, self.consts.value(lo, keys).ok()?)?,
                    None => true,
                };
                let below = match hi {
                    Some(hi) => Self::at_most(subject, self.consts.value(hi, keys).ok()?, inclusive)?,
                    None => true,
                };
                Some(above && below)
            }
            _ => None,
        }
    }

    /// The value of a `const` or a unit variant a pattern names (its tag); `None` for any other name.
    fn named_constant(&mut self, res: Resolution) -> Option<Constant> {
        let Resolution::Decl(decl) = res else { return None };
        match self.design.decl_data(decl).kind {
            DeclKind::Const(_) => self.consts.constant(decl).ok(),
            DeclKind::Variant(_) => super::compound::EnumLayout::variant_tag(self.design, decl).map(Constant::Int),
            _ => None,
        }
    }

    /// `subject >= bound` for two integers or two reals.
    fn at_least(subject: Constant, bound: Constant) -> Option<bool> {
        match (subject, bound) {
            (Constant::Int(a), Constant::Int(b)) => Some(a >= b),
            (Constant::Real(a), Constant::Real(b)) => Some(a.get() >= b.get()),
            _ => None,
        }
    }

    /// `subject <= bound` (`<` when not `inclusive`) for two integers or two reals.
    fn at_most(subject: Constant, bound: Constant, inclusive: bool) -> Option<bool> {
        match (subject, bound) {
            (Constant::Int(a), Constant::Int(b)) => Some(if inclusive { a <= b } else { a < b }),
            (Constant::Real(a), Constant::Real(b)) => Some(if inclusive { a.get() <= b.get() } else { a.get() < b.get() }),
            _ => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// The branches the body of `module`, lowered with the key params `scope.keys`, merges: a branch
    /// whose every contribution is a potential contribution of the constant zero that no condition
    /// selects (after the folds), and whose flow no expression reads (§11 rule 3). Their
    /// contributions are not lowered and their two ends are one node in every instance.
    pub(super) fn merged_branches(&mut self, scope: &Scope, module: DeclId) -> Vec<(Terminal, Terminal)> {
        let found = self.written_contributions(module, &scope.keys);
        let read = self.flow_reads_in(module);
        let mut ends: Vec<[Option<DeclId>; 2]> = Vec::new();
        for contribution in &found {
            if !ends.contains(&contribution.ends) {
                ends.push(contribution.ends);
            }
        }
        let mut merged = Vec::new();
        for pair in ends {
            let all_zero = found.iter().filter(|c| c.ends == pair).all(|c| c.quantity == crate::pom::QuantityRole::Potential && c.conditions.is_empty() && self.is_constant_zero(c.rhs));
            if all_zero && !read.contains(&pair) {
                let terminal = |end: Option<DeclId>| end.and_then(|d| self.frames[0].nodes.get(&d).copied()).unwrap_or(Terminal::Ground);
                merged.push((terminal(pair[0]), terminal(pair[1])));
            }
        }
        merged
    }

    /// Whether `expr` folds to the real zero.
    fn is_constant_zero(&mut self, expr: ExprId) -> bool {
        matches!(self.consts.value(expr, &Bindings::new()), Ok(Constant::Real(value)) if value.get() == 0.0)
    }

    /// The branches (sorted pairs of ends) whose flow an expression of `module`, or of a module it
    /// hoists, reads: a call of the flow quantity that is not the left side of a contribution.
    fn flow_reads_in(&self, module: DeclId) -> Vec<[Option<DeclId>; 2]> {
        let written: BTreeSet<ExprId> = (0..self.design.stmt_count() as u32).filter_map(|k| match &self.design.stmt_node(StmtId(k)).kind {
            StmtKind::Contribution { lhs, .. } => Some(*lhs),
            _ => None,
        }).collect();
        let mut reads = Vec::new();
        self.collect_flow_reads(module, &[], &written, &mut reads);
        reads
    }

    /// Adds to `reads` the branches the flow reads of `module` (written beneath `placements`) and of
    /// the modules it hoists name, the ends read as the body's own module has them.
    fn collect_flow_reads(&self, module: DeclId, placements: &[ExprId], written: &BTreeSet<ExprId>, reads: &mut Vec<[Option<DeclId>; 2]>) {
        for id in self.module_exprs(module) {
            let node = self.design.expr_node(id);
            let ExprKind::Call { callee, args, .. } = &node.kind else { continue };
            if written.contains(&id) || self.quantity_role(*callee) != Some(crate::pom::QuantityRole::Flow) {
                continue;
            }
            let ends: Option<Vec<Option<DeclId>>> = args.iter().map(|a| self.terminal_in(a.value, placements)).collect();
            let mut pair = match ends.as_deref() {
                Some([a]) => [*a, None],
                Some([a, b]) => [*a, *b],
                _ => continue,
            };
            pair.sort();
            reads.push(pair);
        }
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return };
        for call in &data.instances {
            let Some(callee) = self.hoisted_callee(*call) else { continue };
            if placements.iter().any(|p| self.hoisted_callee(*p) == Some(callee)) || callee == module {
                continue;
            }
            let inner: Vec<ExprId> = placements.iter().copied().chain([*call]).collect();
            self.collect_flow_reads(callee, &inner, written, reads);
        }
    }
}

/// The before-run bindings of params a host gives the elaborator (`Elaborator::run_with`):
/// `<instance>.<param>` and the value, the instance as its full path or relative to the top. A
/// binding of a key param selects the variant of that value at that instance; a binding of any other
/// param is ignored here, it stays the host's write before the run (design mechanism 4, C6).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    /// The bindings, in the order given.
    entries: Vec<(String, Given)>,
}

/// A value a host binds to a key param before the run: a number, or a text the elaborator reads by the param's type.
#[derive(Debug, Clone, PartialEq)]
enum Given {
    /// A number, as the type of the param's default holds it.
    Number(f64),
    /// A text: an integer literal, a variant name, `None`, `Some(value)`, a value of `T` for an `Option<T>`.
    Text(String),
}

impl Overrides {
    /// No binding: what `Elaborator::run` elaborates with.
    pub fn none() -> Overrides {
        Overrides::default()
    }

    /// The bindings `entries` (`<instance>.<param>`, value), in the order given; a later entry for
    /// the same param wins, as it does when the host writes them in order.
    pub fn new(entries: &[(String, f64)]) -> Overrides {
        Overrides { entries: entries.iter().map(|(entry, value)| (entry.clone(), Given::Number(*value))).collect() }
    }

    /// These bindings and the texts `texts` (`<instance>.<param>`, text) after them: each read by the type of its param, so a
    /// unit enum's variant name, `None` and `Some(value)` select the variant of their value (PI-02, PI-03).
    pub fn with_texts(mut self, texts: &[(String, String)]) -> Overrides {
        self.entries.extend(texts.iter().map(|(entry, text)| (entry.clone(), Given::Text(text.clone()))));
        self
    }

    /// The value bound to `param` of the instance at `instance`, written in full or relative to
    /// `top` (the first segment of every instance path).
    fn value(&self, top: &str, instance: &str, param: &str) -> Option<&Given> {
        self.entries.iter().rev().find_map(|(entry, value)| {
            let (path, name) = entry.rsplit_once('.')?;
            (name == param && (path == instance || format!("{top}.{path}") == instance)).then_some(value)
        })
    }
}

impl Elaboration<'_, '_> {
    /// `values` and `bindings` of the instance `path`, with each key param the host bound before the
    /// run (`overrides`) set to that value, of the type of the param's default.
    pub(super) fn apply_overrides(&mut self, module: DeclId, path: &str, plans: &[super::body::ParamPlan], mut values: Vec<Constant>, mut bindings: Bindings, at: (FileId, crate::msl::Span)) -> (Vec<Constant>, Bindings) {
        let top = path.split('.').next().unwrap_or(path);
        for key in self.keys.of(module).to_vec() {
            let Some(index) = plans.iter().position(|p| p.decl == key) else { continue };
            let Some(given) = self.overrides.value(top, path, &plans[index].name).cloned() else { continue };
            // D13: the host has no text for a template; the param is bound in the source.
            if self.is_module_param(key) {
                let message = format!("`{path}.{}` is a module-typed param: it is bound in the source, a host cannot give it the value {given:?}", plans[index].name);
                self.report(at.0, crate::msl::Diagnostic::new("E0802", message, at.1, "a module-typed param is bound in the source"));
                continue;
            }
            let value = match (&given, plans[index].default) {
                (Given::Text(text), _) => match self.constant_of_text(key, text) {
                    Ok(value) => value,
                    Err(why) => {
                        let message = format!("`{path}.{}` refuses the value `{text}`: {why}", plans[index].name);
                        self.report(at.0, crate::msl::Diagnostic::new("E0802", message, at.1, "the value a host gave"));
                        continue;
                    }
                },
                (Given::Number(given), Some(Constant::Bool(_))) => Constant::Bool(*given != 0.0),
                (Given::Number(given), Some(Constant::Int(_))) => Constant::Int(*given as i128),
                (Given::Number(given), _) => Constant::Real(piperine_ir::Real::new(*given)),
            };
            if let Some(slot) = values.get_mut(index) {
                *slot = value;
            }
            self.bind_param(&mut bindings, key, value);
        }
        (values, bindings)
    }
}
