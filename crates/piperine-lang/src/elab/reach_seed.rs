//! The demands of a design that its text names (`reach_through_templates.rs`), found before the first run so that a read through
//! children of children costs one pass, not one for each name of the path.
//!
//! A read `p.h1.h2.x` names a placement `p` of the reader, then one name for each hop and the item `x`. Each hop whose name the
//! checker resolved to a placement `let` (`Resolution::Decl`) is a demand of the module that holds that `let`: the names after it.
//! When `x` is a `let` of the module the last hop places, the names that `let` reads through its own children are read through
//! the same hops (§8.2.25: it is evaluated in the reader's context), so the chain goes on with them. A hop the text does not name a
//! module for (a read through a type-parameter template) is left to the run, which asks for what it meets
//! (`NestedDemands::require`).

use std::collections::{BTreeMap, BTreeSet};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::reach_through_templates::NestedDemand;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

/// How many `let`s a chain is followed through: a chain longer than any design writes stops there.
const CLOSURE_DEPTH: usize = 16;

/// One name of a read below a placement: the `let` of a placement the checker resolved it to (none for the item itself, or a name it
/// did not resolve) and the name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ReadName {
    /// The placement `let` the name is, when the checker resolved it to one.
    placement: Option<DeclId>,
    /// The name as written.
    text: String,
}

/// A read of the design: a field chain rooted at a name bound to a placement.
#[derive(Debug, Clone)]
struct ReadChain {
    /// The declaration that owns the expression (the `let` whose value holds it).
    owner: DeclId,
    /// The `let` of the placement the chain starts at.
    root: DeclId,
    /// The names below the root, the item last.
    names: Vec<ReadName>,
}

impl Elaboration<'_, '_> {
    /// The demands the text of the design names, in the order of its reads, without duplicates.
    pub(super) fn static_demands(&self) -> Vec<NestedDemand> {
        let chains = self.outermost_chains();
        let mut by_owner: BTreeMap<DeclId, Vec<&ReadChain>> = BTreeMap::new();
        for chain in &chains {
            by_owner.entry(chain.owner).or_default().push(chain);
        }
        let mut demands = Vec::new();
        let mut seen = BTreeSet::new();
        for chain in &chains {
            // a read of the reader's own child (`c0.sn`, one name) is no demand; it is the chain of a `let` below a hop that is
            let Some(base) = self.placed_by_root(chain.root) else { continue };
            self.add_chain(base, chain.names.clone(), &by_owner, &mut demands, &mut seen, 0);
        }
        demands
    }

    /// Every typed read of the design that is not the receiver of another.
    fn outermost_chains(&self) -> Vec<ReadChain> {
        let count = self.design.expr_count();
        let mut receivers = BTreeSet::new();
        for index in 0..count {
            if let ExprKind::Field { expr, .. } = &self.design.expr_node(ExprId(index as u32)).kind {
                receivers.insert(self.peel_receiver(*expr));
            }
        }
        (0..count).map(|index| ExprId(index as u32)).filter(|expr| !receivers.contains(expr) && self.typed(*expr).is_some()).filter_map(|expr| self.chain_of(expr)).collect()
    }

    /// `expr` through parentheses and indexes.
    fn peel_receiver(&self, mut expr: ExprId) -> ExprId {
        loop {
            match &self.design.expr_node(expr).kind {
                ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => expr = *inner,
                _ => return expr,
            }
        }
    }

    /// The read `expr` is, when it is a field chain rooted at a name bound to a placement.
    fn chain_of(&self, expr: ExprId) -> Option<ReadChain> {
        let mut names = Vec::new();
        let mut at = expr;
        while let ExprKind::Field { expr: receiver, name, res } = &self.design.expr_node(at).kind {
            let placement = match res {
                Resolution::Decl(binding) if self.is_placement_binding(*binding) => Some(*binding),
                _ => None,
            };
            names.push(ReadName { placement, text: name.text.clone() });
            at = self.peel_receiver(*receiver);
        }
        names.reverse();
        let ExprKind::Path(path) = &self.design.expr_node(at).kind else { return None };
        let Some(Resolution::Decl(root)) = self.design.path_node(*path).segments.last().map(|segment| segment.res) else { return None };
        if names.is_empty() || !self.is_placement_binding(root) {
            return None;
        }
        Some(ReadChain { owner: self.design.expr_node(expr).owner, root, names })
    }

    /// The module the placement `binding` places, when the text names it.
    fn placed_by_root(&self, binding: DeclId) -> Option<DeclId> {
        let call = self.peel_parens(self.body_let_value(binding)?);
        self.placed_module(call, &Bindings::new())
    }

    /// The demands of the read of `names` below a placement of `base`, and of the reads the `let` it ends on makes through its own
    /// children, below the same hops.
    fn add_chain(&self, base: DeclId, names: Vec<ReadName>, by_owner: &BTreeMap<DeclId, Vec<&ReadChain>>, demands: &mut Vec<NestedDemand>, seen: &mut BTreeSet<Vec<ReadName>>, depth: usize) {
        if depth > CLOSURE_DEPTH || !seen.insert(names.clone()) {
            return;
        }
        for (at, name) in names.iter().enumerate().take(names.len() - 1) {
            let Some(child) = name.placement else { continue };
            let Some(module) = self.module_holding(child) else { continue };
            let demand = NestedDemand { module, child, path: names[at + 1..].iter().map(|name| name.text.clone()).collect() };
            if !demands.contains(&demand) {
                demands.push(demand);
            }
        }
        let Some(holder) = self.holder_of_last(base, &names) else { return };
        let Some(item) = self.item_named(holder, &names[names.len() - 1].text) else { return };
        let data = self.design.decl_data(item);
        if self.is_placement_binding(item) || self.body_let_value(item).is_none() {
            return;
        }
        let Some(let_decl) = data.parent else { return };
        for inner in by_owner.get(&let_decl).into_iter().flatten() {
            let root = ReadName { placement: Some(inner.root), text: self.design.decl_data(inner.root).name.as_ref().map_or_else(String::new, |n| n.text.clone()) };
            let mut below: Vec<ReadName> = names[..names.len() - 1].to_vec();
            below.push(root);
            below.extend(inner.names.iter().cloned());
            self.add_chain(base, below, by_owner, demands, seen, depth + 1);
        }
    }

    /// The module that holds the item `names` ends on: the one the last hop places, or `base` when the chain has one name.
    fn holder_of_last(&self, base: DeclId, names: &[ReadName]) -> Option<DeclId> {
        let mut holder = base;
        for name in &names[..names.len() - 1] {
            holder = self.placed_by_root(name.placement?)?;
        }
        Some(holder)
    }

    /// The module whose body holds the `let` binding `binding`.
    fn module_holding(&self, binding: DeclId) -> Option<DeclId> {
        let mut parent = self.design.decl_data(binding).parent;
        while let Some(decl) = parent {
            if matches!(self.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some(decl);
            }
            parent = self.design.decl_data(decl).parent;
        }
        None
    }
}
