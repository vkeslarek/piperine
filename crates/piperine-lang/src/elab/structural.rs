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
use piperine_ir::Terminal;
use crate::pom::{Arm, DeclId, DeclKind, Design, ExprId, ExprKind, PatId, PatKind, Resolution, StmtId, StmtKind, UnOp};
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
pub(super) struct WrittenContribution {
    pub(super) ends: [Option<DeclId>; 2],
    pub(super) quantity: crate::pom::QuantityRole,
    pub(super) conditions: Vec<ExprId>,
    /// The right side of the contribution.
    pub(super) rhs: ExprId,
}

/// The walk that lists the contributions of statements with the conditions around them.
struct ContributionWalk<'e, 'd, 't> {
    elaboration: &'e mut Elaboration<'d, 't>,
    /// The key params the walk folds conditions over; none folds nothing.
    keys: Bindings,
    conditions: Vec<ExprId>,
    found: Vec<WrittenContribution>,
}

impl ContributionWalk<'_, '_, '_> {
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
        crate::stack::deep(|| match self.elaboration.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.expression(inner),
            ExprKind::Block(block) => self.statements(&block.stmts),
            // A condition over the key params folds: only the arm it selects is written (`lower.rs`).
            ExprKind::If { cond, then, else_ } => match self.elaboration.folded_condition(&self.keys, cond) {
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
            ExprKind::Match { scrutinee, arms } => match self.elaboration.folded_arm(&self.keys, scrutinee, &arms) {
                Some(selected) => self.expression(arms[selected].body),
                None => {
                    self.conditions.push(scrutinee);
                    for arm in arms {
                        self.expression(arm.body);
                    }
                    self.conditions.pop();
                }
            },
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
            let Some(end) = self.elaboration.terminal_decl(arg.value) else { return };
            ends.push(end);
        }
        let mut ends = match ends[..] {
            [a] => [a, None],
            [a, b] => [a, b],
            _ => return,
        };
        ends.sort();
        self.found.push(WrittenContribution { ends, quantity, conditions: self.conditions.clone(), rhs });
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

    /// The contributions `module` writes in its `analog` blocks, with the conditions that enclose
    /// each; a condition over the key params `keys` folds and is not one of them.
    pub(super) fn written_contributions(&mut self, module: DeclId, keys: &Bindings) -> Vec<WrittenContribution> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return Vec::new() };
        let blocks = data.behaviors.clone();
        let mut walk = ContributionWalk { elaboration: self, keys: keys.clone(), conditions: Vec::new(), found: Vec::new() };
        for block in blocks {
            if let DeclKind::Behavior(behavior) = &walk.elaboration.design.decl_data(block).kind.clone()
                && behavior.kind == crate::pom::BehaviorKind::Analog
            {
                walk.statements(&behavior.body);
            }
        }
        walk.found
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
    pub(super) fn folded_condition(&mut self, keys: &Bindings, cond: ExprId) -> Option<bool> {
        if keys.is_empty() {
            return None;
        }
        let reads = ParamReads::new(self.design, self.types).of(cond)?;
        if !reads.iter().all(|param| keys.get(*param).is_some()) {
            return None;
        }
        match self.consts.value(cond, keys) {
            Ok(Constant::Bool(holds)) => Some(holds),
            _ => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// The position of the arm a `match` on `scrutinee` selects when the scrutinee reads only the
    /// key params `keys` binds and constants (§11 rule 3: a selector over params only is fixed at
    /// elaboration, an `if` and a `match` alike). `None` when the scrutinee is not such a value,
    /// an arm has a guard or a pattern that binds, or no arm matches: the `match` is then lowered
    /// as written, a run-time selector.
    pub(super) fn folded_arm(&mut self, keys: &Bindings, scrutinee: ExprId, arms: &[Arm]) -> Option<usize> {
        if keys.is_empty() {
            return None;
        }
        let reads = ParamReads::new(self.design, self.types).of(scrutinee)?;
        if !reads.iter().all(|param| keys.get(*param).is_some()) {
            return None;
        }
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
    fn constant_pattern_holds(&mut self, keys: &Bindings, pat: PatId, subject: Constant) -> Option<bool> {
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

    /// The branches (sorted pairs of ends) whose flow an expression of `module` reads: a call of
    /// the flow quantity that is not the left side of a contribution.
    fn flow_reads_in(&self, module: DeclId) -> Vec<[Option<DeclId>; 2]> {
        let written: BTreeSet<ExprId> = (0..self.design.stmt_count() as u32).filter_map(|k| match &self.design.stmt_node(StmtId(k)).kind {
            StmtKind::Contribution { lhs, .. } => Some(*lhs),
            _ => None,
        }).collect();
        let mut reads = Vec::new();
        for id in self.module_exprs(module) {
            let node = self.design.expr_node(id);
            let ExprKind::Call { callee, args, .. } = &node.kind else { continue };
            if written.contains(&id) || self.quantity_role(*callee) != Some(crate::pom::QuantityRole::Flow) {
                continue;
            }
            let ends: Option<Vec<Option<DeclId>>> = args.iter().map(|a| self.terminal_decl(a.value)).collect();
            let mut pair = match ends.as_deref() {
                Some([a]) => [*a, None],
                Some([a, b]) => [*a, *b],
                _ => continue,
            };
            pair.sort();
            reads.push(pair);
        }
        reads
    }
}

/// The before-run bindings of params a host gives the elaborator (`Elaborator::run_with`):
/// `<instance>.<param>` and the value, the instance as its full path or relative to the top. A
/// binding of a key param selects the variant of that value at that instance; a binding of any other
/// param is ignored here, it stays the host's write before the run (design mechanism 4, C6).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    entries: Vec<(String, f64)>,
}

impl Overrides {
    /// No binding: what `Elaborator::run` elaborates with.
    pub fn none() -> Overrides {
        Overrides::default()
    }

    /// The bindings `entries` (`<instance>.<param>`, value), in the order given; a later entry for
    /// the same param wins, as it does when the host writes them in order.
    pub fn new(entries: &[(String, f64)]) -> Overrides {
        Overrides { entries: entries.to_vec() }
    }

    /// The value bound to `param` of the instance at `instance`, written in full or relative to
    /// `top` (the first segment of every instance path).
    fn value(&self, top: &str, instance: &str, param: &str) -> Option<f64> {
        self.entries.iter().rev().find_map(|(entry, value)| {
            let (path, name) = entry.rsplit_once('.')?;
            (name == param && (path == instance || format!("{top}.{path}") == instance)).then_some(*value)
        })
    }
}

impl Elaboration<'_, '_> {
    /// `values` and `bindings` of the instance `path`, with each key param the host bound before the
    /// run (`overrides`) set to that value, of the type of the param's default.
    pub(super) fn apply_overrides(&self, module: DeclId, path: &str, plans: &[super::body::ParamPlan], mut values: Vec<Constant>, mut bindings: Bindings) -> (Vec<Constant>, Bindings) {
        let top = path.split('.').next().unwrap_or(path);
        for key in self.keys.of(module) {
            let Some(index) = plans.iter().position(|p| p.decl == *key) else { continue };
            let Some(given) = self.overrides.value(top, path, &plans[index].name) else { continue };
            let value = match plans[index].default {
                Some(Constant::Bool(_)) => Constant::Bool(given != 0.0),
                Some(Constant::Int(_)) => Constant::Int(given as i128),
                _ => Constant::Real(piperine_ir::Real::new(given)),
            };
            if let Some(slot) = values.get_mut(index) {
                *slot = value;
            }
            bindings.bind(*key, value);
        }
        (values, bindings)
    }
}
