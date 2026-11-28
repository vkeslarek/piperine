//! The params an evaluation may read: [`Bindings`].

use std::collections::BTreeMap;

use piperine_ir::Constant;

use super::ConstTree;
use crate::pom::DeclId;

/// The bound value of each param declaration an evaluation may read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings {
    pub(super) values: BTreeMap<DeclId, Constant>,
    /// The value of each composite param, taken apart: a `Constant::Aggregate` is an entry of the
    /// IR's arena, which this evaluator cannot read, so it reads the tree.
    pub(super) trees: BTreeMap<DeclId, ConstTree>,
}

impl Bindings {
    /// No param bound: the bindings of a `const` item.
    pub fn new() -> Self {
        Bindings::default()
    }

    pub fn bind(&mut self, param: DeclId, value: Constant) {
        self.values.insert(param, value);
    }

    /// Binds a composite param to its constant of the IR's arena and to `tree`, the same value
    /// taken apart for the evaluator.
    pub fn bind_composite(&mut self, param: DeclId, value: Constant, tree: ConstTree) {
        self.values.insert(param, value);
        self.trees.insert(param, tree);
    }

    /// The integer value of each param bound to one, by param.
    pub fn integers(&self) -> Vec<(DeclId, i128)> {
        self.values.iter().filter_map(|(param, value)| match value {
            Constant::Int(number) => Some((*param, *number)),
            _ => None,
        }).collect()
    }

    /// The value of the composite param `param`, taken apart.
    pub fn tree(&self, param: DeclId) -> Option<&ConstTree> {
        self.trees.get(&param)
    }

    /// Whether no param is bound.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, param: DeclId) -> Option<Constant> {
        self.values.get(&param).copied()
    }

    /// The params bound to a non-negative integer, as the integer folder reads them.
    pub(in crate::elab) fn counts(&self) -> BTreeMap<DeclId, u128> {
        self.values.iter().filter_map(|(decl, value)| match value {
            Constant::Int(count) => u128::try_from(*count).ok().map(|count| (*decl, count)),
            _ => None,
        }).collect()
    }

    /// These bindings with those of `other` added: `other` wins where both bind a name.
    pub(in crate::elab) fn overlaid(&self, other: &Bindings) -> Bindings {
        let mut merged = self.clone();
        merged.values.extend(other.values.iter().map(|(decl, value)| (*decl, *value)));
        merged.trees.extend(other.trees.iter().map(|(decl, tree)| (*decl, tree.clone())));
        merged
    }

    /// Forgets the value of `decl`; the value it had, if any.
    pub(in crate::elab) fn unbind(&mut self, decl: DeclId) -> Option<Constant> {
        self.trees.remove(&decl);
        self.values.remove(&decl)
    }
}
