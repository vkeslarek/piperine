//! The demands of a design that its text names (`reach/demands.rs`), found before the first run so that a read through
//! children of children costs one pass, not one for each name of the path.
//!
//! A read `p.h1.h2.x` names a placement `p` of the reader, then one name for each hop and the item `x`. Each hop whose name the
//! checker resolved to a placement `let` (`Resolution::Decl`) is a demand of the module that holds that `let`: the names after it.
//! When `x` is a `let` of the module the last hop places, what that `let` reads is read through the same hops (§8.2.25: it is
//! evaluated in the reader's context), so the chain goes on with each of its reads: a field chain through its own children
//! (`c1.sn`), and a plain name of an item of its own module (`v(sn)` reads the node `sn`, `twice + 1.0` the `let` `twice`, whose
//! reads go on in turn). Every read of a module item in an expression, whatever the form around it (a call, an operator, a method,
//! an index, a block), is a path expression of the arena, so these two kinds are every read a `let` makes. A hop the text does not
//! name a module for (a read through a type-parameter template) is left to the run, which asks for what it meets
//! (`NestedDemands::require`).

use std::collections::{BTreeMap, BTreeSet};

use crate::elab::consts::Bindings;
use crate::elab::elaborator::Elaboration;
use super::demands::NestedDemand;
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

/// What the expressions of the design read, by the declaration that owns them (the `let` whose value holds them).
#[derive(Debug, Default)]
struct DesignReads<'c> {
    /// The field chains rooted at a placement.
    chains: BTreeMap<DeclId, Vec<&'c ReadChain>>,
    /// The declarations a plain name resolves to.
    names: BTreeMap<DeclId, BTreeSet<DeclId>>,
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
    pub(in crate::elab) fn static_demands(&self) -> Vec<NestedDemand> {
        let chains = self.outermost_chains();
        let mut reads = DesignReads { names: self.plain_reads(), ..DesignReads::default() };
        for chain in &chains {
            reads.chains.entry(chain.owner).or_default().push(chain);
        }
        let mut demands = Vec::new();
        let mut seen = BTreeSet::new();
        for chain in &chains {
            // a read of the reader's own child (`c0.sn`, one name) is no demand; it is the chain of a `let` below a hop that is
            let Some(base) = self.placed_by_root(chain.root) else { continue };
            self.add_chain(base, chain.names.clone(), &reads, &mut demands, &mut seen, 0);
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

    /// The declaration each plain name of the design resolves to, by the declaration that owns the name's expression.
    fn plain_reads(&self) -> BTreeMap<DeclId, BTreeSet<DeclId>> {
        let mut reads: BTreeMap<DeclId, BTreeSet<DeclId>> = BTreeMap::new();
        for index in 0..self.design.expr_count() {
            let node = self.design.expr_node(ExprId(index as u32));
            let ExprKind::Path(path) = &node.kind else { continue };
            if let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                reads.entry(node.owner).or_default().insert(decl);
            }
        }
        reads
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

    /// The demands of the read of `names` below a placement of `base`, and of the reads the `let` it ends on makes, below the same
    /// hops: through its own children, and of the items of its own module it names.
    fn add_chain(&self, base: DeclId, names: Vec<ReadName>, reads: &DesignReads, demands: &mut Vec<NestedDemand>, seen: &mut BTreeSet<Vec<ReadName>>, depth: usize) {
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
        let hops = &names[..names.len() - 1];
        for inner in reads.chains.get(&let_decl).into_iter().flatten() {
            let mut below: Vec<ReadName> = hops.to_vec();
            below.push(ReadName { placement: Some(inner.root), text: self.decl_name(inner.root) });
            below.extend(inner.names.iter().cloned());
            self.add_chain(base, below, reads, demands, seen, depth + 1);
        }
        // a name of the holder's own node, carrier or `let` (`v(sn)`): read through the same hops; a placement is read only through a
        // field chain, met above, and a param is folded with the frame, no demand
        for read in reads.names.get(&let_decl).into_iter().flatten() {
            let text = self.decl_name(*read);
            let own = self.item_named(holder, &text) == Some(*read);
            let item = matches!(self.design.decl_data(*read).kind, DeclKind::Port(_) | DeclKind::Signal(_)) || (self.body_let_value(*read).is_some() && !self.is_placement_binding(*read));
            if own && item {
                let mut below: Vec<ReadName> = hops.to_vec();
                below.push(ReadName { placement: None, text });
                self.add_chain(base, below, reads, demands, seen, depth + 1);
            }
        }
    }

    /// The name `decl` is declared with.
    fn decl_name(&self, decl: DeclId) -> String {
        self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone())
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
