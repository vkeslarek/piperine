//! Aggregates in a const context (CV-36, CV-13 AC 3; part I §1.2.4, §1.2.5, §1.3, §3.2, H.5 to H.8):
//! tuples, arrays, structs, enum variants with fields, `Option`s, ranges and iterators evaluate at
//! elaboration to the value they have at run time.
//!
//! A composite is held by the evaluator as an entry of its own arena, named inside a [`Constant`] by
//! a `Constant::Aggregate` handle that means nothing outside the evaluator: the public entry points
//! never give one out (`ConstEval::folded` and its kin refuse it), and [`ConstEval::tree`] gives the
//! value as an owned [`ConstTree`] for the elaborator to turn into an IR constant. Every part is a
//! `Constant`; a value is copied where it is used (§1.4.6), so an
//! entry is never changed: an assignment builds the composite again with the one part replaced.
//!
//! An iterator holds its elements (`iter()`, the adaptors and a range used as an iterator are
//! arrays of them); an adaptor runs its closure on every element when it is called, a consumer
//! stops at the element that decides it (`any`, `all`, `contains`). An index outside an array is a
//! panic (§1 rule 11): `E0104` naming the expression. A unit variant of an enum with no field in
//! any variant stays its tag (§2 rule 5), as everywhere else in this evaluator.


use piperine_ir::{AggregateId, Constant};

use super::control::{Argument};
use super::{Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm};
use crate::pom::{DeclId, ExprId};

/// What a composite value is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeKind {
    /// A tuple: one part per component.
    Tuple,
    /// An array: one part per element.
    Array,
    /// A struct: one part per field, in declaration order.
    Struct,
    /// The variant at this position of an enum with fields: one part per field of the variant.
    Variant(u32),
    /// An `Option`: no part for `None`, one for `Some`.
    Option,
    /// A range: the bounds it has (low, then high), whether `hi` is included.
    Range {
        /// Whether the range has a low bound.
        low: bool,
        /// Whether the range has a high bound.
        high: bool,
        /// Whether the high bound is included.
        inclusive: bool,
    },
}

/// One composite the evaluator holds.
#[derive(Debug, Clone)]
pub(super) struct Composite {
    /// What it is made of.
    pub(super) kind: CompositeKind,
    /// Its parts.
    pub(super) parts: Vec<Constant>,
}

/// A folded value with its composites taken apart, for the elaborator to turn into IR constants.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstTree {
    /// A scalar.
    Scalar(Constant),
    /// A composite and its parts.
    Composite {
        /// What it is made of.
        kind: CompositeKind,
        /// Its parts, each a tree.
        parts: Vec<ConstTree>,
    },
}

impl ConstEval<'_, '_> {
    /// Adds the composite of `kind` and `parts`; the `Constant` that names it.
    pub(super) fn make(&mut self, kind: CompositeKind, parts: Vec<Constant>) -> Constant {
        self.composites.push(Composite { kind, parts });
        Constant::Aggregate(AggregateId((self.composites.len() - 1) as u32))
    }

    /// The composite `value` names, when it names one.
    pub(super) fn composite_of(&self, value: Constant) -> Option<&Composite> {
        match value {
            Constant::Aggregate(id) => self.composites.get(id.index()),
            _ => None,
        }
    }

    /// The levels word the function `encode` of a hand-written `Repr` gives for the value `init` folds to: level `k`
    /// is bit `k` (GN-05). The word is what a carrier of such a type is initialized with.
    pub(in crate::elab) fn encoded_word(&mut self, init: ExprId, encode: DeclId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.fold(init, bindings)?;
        self.levels_word(init, encode, value)
    }

    /// [`ConstEval::encoded_word`] for a value given as a `tree` (the `Default` of a type, which no expression writes);
    /// `at` is the expression a fault is reported at.
    pub(in crate::elab) fn encoded_tree_word(&mut self, at: ExprId, encode: DeclId, tree: &ConstTree) -> Result<Constant, ConstFault> {
        let value = self.adopt(tree);
        self.levels_word(at, encode, value)
    }

    /// The levels word the function `encode` gives for `value`, `at` the expression a fault names.
    pub(super) fn levels_word(&mut self, init: ExprId, encode: DeclId, value: Constant) -> Result<Constant, ConstFault> {
        let levels = self.apply_function(init, encode, vec![Argument { value: Some(value), function: None }])?;
        let (_, parts) = self.parts_of(init, levels, "the levels of an `encode`")?;
        let mut word: i128 = 0;
        for (level, part) in parts.iter().enumerate() {
            let Constant::Bool(high) = part else { return Err(ConstFault::refused(init, ConstantForm::EncodeThatGivesLevelThatIsNot)) };
            word |= i128::from(*high) << level;
        }
        Ok(Constant::Int(word))
    }

    /// The composite `value` names and its parts copied out, or a fault naming `expr`.
    pub(super) fn parts_of(&self, expr: ExprId, value: Constant, what: &str) -> Result<(CompositeKind, Vec<Constant>), ConstFault> {
        match self.composite_of(value) {
            Some(composite) => Ok((composite.kind, composite.parts.clone())),
            None => Err(ConstFault::refused_with(expr, ConstantForm::ThatIsNotCompositeValue, [what.to_string()])),
        }
    }

    /// The value `tree` describes, its composites added to this evaluator's own.
    pub(super) fn adopt(&mut self, tree: &ConstTree) -> Constant {
        match tree {
            ConstTree::Scalar(value) => *value,
            ConstTree::Composite { kind, parts } => {
                let parts = parts.iter().map(|part| self.adopt(part)).collect();
                self.make(*kind, parts)
            }
        }
    }

    /// `value` as a tree: its composites taken apart down to the scalars.
    pub(super) fn export(&self, value: Constant) -> ConstTree {
        match self.composite_of(value) {
            Some(composite) => ConstTree::Composite { kind: composite.kind, parts: composite.parts.iter().map(|part| self.export(*part)).collect() },
            None => ConstTree::Scalar(value),
        }
    }

    /// The tree of the value of `expr` over `bindings`.
    pub fn tree(&mut self, expr: ExprId, bindings: &Bindings) -> Result<ConstTree, ConstFault> {
        let value = self.fold(expr, bindings)?;
        Ok(self.export(value))
    }

    /// The tree of the value of the `const` item `decl`.
    pub fn constant_tree(&mut self, decl: DeclId) -> Result<ConstTree, ConstFault> {
        let value = self.fold_constant(decl)?;
        Ok(self.export(value))
    }

    /// Whether the `const` item `decl` folds to a composite value.
    pub fn is_composite_constant(&mut self, decl: DeclId) -> bool {
        self.fold_constant(decl).is_ok_and(|value| self.composite_of(value).is_some())
    }

    // ---- building ----------------------------------------------------------------------------

    // ---- reading -----------------------------------------------------------------------------

    // ---- comparison --------------------------------------------------------------------------

    // ---- assignment --------------------------------------------------------------------------

    // ---- patterns ----------------------------------------------------------------------------

    // ---- sequences and their rows ------------------------------------------------------------

}
