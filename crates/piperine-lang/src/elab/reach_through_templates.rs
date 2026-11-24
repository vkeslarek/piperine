//! `inst.name` through a child of a child (§8.2.8, §8.4.8): `bank.c0.sn`, where `c0` is a `pub let` of `bank`'s module and its
//! placement goes through a template-typed param (`cell: C`, `C: Fn(..)`), so the module that owns `sn` is only known once the
//! template is folded. A body reads the items of its own children, never of a grandchild's, so the child's body carries a reached
//! node for the item (`NodeRole::Reached`, bound to the grandchild's own when it is placed) and the body that reads it reaches that node.
//!
//! * A **demand** is one `child.item` read through a child (`bank.c0.sn` demands `c0.sn` of the module of `bank`): they are
//!   found once for the design, in source order, and each body of the module that is the child lowers a reached node for every
//!   demand its variant can meet (one per iteration of a child placed in generation), right after its own nodes
//!   (`lower_nested_demands`), so the node's index is known to the reader (`DemandEntry`).
//! * The reader resolves the item against the concrete module (`E0343` for a missing or non-`pub` one), makes its own reached node
//!   of the child's node, and binds it to the child's (`nested_terminal`).

use std::collections::BTreeMap;

use piperine_ir::{BodyNode, Constant, NodeRole, Reach, ReachedItem, Terminal};

use super::body::NodePlan;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::generation::Iteration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

/// One item `item` of the child `child` (a `let` binding of a module's body) that another body reads as `owner.child.item`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NestedDemand {
    /// The `let` binding of the placement in the module's body.
    pub(super) child: DeclId,
    /// The name of the item the read names.
    pub(super) item: String,
}

/// One reached node a body lowers for a demand: the placement it is made for and the iteration of that placement.
#[derive(Debug, Clone)]
pub(super) struct DemandEntry {
    /// The demand the node answers.
    pub(super) demand: NestedDemand,
    /// The placement call of the child.
    pub(super) call: ExprId,
    /// The iteration of the generation the placement is written in, `None` outside generation.
    pub(super) iteration: Option<Iteration>,
}

/// One read of a demanded item that [`Elaboration::add_nested_node`] answers with a reached node of the body: where the child is
/// placed, which demand it is, the iteration the read names and the item it resolved to.
struct NestedRead<'a> {
    /// The frame of the child (`bank`) the body places.
    frame: usize,
    /// The module whose body holds the child the demand names (`bank`'s module).
    module: DeclId,
    /// The demanded item of the grandchild.
    demand: &'a NestedDemand,
    /// The iteration the read names, one ordinal per generation loop around the grandchild's placement.
    ordinals: &'a [u64],
    /// The item of the concrete module the read resolved to.
    decl: DeclId,
    /// Where the read is written.
    span: crate::msl::Span,
}

/// The demands of the design, by the module whose body holds the child.
#[derive(Debug, Default)]
pub(super) struct NestedDemands {
    found: Option<BTreeMap<DeclId, Vec<NestedDemand>>>,
}

impl NestedDemands {
    /// The demands the design makes of `module`, in source order.
    fn of(&self, module: DeclId) -> &[NestedDemand] {
        self.found.as_ref().and_then(|all| all.get(&module)).map_or(&[], Vec::as_slice)
    }
}

impl Elaboration<'_, '_> {
    /// Finds the demands of the design the first time a body is lowered.
    pub(super) fn find_demands_once(&mut self) {
        if self.nested_demands.found.is_none() {
            let found = self.find_nested_demands();
            self.nested_demands.found = Some(found);
        }
    }

    /// Every read `a.b.x` of the design where `b` is a `let` of the module `a` names: a demand of that module.
    fn find_nested_demands(&self) -> BTreeMap<DeclId, Vec<NestedDemand>> {
        let mut found: BTreeMap<DeclId, Vec<NestedDemand>> = BTreeMap::new();
        for index in 0..self.design.expr_count() {
            let expr = ExprId(index as u32);
            if self.typed(expr).is_none() {
                continue;
            }
            let ExprKind::Field { expr: receiver, name, .. } = &self.design.expr_node(expr).kind else { continue };
            let Some((module, child)) = self.child_of_module(*receiver) else { continue };
            let demand = NestedDemand { child, item: name.text.clone() };
            let list = found.entry(module).or_default();
            if !list.contains(&demand) {
                list.push(demand);
            }
        }
        found
    }

    /// The `let` binding and the module whose body holds it, when `receiver` (through parentheses and indexes) is `owner.child` with
    /// `child` the name of a placement `let`.
    fn child_of_module(&self, receiver: ExprId) -> Option<(DeclId, DeclId)> {
        let mut at = receiver;
        loop {
            match &self.design.expr_node(at).kind {
                ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => at = *inner,
                _ => break,
            }
        }
        let ExprKind::Field { res: Resolution::Decl(child), .. } = &self.design.expr_node(at).kind else { return None };
        let value = self.body_let_value(*child)?;
        let ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. } = &self.design.expr_node(self.peel_parens(value)).kind else { return None };
        let mut parent = self.design.decl_data(*child).parent;
        while let Some(decl) = parent {
            if matches!(self.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some((decl, *child));
            }
            parent = self.design.decl_data(decl).parent;
        }
        None
    }

    /// The item named `name` of `module` (a port, a signal, a param or a `let` name, entering generation).
    fn item_named(&self, module: DeclId, name: &str) -> Option<DeclId> {
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

    /// The node item of the concrete module of `demand`'s child under the key params `keys`: the child's placement resolved, its
    /// module's item of that name when it is a visible port or signal that is a node. `None` for a demand this variant cannot meet.
    fn met_demand(&self, keys: &Bindings, demand: &NestedDemand) -> Option<DeclId> {
        let module = self.concrete_module_of(keys, demand)?;
        let item = self.item_named(module, &demand.item)?;
        let is_node = matches!(self.design.decl_data(item).kind, DeclKind::Port(_) | DeclKind::Signal(_)) && self.types.carrier(item).is_none();
        (is_node && self.is_visible_item(item)).then_some(item)
    }

    /// The module the placement bound to `demand.child` places under the key params `keys`.
    fn concrete_module_of(&self, keys: &Bindings, demand: &NestedDemand) -> Option<DeclId> {
        let value = self.body_let_value(demand.child)?;
        self.placed_module(self.peel_parens(value), keys)
    }

    /// The reached nodes the body of `module` lowers, in order, under the key params `keys` and the generation expanded now
    /// (`self.generated`): every demand it meets, once for each iteration of the placement it names.
    fn demand_entries(&self, module: DeclId, keys: &Bindings) -> Vec<DemandEntry> {
        let mut entries = Vec::new();
        for demand in self.nested_demands.of(module) {
            if self.met_demand(keys, demand).is_none() {
                continue;
            }
            let Some(value) = self.body_let_value(demand.child) else { continue };
            let call = self.peel_parens(value);
            let owner = self.design.expr_node(call).owner;
            let iterations: Vec<Option<Iteration>> = match self.generated.of(owner) {
                Some(iterations) => iterations.iter().cloned().map(Some).collect(),
                None => vec![None],
            };
            entries.extend(iterations.into_iter().map(|iteration| DemandEntry { demand: demand.clone(), call, iteration }));
        }
        entries
    }

    /// The ordinals of `iteration` in the generation `for`s around the placement `call`, outermost first.
    fn entry_ordinals(&self, entry: &DemandEntry) -> Vec<u64> {
        let owner = self.design.expr_node(entry.call).owner;
        let Some(iteration) = &entry.iteration else { return Vec::new() };
        self.iteration_ordinals(owner, iteration)
    }

    /// The ordinals of `iteration` in the generation `for`s around the placement owned by `owner`, outermost first.
    pub(super) fn iteration_ordinals(&self, owner: DeclId, iteration: &Iteration) -> Vec<u64> {
        self.generation_loops(owner).iter().filter_map(|generation| iteration.ordinals.iter().find(|(held, _)| held == generation).map(|(_, ordinal)| u64::from(*ordinal))).collect()
    }

    /// Gives the body of `module`, being lowered, the reached node of every demand its variant meets, right after its own nodes.
    pub(super) fn lower_nested_demands(&mut self, scope: &mut Scope, module: DeclId) {
        let keys = self.body_keys.clone();
        let mut slot = self.structure.nodes.len() as u32;
        for entry in self.demand_entries(module, &keys) {
            let Some(item) = self.met_demand(&keys, &entry.demand) else { continue };
            let span = self.design.expr_node(entry.call).span;
            let iteration = entry.iteration.clone().unwrap_or_default();
            for (variable, value) in &iteration.values {
                scope.unrolled.bind(*variable, Constant::Int(*value));
            }
            scope.ordinals = iteration.ordinals.clone();
            let key = self.placement_key(scope, entry.call);
            let made = self.placement_frame(scope, entry.demand.child, key, span).and_then(|frame| self.frame_node(frame, item));
            scope.ordinals.clear();
            for (variable, _) in &iteration.values {
                scope.unrolled.unbind(*variable);
            }
            if made != Some(Terminal::Node(slot)) {
                self.unsupported(scope.file, span, "a node of a child read by its parent that the child's body could not place where the parent expects it");
            }
            slot += 1;
        }
    }

    /// The node `bank.c0.sn` or `bank.cells[r][c].sn` names, as a reached node of the body: the node `bank`'s body reaches into
    /// its child, itself reached from `bank`. `None` when `expr` is not a read of an item of a child of a child; `Some(None)`
    /// after a diagnostic.
    pub(super) fn nested_terminal(&mut self, scope: &Scope, expr: ExprId) -> Option<Option<Terminal>> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Field { expr: receiver, name, .. } = &node.kind else { return None };
        let (module, child) = self.child_of_module(*receiver)?;
        let mut owner = self.peel_parens(*receiver);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr: inner, index } = self.design.expr_node(owner).kind.clone() {
            indexes.push(index);
            owner = self.peel_parens(inner);
        }
        indexes.reverse();
        let ExprKind::Field { expr: bank, .. } = self.design.expr_node(owner).kind.clone() else { return None };
        Some(self.reach_grandchild(scope, bank, (module, child), &name.text, &indexes, node.span))
    }

    /// The reached node of the body for `item` of the child `child` of the module `module` (the pair), which the child `bank`
    /// of the body places; `indexes` pick the iteration of a child placed in generation.
    fn reach_grandchild(&mut self, scope: &Scope, bank: ExprId, (module, child): (DeclId, DeclId), item: &str, indexes: &[ExprId], span: crate::msl::Span) -> Option<Terminal> {
        let (binding, key) = self.receiver_placement(scope, bank)?;
        let frame = self.placement_frame(scope, binding, key, span)?;
        let ordinals = self.fold_ordinals(scope, indexes)?;
        let cached = (frame, child, item.to_string(), ordinals.clone());
        if let Some(terminal) = self.nested_nodes.get(&cached) {
            return Some(*terminal);
        }
        let keys = self.frame_bindings(frame);
        let demand = NestedDemand { child, item: item.to_string() };
        let Some(concrete) = self.concrete_module_of(&keys, &demand) else {
            self.unsupported(scope.file, span, "an item of a child whose module is not known");
            return None;
        };
        let decl = self.visible_item_of(scope, concrete, item, span)?;
        if self.met_demand(&keys, &demand) != Some(decl) {
            self.unsupported(scope.file, span, "an item of a child's child that is not one of its nodes");
            return None;
        }
        let terminal = self.add_nested_node(scope, &NestedRead { frame, module, demand: &demand, ordinals: &ordinals, decl, span })?;
        self.nested_nodes.insert(cached, terminal);
        Some(terminal)
    }

    /// The item `item` of `concrete` as a parent may read it: `E0343` after the diagnostic of an item that is missing or not `pub`.
    pub(super) fn visible_item_of(&mut self, scope: &Scope, concrete: DeclId, item: &str, span: crate::msl::Span) -> Option<DeclId> {
        let module_name = self.design.decl_data(concrete).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(decl) = self.item_named(concrete, item) else {
            self.report(scope.file, Diagnostic::new("E0343", format!("no item `{item}` in module `{module_name}`"), span, "no such item"));
            return None;
        };
        if !self.is_visible_item(decl) {
            self.report(scope.file, Diagnostic::new("E0343", format!("`{item}` is not `pub` in module `{module_name}`"), span, "not `pub`"));
            return None;
        }
        Some(decl)
    }

    /// The ordinals the index expressions `indexes` fold to in the scope: `None` after a diagnostic.
    fn fold_ordinals(&mut self, scope: &Scope, indexes: &[ExprId]) -> Option<Vec<u64>> {
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

    /// The node of the body that stands for the demanded node of the child of `frame`: its slot in the child's body is the
    /// first slot after the child's own nodes, plus the entries of the module that precede it. The entries are those of the
    /// module's generation expanded under the child's own keys, which the module's lowering made in the same order.
    fn add_nested_node(&mut self, scope: &Scope, read: &NestedRead) -> Option<Terminal> {
        let NestedRead { frame, module, demand, ordinals, decl, span } = *read;
        let bank = self.frames[frame].child?;
        let own = self.own_node_total(frame)?;
        let entries = self.entries_under(frame, module);
        let call = entries.iter().find(|e| e.demand == *demand).map(|e| e.call).or_else(|| self.body_let_value(demand.child).map(|v| self.peel_parens(v)))?;
        let owner = self.design.expr_node(call).owner;
        let loops = self.generation_loops(owner).len();
        if ordinals.len() != loops {
            self.unsupported(scope.file, span, "an item of a child placed in generation, read without naming its iteration: index every loop");
            return None;
        }
        let Some(position) = entries.iter().position(|e| e.demand == *demand && self.entry_ordinals(e) == ordinals) else {
            let (index, length) = self.first_out_of_range_loop(&entries, demand, ordinals);
            self.refer_outside_generation(scope.file, span, demand.child, Some((index, length)));
            return None;
        };
        let ty = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return None,
        };
        let nature = self.nature_decl_of(ty).and_then(|n| self.nature(n))?;
        let subscripts: String = ordinals.iter().map(|ordinal| format!("[{ordinal}]")).collect();
        let name = format!("{}{}{subscripts}.{}", self.frames[frame].prefix, self.design.decl_data(demand.child).name.as_ref().map_or("", |n| n.text.as_str()), demand.item);
        let origin = self.locator.decl(decl);
        let spec = BodyNode { name: name.clone(), nature, role: NodeRole::Reached, init: None, origin: origin.clone() };
        let plan = NodePlan { name, role: NodeRole::Reached, nature, init: None, origin };
        self.structure.nodes.push((spec, plan));
        let node = self.structure.nodes.len() as u32 - 1;
        self.structure.reaches.push(Reach { child: bank, item: ReachedItem::Node { node, child_node: own + position as u32 } });
        Some(Terminal::Node(node))
    }

    /// The outermost loop of the placement `demand` names whose ordinal in `ordinals` its iterations do not reach, as that ordinal and
    /// the number of ordinals the loop has under the loops outside it.
    fn first_out_of_range_loop(&self, entries: &[DemandEntry], demand: &NestedDemand, ordinals: &[u64]) -> (u64, u64) {
        let held: Vec<Vec<u64>> = entries.iter().filter(|e| e.demand == *demand).map(|e| self.entry_ordinals(e)).collect();
        Self::outermost_missing_ordinal(&held, ordinals)
    }

    /// The outermost level of `ordinals` that no row of `held` (the ordinals the iterations of a generation make) reaches under the
    /// levels outside it, as its ordinal and the number of ordinals that level does have there.
    pub(super) fn outermost_missing_ordinal(held: &[Vec<u64>], ordinals: &[u64]) -> (u64, u64) {
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

    /// The entries the body of `module` lowers in the variant of `frame`: its generation expanded under the frame's keys and
    /// generic arguments, the state of the body being lowered put back after.
    fn entries_under(&mut self, frame: usize, module: DeclId) -> Vec<DemandEntry> {
        let keys = self.frame_bindings(frame);
        let outer_generated = std::mem::take(&mut self.generated);
        let outer_keys = std::mem::replace(&mut self.body_keys, keys.clone());
        let outer_params = self.consts.set_frame_params(keys.clone());
        let outer_substitution = self.consts.set_substitution(self.frames[frame].substitution.clone());
        self.expand_generation(module);
        let entries = self.demand_entries(module, &keys);
        self.consts.set_substitution(outer_substitution);
        self.consts.set_frame_params(outer_params);
        self.body_keys = outer_keys;
        self.generated = outer_generated;
        entries
    }
}
