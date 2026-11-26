//! `inst.name` through children of children, at any depth (§8.2.8, §8.4.8): `bank.c0.sn`, `p.mid.c0.sn`, `a.b.c.d.x`, each child
//! placed by name, through a template or in generation. A body reads the items of its own children, never of a grandchild's, so
//! the child's body carries a reached node (or carrier) for the item (`NodeRole::Reached`, bound to the grandchild's own when it
//! is placed) and the body that reads it reaches that one: a read of `a.b.c.x` is a reach of `a`'s body into `b.c.x`, which is a
//! reach of `b`'s body into `c.x`, one hop for each name, every hop the same.
//!
//! * A **demand** is one such hop: the body of `module` reaches `child.path` (the names after the child, the item last). A body
//!   lowers a reached slot for every demand it meets, once for each iteration of the placement it names, right after its own nodes
//!   and its own carriers (`lower_nested_demands`, `lower_nested_carrier_demands`), so the slot's index is known to the body that
//!   reads it (`DemandEntry`).
//! * The demands are what the design reads. The names it writes are found before the first run (`reach/seed.rs`: each hop the checker
//!   resolved to a placement, and the reads the `let` a path ends on makes through its own children); a hop the text does not name a
//!   module for (a read through a type-parameter template) is found by running: a hop whose demand the run's tables lack records it
//!   (`NestedDemands::require`) and the run is made again with it (`Elaborator::run_with`). A read of `a.b.c.x` asks `a`'s module
//!   for `b.c.x`; that demand, met, asks `b`'s module for `c.x`.
//! * The reader makes a reach frame for each hop (`reach/fields.rs`), and the slot of a node or carrier of the last is the entry of the
//!   first hop's module that names it (`Elaboration::nested_slot`).

use piperine_ir::Constant;

use crate::elab::consts::Bindings;
use crate::elab::elaborator::Elaboration;
use crate::elab::generation::Iteration;
use crate::elab::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind};

/// One hop of a read through children of children: the body of `module` reaches the item at the end of `path` below its child
/// `child`, a `let` that places a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::elab) struct NestedDemand {
    /// The module whose body holds the child.
    pub(in crate::elab) module: DeclId,
    /// The `let` binding of the placement in the module's body.
    pub(in crate::elab) child: DeclId,
    /// The names below the child: the children of the child it passes through, then the item.
    pub(in crate::elab) path: Vec<String>,
}

/// The demands a run knows and the ones it found missing.
#[derive(Debug, Default)]
pub(in crate::elab) struct NestedDemands {
    /// The demands the run lowers slots for, in the order they were found.
    known: Vec<NestedDemand>,
    /// The demands a read met that `known` lacks.
    wanted: Vec<NestedDemand>,
}

impl NestedDemands {
    /// The tables of a run that knows `demands`.
    pub(in crate::elab) fn known(demands: Vec<NestedDemand>) -> NestedDemands {
        NestedDemands { known: demands, wanted: Vec::new() }
    }

    /// The demands of `module`, in the order they were found.
    fn of(&self, module: DeclId) -> Vec<NestedDemand> {
        self.known.iter().filter(|demand| demand.module == module).cloned().collect()
    }

    /// Records `demand` as wanted unless the run knows it: the run is made again with it.
    pub(in crate::elab) fn require(&mut self, demand: NestedDemand) {
        if !self.known.contains(&demand) && !self.wanted.contains(&demand) {
            self.wanted.push(demand);
        }
    }

    /// The demands the run found missing.
    pub(in crate::elab) fn into_wanted(self) -> Vec<NestedDemand> {
        self.wanted
    }
}

/// What a reached slot stands for in the body that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::elab) enum ReachedKind {
    /// A node of the grandchild.
    Node,
    /// A carrier of the grandchild.
    Carrier,
}

/// One reached slot a body lowers for a demand: the iteration of every placement the path passes through, the kind of the item,
/// and the frame and item it is made for.
#[derive(Debug, Clone)]
pub(in crate::elab) struct DemandEntry {
    /// The demand the slot answers.
    pub(in crate::elab) demand: NestedDemand,
    /// The iteration of each placement on the way, outermost first, one ordinal for each generation `for` around it.
    pub(in crate::elab) ordinals: Vec<u64>,
    /// Whether the item is a node or a carrier.
    pub(in crate::elab) kind: ReachedKind,
    /// The reach frame of the module that declares the item.
    pub(in crate::elab) frame: usize,
    /// The item.
    pub(in crate::elab) item: DeclId,
}

impl Elaboration<'_, '_> {
    /// Whether the `let` binding `binding` of a module's body is a placement.
    pub(in crate::elab) fn is_placement_binding(&self, binding: DeclId) -> bool {
        let Some(value) = self.body_let_value(binding) else { return false };
        matches!(&self.design.expr_node(self.peel_parens(value)).kind, ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. })
    }

    /// The item named `name` of `module` (a port, a signal, a param or a `let` name, entering generation).
    pub(in crate::elab) fn item_named(&self, module: DeclId, name: &str) -> Option<DeclId> {
        let mut pending = vec![module];
        while let Some(at) = pending.pop() {
            for child in &self.design.decl_data(at).children {
                let data = self.design.decl_data(*child);
                let named = data.name.as_ref().is_some_and(|n| n.text == name);
                match &data.kind {
                    DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) if named => return Some(*child),
                    DeclKind::Let(_) => {
                        if let Some(binding) = data.children.iter().find(|b| self.design.decl_data(**b).name.as_ref().is_some_and(|n| n.text == name)) {
                            return Some(*binding);
                        }
                    }
                    DeclKind::Generation(_) => pending.push(*child),
                    _ => {}
                }
            }
        }
        None
    }

    /// Whether the code of a parent may name `item` of a child: a port always, any other item when it is `pub` (§8.2.8).
    fn is_visible_item(&self, item: DeclId) -> bool {
        let data = self.design.decl_data(item);
        match &data.kind {
            DeclKind::Port(_) => true,
            DeclKind::Binding(_) => data.parent.is_some_and(|p| self.design.decl_data(p).public),
            _ => data.public,
        }
    }

    /// The item `item` of `concrete` as a parent may read it: `E0343` after the diagnostic of an item that is missing or not `pub`.
    pub(in crate::elab) fn visible_item_of(&mut self, scope: &Scope, concrete: DeclId, item: &str, span: crate::msl::Span) -> Option<DeclId> {
        let module_name = self.design.decl_data(concrete).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(decl) = self.item_named(concrete, item) else {
            self.report(scope.file, crate::msl::Diagnostic::new("E0343", format!("no item `{item}` in module `{module_name}`"), span, "no such item"));
            return None;
        };
        if !self.is_visible_item(decl) {
            self.report(scope.file, crate::msl::Diagnostic::new("E0343", format!("`{item}` is not `pub` in module `{module_name}`"), span, "not `pub`"));
            return None;
        }
        Some(decl)
    }

    /// What the item `item` of a module is when a parent reaches it: a node, a carrier, or neither (a param, a `let`, an array).
    fn reached_kind(&self, item: DeclId) -> Option<ReachedKind> {
        if !matches!(self.design.decl_data(item).kind, DeclKind::Port(_) | DeclKind::Signal(_)) || self.design.is_bundle_owner(item) || self.types.node_array(item).is_some() {
            return None;
        }
        Some(if self.types.carrier(item).is_some() { ReachedKind::Carrier } else { ReachedKind::Node })
    }

    /// The reached slots the body of `module` lowers, in order, seen from `frame` (the body's own frame 0, or the reach frame of
    /// an instance of the module): every demand it meets, once for each iteration of the placement it names, and below it one for
    /// each slot the child's own module lowers for the rest of the path. The generation of `module` is expanded now
    /// (`self.generated`).
    pub(in crate::elab) fn demand_entries(&mut self, frame: usize, module: DeclId) -> Vec<DemandEntry> {
        let mut entries = Vec::new();
        for demand in self.nested_demands.of(module) {
            let Some(value) = self.body_let_value(demand.child) else { continue };
            let call = self.peel_parens(value);
            let keys = self.frame_bindings(frame);
            if self.placed_module(call, &keys).is_none() || self.hoisted_callee(call).is_some() {
                continue;
            }
            let owner = self.design.expr_node(call).owner;
            let iterations: Vec<Option<Iteration>> = match self.generated.of(owner) {
                Some(iterations) => iterations.iter().cloned().map(Some).collect(),
                None => vec![None],
            };
            let span = self.design.expr_node(call).span;
            for iteration in iterations {
                let ordinals = iteration.as_ref().map_or_else(Vec::new, |iteration| self.iteration_ordinals(owner, iteration));
                let Some(child) = self.frame_of_iteration(frame, demand.child, iteration.as_ref(), span) else { continue };
                entries.extend(self.entries_below(child, &demand, &ordinals));
            }
        }
        entries
    }

    /// The entries of `demand` at the reach frame `child` of its child, the child's iteration being `ordinals`: the item itself when
    /// the path ends, else what the child's own module lowers for the rest of the path.
    fn entries_below(&mut self, child: usize, demand: &NestedDemand, ordinals: &[u64]) -> Vec<DemandEntry> {
        let module = self.frames[child].module;
        let Some(next) = self.item_named(module, &demand.path[0]) else { return Vec::new() };
        if demand.path.len() == 1 {
            return match self.reached_kind(next) {
                Some(kind) => vec![DemandEntry { demand: demand.clone(), ordinals: ordinals.to_vec(), kind, frame: child, item: next }],
                None => Vec::new(),
            };
        }
        if !self.is_placement_binding(next) {
            return Vec::new();
        }
        let below = NestedDemand { module, child: next, path: demand.path[1..].to_vec() };
        self.nested_demands.require(below.clone());
        self.entries_under(child, module)
            .into_iter()
            .filter(|entry| entry.demand == below)
            .map(|entry| DemandEntry { demand: demand.clone(), ordinals: [ordinals, entry.ordinals.as_slice()].concat(), ..entry })
            .collect()
    }

    /// The entries the body of `module` lowers in the variant of `frame`: its generation expanded under the frame's keys and
    /// generic arguments.
    fn entries_under(&mut self, frame: usize, module: DeclId) -> Vec<DemandEntry> {
        self.with_generation_of(frame, module, |this, _| this.demand_entries(frame, module))
    }

    /// The reach frame of the placement bound by `binding` in the module of `parent`, for the iteration `iteration` of its generation
    /// (`None` outside generation).
    pub(in crate::elab) fn frame_of_iteration(&mut self, parent: usize, binding: DeclId, iteration: Option<&Iteration>, span: crate::msl::Span) -> Option<usize> {
        let call = self.peel_parens(self.body_let_value(binding)?);
        let file = self.design.decl_data(self.design.expr_node(call).owner).file;
        let mut within = Scope::new(file);
        within.frame = parent;
        within.keys = self.frame_bindings(parent);
        if let Some(iteration) = iteration {
            for (variable, value) in &iteration.values {
                within.unrolled.bind(*variable, Constant::Int(*value));
            }
            within.ordinals = iteration.ordinals.clone();
        }
        let key = self.placement_key(&within, call);
        self.placement_frame(&within, binding, key, span)
    }

    /// Gives the body of `module`, being lowered, the reached node of every demand its variant meets, right after its own nodes.
    pub(in crate::elab) fn lower_nested_demands(&mut self, scope: &Scope, module: DeclId) {
        for entry in self.demand_entries(0, module).into_iter().filter(|entry| entry.kind == ReachedKind::Node) {
            let slot = self.structure.nodes.len() as u32;
            let made = self.frame_node(entry.frame, entry.item);
            if made != Some(piperine_ir::Terminal::Node(slot)) {
                let span = self.design.decl_data(entry.item).span;
                self.unsupported(scope.file, span, "a node of a child read by its parent that the child's body could not place where the parent expects it");
            }
        }
    }

    /// Gives the body of `module`, being lowered, the reached carrier of every demand its variant meets, right after its own carriers.
    pub(in crate::elab) fn lower_nested_carrier_demands(&mut self, scope: &Scope, module: DeclId) {
        for entry in self.demand_entries(0, module).into_iter().filter(|entry| entry.kind == ReachedKind::Carrier) {
            let slot = self.structure.carriers.len() as u32;
            let made = self.frame_carrier(entry.frame, entry.item);
            if made != Some(slot) {
                let span = self.design.decl_data(entry.item).span;
                self.unsupported(scope.file, span, "a carrier of a child read by its parent that the child's body could not place where the parent expects it");
            }
        }
    }

    /// The reach frame at the root of the chain `frame` hangs from (the frame of a child of the body, which has a body of its own), and
    /// the placements the chain passes through below it, outermost first, each with the ordinals of its iteration. `None` for a frame
    /// that is no reach frame.
    fn reach_chain(&self, frame: usize) -> Option<(usize, Vec<(DeclId, Vec<u64>)>)> {
        let mut steps = Vec::new();
        let mut at = frame;
        while self.frames[at].child.is_none() {
            steps.push(self.frames[at].placement.clone()?);
            at = self.frames[at].parent?;
        }
        steps.reverse();
        Some((at, steps))
    }

    /// The index the reached slot of the item `item` of the reach frame `frame` takes in the body of the child that roots its chain,
    /// and that child's index among the body's children: the slots of its own, plus the entries of its module that precede it. The
    /// demand is asked for when the run does not know it. `None` for a frame of a child of the body, whose item is the child's own,
    /// and for a slot the child's module does not lower.
    pub(in crate::elab) fn nested_slot(&mut self, frame: usize, item: DeclId, kind: ReachedKind) -> Option<(u32, u32)> {
        let (root, steps) = self.reach_chain(frame)?;
        let ((first, _), rest) = steps.split_first()?;
        let module = self.frames[root].module;
        let mut path: Vec<String> = rest.iter().map(|(binding, _)| self.design.decl_data(*binding).name.as_ref().map_or_else(String::new, |n| n.text.clone())).collect();
        path.push(self.design.decl_data(item).name.as_ref().map_or_else(String::new, |n| n.text.clone()));
        let ordinals: Vec<u64> = steps.iter().flat_map(|(_, ordinals)| ordinals.iter().copied()).collect();
        let demand = NestedDemand { module, child: *first, path };
        self.nested_demands.require(demand.clone());
        let entries = self.entries_under(root, module);
        let position = entries.iter().filter(|entry| entry.kind == kind).position(|entry| entry.demand == demand && entry.ordinals == ordinals)? as u32;
        let own = match kind {
            ReachedKind::Node => self.own_node_total(root)?,
            ReachedKind::Carrier => self.own_carrier_total(root)?,
        };
        Some((self.frames[root].child?, own + position))
    }

    /// The iterations of the generation around the placement owned by `owner` in the module of `frame`, expanded under the frame's
    /// keys and generic arguments.
    pub(in crate::elab) fn iterations_under(&mut self, frame: usize, owner: DeclId) -> Vec<Iteration> {
        let module = self.frames[frame].module;
        self.with_generation_of(frame, module, |this, _| this.generated.of(owner).cloned().unwrap_or_default())
    }

    /// The ordinals of `iteration` in the generation `for`s around the placement owned by `owner`, outermost first.
    pub(in crate::elab) fn iteration_ordinals(&self, owner: DeclId, iteration: &Iteration) -> Vec<u64> {
        self.generation_loops(owner).iter().filter_map(|generation| iteration.ordinals.iter().find(|(held, _)| held == generation).map(|(_, ordinal)| u64::from(*ordinal))).collect()
    }

    /// The ordinals the index expressions `indexes` fold to in the scope: `None` after a diagnostic.
    pub(in crate::elab) fn fold_ordinals(&mut self, scope: &Scope, indexes: &[ExprId]) -> Option<Vec<u64>> {
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        let mut ordinals = Vec::new();
        for index in indexes {
            match self.consts.value(*index, &bindings) {
                Ok(Constant::Int(ordinal)) if ordinal >= 0 => ordinals.push(ordinal as u64),
                _ => {
                    self.not_folded(scope, self.design.expr_node(*index).span, "the index of a generated instance");
                    return None;
                }
            }
        }
        Some(ordinals)
    }

    /// The outermost level of `ordinals` that no row of `held` (the ordinals the iterations of a generation make) reaches under the
    /// levels outside it, as its ordinal and the number of ordinals that level does have there.
    pub(in crate::elab) fn outermost_missing_ordinal(held: &[Vec<u64>], ordinals: &[u64]) -> (u64, u64) {
        for level in 0..ordinals.len() {
            let mut reached: Vec<u64> = held.iter().filter(|each| each[..level] == ordinals[..level]).map(|each| each[level]).collect();
            reached.sort_unstable();
            reached.dedup();
            if !reached.contains(&ordinals[level]) {
                return (ordinals[level], reached.len() as u64);
            }
        }
        (ordinals.last().copied().unwrap_or(0), held.len() as u64)
    }

    /// `read` of the generation of `module` expanded under the keys and generic arguments of `frame`, the state of the body being
    /// lowered put back after.
    fn with_generation_of<T>(&mut self, frame: usize, module: DeclId, read: impl FnOnce(&mut Self, &Bindings) -> T) -> T {
        let keys = self.frame_bindings(frame);
        let outer_generated = std::mem::take(&mut self.generated);
        let outer_keys = std::mem::replace(&mut self.body_keys, keys.clone());
        let outer_params = self.consts.set_frame_params(keys.clone());
        let outer_substitution = self.consts.set_substitution(self.frames[frame].substitution.clone());
        self.expand_generation(module);
        let found = read(self, &keys);
        self.consts.set_substitution(outer_substitution);
        self.consts.set_frame_params(outer_params);
        self.body_keys = outer_keys;
        self.generated = outer_generated;
        found
    }
}
