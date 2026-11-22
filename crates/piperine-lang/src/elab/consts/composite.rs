//! Aggregates in a const context (CV-36, CV-13 AC 3; part I §1.2.4, §1.2.5, §1.3, §3.2, H.5 to H.8):
//! tuples, arrays, structs, enum variants with fields, `Option`s, ranges and iterators evaluate at
//! elaboration to the value they have at run time.
//!
//! A composite is held by the evaluator as an entry of its own arena, named inside a [`Folded`] by
//! a `Constant::Aggregate` handle that means nothing outside the evaluator: the public entry points
//! never give one out (`ConstEval::folded` and its kin refuse it), and [`ConstEval::tree`] gives the
//! value as an owned [`ConstTree`] for the elaborator to turn into an IR constant. Every part is a
//! `Folded`, so a part may be unknown (§3.3); a value is copied where it is used (§1.4.6), so an
//! entry is never changed: an assignment builds the composite again with the one part replaced.
//!
//! An iterator holds its elements (`iter()`, the adaptors and a range used as an iterator are
//! arrays of them); an adaptor runs its closure on every element when it is called, a consumer
//! stops at the element that decides it (`any`, `all`, `contains`). An index outside an array is a
//! panic (§1 rule 11): `E0104` naming the expression. A unit variant of an enum with no field in
//! any variant stays its tag (§2 rule 5), as everywhere else in this evaluator.

use std::cmp::Ordering;

use piperine_ir::{AggregateId, Constant, Real};

use super::super::compound::EnumLayout;
use super::control::{Argument, FnValue};
use super::{Bindings, ConstEval, ConstFault, Folded};
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, FieldInit, PatId, PatKind, Resolution};
use crate::types::{Method, Ty};

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
    pub(super) parts: Vec<Folded>,
}

/// A folded value with its composites taken apart, for the elaborator to turn into IR constants.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstTree {
    /// A scalar, or a part that is unknown.
    Scalar(Folded),
    /// A composite and its parts.
    Composite {
        /// What it is made of.
        kind: CompositeKind,
        /// Its parts, each a tree.
        parts: Vec<ConstTree>,
    },
}

/// One step into a composite from a place: a field by position, or an element by value.
enum Step {
    /// The field at this position of a struct or tuple.
    Field(usize),
    /// The element an index expression names.
    Element(ExprId),
}

impl ConstEval<'_, '_> {
    /// Adds the composite of `kind` and `parts`; the `Folded` that names it.
    pub(super) fn make(&mut self, kind: CompositeKind, parts: Vec<Folded>) -> Folded {
        self.composites.push(Composite { kind, parts });
        Folded::Known(Constant::Aggregate(AggregateId((self.composites.len() - 1) as u32)))
    }

    /// The composite `value` names, when it names one.
    pub(super) fn composite_of(&self, value: Folded) -> Option<&Composite> {
        match value {
            Folded::Known(Constant::Aggregate(id)) => self.composites.get(id.index()),
            _ => None,
        }
    }

    /// The composite `value` names and its parts copied out, or a fault naming `expr`.
    fn parts_of(&self, expr: ExprId, value: Folded, what: &str) -> Result<(CompositeKind, Vec<Folded>), ConstFault> {
        match self.composite_of(value) {
            Some(composite) => Ok((composite.kind, composite.parts.clone())),
            None => Err(ConstFault::Unsupported { expr, what: format!("{what} that is not a composite value") }),
        }
    }

    /// The value `tree` describes, its composites added to this evaluator's own.
    pub(super) fn adopt(&mut self, tree: &ConstTree) -> Folded {
        match tree {
            ConstTree::Scalar(value) => *value,
            ConstTree::Composite { kind, parts } => {
                let parts = parts.iter().map(|part| self.adopt(part)).collect();
                self.make(*kind, parts)
            }
        }
    }

    /// `value` as a tree: its composites taken apart down to the scalars.
    pub(super) fn export(&self, value: Folded) -> ConstTree {
        match self.composite_of(value) {
            Some(composite) => ConstTree::Composite { kind: composite.kind, parts: composite.parts.iter().map(|part| self.export(*part)).collect() },
            None => ConstTree::Scalar(value),
        }
    }

    /// The tree of the value of `expr` over `bindings`. A composite whole that is unknown is a
    /// fault ([`ConstFault::Unknown`]); a part may be unknown.
    pub fn tree(&mut self, expr: ExprId, bindings: &Bindings) -> Result<ConstTree, ConstFault> {
        match self.fold(expr, bindings)? {
            Folded::Unknown => Err(ConstFault::Unknown { expr }),
            value => Ok(self.export(value)),
        }
    }

    /// The tree of the value of the `const` item `decl`. A composite whole that is unknown is a
    /// fault ([`ConstFault::Unknown`]) on its initializer.
    pub fn constant_tree(&mut self, decl: DeclId) -> Result<ConstTree, ConstFault> {
        match self.fold_constant(decl)? {
            Folded::Unknown => match &self.design.decl_data(decl).kind {
                DeclKind::Const(crate::pom::ConstData { value: Some(expr), .. }) => Err(ConstFault::Unknown { expr: *expr }),
                _ => Err(ConstFault::NoValue { decl }),
            },
            value => Ok(self.export(value)),
        }
    }

    /// Whether the `const` item `decl` folds to a composite value.
    pub fn is_composite_constant(&mut self, decl: DeclId) -> bool {
        self.fold_constant(decl).is_ok_and(|value| self.composite_of(value).is_some())
    }

    // ---- building ----------------------------------------------------------------------------

    /// `(a, b, ..)`: its operands evaluated in the order written.
    pub(super) fn tuple_value(&mut self, items: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let parts = items.iter().map(|item| self.fold(*item, bindings)).collect::<Result<Vec<_>, _>>()?;
        Ok(self.make(CompositeKind::Tuple, parts))
    }

    /// `[a, b, ..]`.
    pub(super) fn array_value(&mut self, items: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let parts = items.iter().map(|item| self.fold(*item, bindings)).collect::<Result<Vec<_>, _>>()?;
        Ok(self.make(CompositeKind::Array, parts))
    }

    /// `[x; N]`: `x` evaluated once, `N` copies of it.
    pub(super) fn repeat_value(&mut self, expr: ExprId, element: ExprId, length: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let count = match self.types.integer_constant(length) {
            Some(count) => count as usize,
            None => match self.known(length, bindings)? {
                Constant::Int(count) if count >= 0 => count as usize,
                _ => return Err(ConstFault::Unsupported { expr, what: "an array length that is not a non-negative integer".to_string() }),
            },
        };
        let value = self.fold(element, bindings)?;
        Ok(self.make(CompositeKind::Array, vec![value; count]))
    }

    /// `Name { field: value, .. }` of a struct or an enum variant: its fields in declaration order,
    /// evaluated in the order written (§1.3.11).
    pub(super) fn struct_value(&mut self, expr: ExprId, path: crate::pom::PathId, inits: &[FieldInit], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let owner = match self.types.ty(expr) {
            Some(Ty::Struct(decl)) => *decl,
            _ => match self.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(variant)) if matches!(self.design.decl_data(variant).kind, DeclKind::Variant(_)) => variant,
                _ => return Err(ConstFault::Unsupported { expr, what: "a struct expression that names neither a struct nor a variant".to_string() }),
            },
        };
        let layout: Vec<DeclId> = self.types.fields(owner).map(|fields| fields.iter().map(|f| f.decl).collect()).unwrap_or_default();
        let mut parts = vec![Folded::Unknown; layout.len()];
        for init in inits {
            let at = match init.res {
                Resolution::Decl(field) => layout.iter().position(|f| *f == field),
                _ => None,
            };
            let Some(at) = at else {
                return Err(ConstFault::Unsupported { expr, what: "a field the struct does not have".to_string() });
            };
            parts[at] = self.fold(init.value, bindings)?;
        }
        self.owned(expr, owner, parts)
    }

    /// `Name(a, b)` of a tuple struct or a variant with positional fields.
    pub(super) fn constructor_value(&mut self, expr: ExprId, owner: DeclId, args: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let parts = args.iter().map(|arg| self.fold(*arg, bindings)).collect::<Result<Vec<_>, _>>()?;
        self.owned(expr, owner, parts)
    }

    /// The value of the struct or variant `owner` with `parts`.
    fn owned(&mut self, expr: ExprId, owner: DeclId, parts: Vec<Folded>) -> Result<Folded, ConstFault> {
        match self.design.decl_data(owner).kind {
            DeclKind::Struct(_) => Ok(self.make(CompositeKind::Struct, parts)),
            DeclKind::Variant(_) => match EnumLayout::variant_tag(self.design, owner) {
                Some(position) => Ok(self.make(CompositeKind::Variant(position as u32), parts)),
                None => Err(ConstFault::Unsupported { expr, what: "a variant outside its enum".to_string() }),
            },
            _ => Err(ConstFault::Unsupported { expr, what: "a value built from a name that is neither a struct nor a variant".to_string() }),
        }
    }

    /// The declaration a callee names when the call builds a struct or a variant.
    pub(super) fn constructed(&self, callee: ExprId) -> Option<DeclId> {
        let mut callee = callee;
        while let ExprKind::Paren(inner) = &self.design.expr_node(callee).kind {
            callee = *inner;
        }
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Struct(_) | DeclKind::Variant(_)) => Some(d),
            _ => None,
        }
    }

    /// Whether any variant of the enum that has `variant` has a field: then its values are
    /// variants held with their parts, not tags.
    pub(super) fn enum_has_fields(&self, variant: DeclId) -> bool {
        let Some(parent) = self.design.decl_data(variant).parent else { return false };
        self.design.decl_data(parent).children.iter().any(|v| self.types.fields(*v).is_some_and(|fields| !fields.is_empty()))
    }

    /// A unit struct or a unit variant of an enum with fields named as a value; `None` for any other
    /// name.
    pub(super) fn unit_value(&mut self, decl: DeclId) -> Option<Folded> {
        match self.design.decl_data(decl).kind {
            DeclKind::Struct(_) => Some(self.make(CompositeKind::Struct, Vec::new())),
            DeclKind::Variant(_) if self.enum_has_fields(decl) => {
                let position = EnumLayout::variant_tag(self.design, decl)?;
                Some(self.make(CompositeKind::Variant(position as u32), Vec::new()))
            }
            _ => None,
        }
    }

    /// `Some(x)` or `None` named as a value; `None` for any other call or name.
    pub(super) fn option_value(&mut self, expr: ExprId, bindings: &Bindings) -> Option<Result<Folded, ConstFault>> {
        match &self.design.expr_node(expr).kind.clone() {
            ExprKind::Path(path) if self.builtin_name(*path) == Some("None") => Some(Ok(self.make(CompositeKind::Option, Vec::new()))),
            ExprKind::Call { callee, args, .. } if self.builtin_callee(*callee) == Some("Some") && args.len() == 1 => {
                let inner = self.fold(args[0].value, bindings);
                Some(inner.map(|inner| self.make(CompositeKind::Option, vec![inner])))
            }
            _ => None,
        }
    }

    /// The name of the built-in a path resolves to.
    fn builtin_name(&self, path: crate::pom::PathId) -> Option<&'static str> {
        match self.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Builtin(id)) => Some(id.info().name),
            _ => None,
        }
    }

    /// The name of the built-in a callee expression names.
    fn builtin_callee(&self, callee: ExprId) -> Option<&'static str> {
        match &self.design.expr_node(callee).kind {
            ExprKind::Path(path) => self.builtin_name(*path),
            ExprKind::Paren(inner) => self.builtin_callee(*inner),
            _ => None,
        }
    }

    /// `lo..hi`, `lo..=hi`, `lo..`, `..hi`: a range value.
    pub(super) fn range_value(&mut self, lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let mut parts = Vec::new();
        for bound in lo.iter().chain(hi.iter()) {
            parts.push(self.fold(*bound, bindings)?);
        }
        Ok(self.make(CompositeKind::Range { low: lo.is_some(), high: hi.is_some(), inclusive }, parts))
    }

    /// `T::default()` (H.8 rule 4): `false`, zero, `None`, and the tuple, array or struct of defaults.
    pub(super) fn default_value(&mut self, expr: ExprId, ty: &Ty) -> Result<Folded, ConstFault> {
        match ty {
            Ty::F64 => Ok(Folded::Known(Constant::Real(Real::new(0.0)))),
            Ty::Bool => Ok(Folded::Known(Constant::Bool(false))),
            Ty::UInt(_) | Ty::Int(_) => Ok(Folded::Known(Constant::Int(0))),
            Ty::Option(_) => Ok(self.make(CompositeKind::Option, Vec::new())),
            Ty::Tuple(items) => {
                let parts = items.iter().map(|item| self.default_value(expr, item)).collect::<Result<Vec<_>, _>>()?;
                Ok(self.make(CompositeKind::Tuple, parts))
            }
            Ty::Array(element, count) => {
                let part = self.default_value(expr, element)?;
                Ok(self.make(CompositeKind::Array, vec![part; *count as usize]))
            }
            Ty::Struct(decl) => {
                let fields: Vec<Ty> = self.types.fields(*decl).map(|fields| fields.iter().map(|f| f.ty.clone()).collect()).unwrap_or_default();
                let parts = fields.iter().map(|field| self.default_value(expr, field)).collect::<Result<Vec<_>, _>>()?;
                Ok(self.make(CompositeKind::Struct, parts))
            }
            _ => Err(ConstFault::Unsupported { expr, what: "the default of this type".to_string() }),
        }
    }

    // ---- reading -----------------------------------------------------------------------------

    /// `e.name` and `e.0` over a tuple or a struct.
    pub(super) fn field_value(&mut self, expr: ExprId, recv: ExprId, name: &str, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let whole = self.fold(recv, bindings)?;
        if whole == Folded::Unknown {
            return Ok(Folded::Unknown);
        }
        let (_, parts) = self.parts_of(expr, whole, "a field read of a value")?;
        let at = self.field_position(recv, name).ok_or(ConstFault::Unsupported { expr, what: "a field the value does not have".to_string() })?;
        parts.get(at).copied().ok_or(ConstFault::Unsupported { expr, what: "a field out of range".to_string() })
    }

    /// The position of the field `name` of the tuple or struct `recv` is.
    fn field_position(&self, recv: ExprId, name: &str) -> Option<usize> {
        match self.types.ty(recv)? {
            Ty::Tuple(_) => name.parse().ok(),
            Ty::Struct(decl) => {
                let fields = self.types.fields(*decl)?;
                match name.parse::<usize>() {
                    Ok(position) if fields.iter().all(|f| f.name.is_none()) => Some(position),
                    _ => fields.iter().position(|f| f.name.as_deref() == Some(name)),
                }
            }
            _ => None,
        }
    }

    /// `a[i]` and `a[i..j]` over an array: an index out of range is a panic naming the expression
    /// (§1 rule 11, §1.5.25); an index that is unknown reads the merge of the elements (§3.3.13).
    pub(super) fn index_value(&mut self, expr: ExprId, base: ExprId, index: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let whole = self.fold(base, bindings)?;
        if whole == Folded::Unknown {
            return Ok(Folded::Unknown);
        }
        let (_, elements) = self.parts_of(expr, whole, "an index into a value")?;
        let index_node = self.design.expr_node(index).kind.clone();
        if let ExprKind::Range { lo, hi, inclusive } = index_node {
            return self.slice_value(expr, &elements, (lo, hi, inclusive), bindings);
        }
        match self.fold(index, bindings)? {
            Folded::Known(Constant::Int(at)) => match usize::try_from(at).ok().and_then(|at| elements.get(at)) {
                Some(element) => Ok(*element),
                None => Err(ConstFault::Panic { expr, what: format!("the index {at} is out of range for an array of {} elements (§1.5.25)", elements.len()) }),
            },
            Folded::Known(_) => Err(ConstFault::Unsupported { expr, what: "an index that is not an integer".to_string() }),
            Folded::Unknown => Ok(elements.iter().copied().reduce(Folded::merge).unwrap_or(Folded::Unknown)),
        }
    }

    /// `a[i..j]`: the array of the elements `i` to `j - 1`; a missing bound is the array's end.
    fn slice_value(&mut self, expr: ExprId, elements: &[Folded], (lo, hi, inclusive): (Option<ExprId>, Option<ExprId>, bool), bindings: &Bindings) -> Result<Folded, ConstFault> {
        let bound = |this: &mut Self, bound: Option<ExprId>, default: usize| -> Result<usize, ConstFault> {
            match bound {
                None => Ok(default),
                // a bound written as a literal or a const name has no type of its own (§1.2.5, as an array length)
                Some(bound) if let Some(at) = this.types.integer_constant(bound) => Ok(at as usize),
                Some(bound) => match this.known(bound, bindings)? {
                    Constant::Int(at) if at >= 0 => Ok(at as usize),
                    _ => Err(ConstFault::Unsupported { expr, what: "a range index bound that is not a non-negative integer".to_string() }),
                },
            }
        };
        let start = bound(self, lo, 0)?;
        let end = bound(self, hi, elements.len())? + usize::from(inclusive && hi.is_some());
        if start > end || end > elements.len() {
            return Err(ConstFault::Panic { expr, what: format!("the range {start}..{end} is out of range for an array of {} elements (§1.5.25)", elements.len()) });
        }
        Ok(self.make(CompositeKind::Array, elements[start..end].to_vec()))
    }

    // ---- comparison --------------------------------------------------------------------------

    /// Whether `a` and `b` are equal under the derived `==` (H.8 rule 2): the same variant and equal
    /// parts, field by field. `None` when an unknown part decides.
    pub(super) fn equal(&self, a: Folded, b: Folded) -> Option<bool> {
        match (self.composite_of(a), self.composite_of(b)) {
            (Some(x), Some(y)) => {
                if x.kind != y.kind || x.parts.len() != y.parts.len() {
                    return Some(false);
                }
                let mut unknown = false;
                for (p, q) in x.parts.iter().zip(&y.parts) {
                    match self.equal(*p, *q) {
                        Some(false) => return Some(false),
                        Some(true) => {}
                        None => unknown = true,
                    }
                }
                if unknown { None } else { Some(true) }
            }
            (None, None) => match (a, b) {
                (Folded::Known(Constant::Real(p)), Folded::Known(Constant::Real(q))) => Some(p.get() == q.get()),
                (Folded::Known(p), Folded::Known(q)) => Some(p == q),
                _ => None,
            },
            _ => Some(false),
        }
    }

    /// The order of `a` and `b` under the derived `PartialOrd` (H.8 rule 3, §1.5.23): fields in
    /// declaration order, a variant by its position then its fields, `None` below every `Some`.
    /// The outer `None` is unknown; the inner one is unordered (a NaN).
    pub(super) fn ordering(&self, a: Folded, b: Folded) -> Option<Option<Ordering>> {
        match (self.composite_of(a), self.composite_of(b)) {
            (Some(x), Some(y)) => {
                let by_kind = match (x.kind, y.kind) {
                    (CompositeKind::Variant(p), CompositeKind::Variant(q)) => p.cmp(&q),
                    (CompositeKind::Option, CompositeKind::Option) => x.parts.len().cmp(&y.parts.len()),
                    _ => Ordering::Equal,
                };
                if by_kind != Ordering::Equal {
                    return Some(Some(by_kind));
                }
                for (p, q) in x.parts.iter().zip(&y.parts) {
                    match self.ordering(*p, *q)? {
                        Some(Ordering::Equal) => {}
                        other => return Some(other),
                    }
                }
                Some(Some(x.parts.len().cmp(&y.parts.len())))
            }
            (None, None) => match (a, b) {
                (Folded::Known(p), Folded::Known(q)) => Some(Self::order(p, q).or_else(|| match (p, q) {
                    (Constant::Bool(p), Constant::Bool(q)) => Some(p.cmp(&q)),
                    _ => None,
                })),
                _ => None,
            },
            _ => Some(None),
        }
    }

    /// `a op b` over two operands one of which is a composite: `==`, `!=`, `<`, `<=`, `>`, `>=`.
    pub(super) fn composite_binary(&self, expr: ExprId, op: BinOp, a: Folded, b: Folded) -> Result<Folded, ConstFault> {
        let holds = |value: bool| Ok(Folded::Known(Constant::Bool(value)));
        match op {
            BinOp::Eq | BinOp::Ne => match self.equal(a, b) {
                Some(equal) => holds(equal == (op == BinOp::Eq)),
                None => Ok(Folded::Unknown),
            },
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => match self.ordering(a, b) {
                None => Ok(Folded::Unknown),
                Some(None) => holds(false),
                Some(Some(order)) => holds(match op {
                    BinOp::Lt => order.is_lt(),
                    BinOp::Le => order.is_le(),
                    BinOp::Gt => order.is_gt(),
                    _ => order.is_ge(),
                }),
            },
            _ => Err(ConstFault::Unsupported { expr, what: "an operator other than a comparison on a composite value".to_string() }),
        }
    }

    // ---- assignment --------------------------------------------------------------------------

    /// `place = value;` over a field or an element: the local the place starts at takes the
    /// composite built again with that part replaced (§1.5.12, §1.4.6). `None` when the target is
    /// a plain name.
    pub(super) fn assign_into(&mut self, expr: ExprId, target: ExprId, op: Option<BinOp>, rhs: ExprId, bindings: &Bindings) -> Option<Result<(), ConstFault>> {
        let (local, steps) = self.place_steps(target)?;
        Some(self.write_place(expr, local, &steps, target, op, rhs, bindings))
    }

    /// The local and the steps a place expression names, outermost step last; `None` for a name.
    fn place_steps(&self, target: ExprId) -> Option<(DeclId, Vec<Step>)> {
        let mut steps = Vec::new();
        let mut at = target;
        loop {
            match &self.design.expr_node(at).kind {
                ExprKind::Paren(inner) => at = *inner,
                ExprKind::Field { expr: recv, name, .. } => {
                    steps.push(Step::Field(self.field_position(*recv, &name.text)?));
                    at = *recv;
                }
                ExprKind::Index { expr: base, index } => {
                    steps.push(Step::Element(*index));
                    at = *base;
                }
                ExprKind::Path(path) => {
                    let Some(Resolution::Decl(local)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                    if steps.is_empty() {
                        return None;
                    }
                    steps.reverse();
                    return Some((local, steps));
                }
                _ => return None,
            }
        }
    }

    /// Evaluates `rhs` and writes it into `local` at `steps`.
    fn write_place(&mut self, expr: ExprId, local: DeclId, steps: &[Step], target: ExprId, op: Option<BinOp>, rhs: ExprId, bindings: &Bindings) -> Result<(), ConstFault> {
        let value = self.fold(rhs, bindings)?;
        let assigned = match op {
            None => value,
            Some(op) => {
                let current = self.fold(target, bindings)?;
                match (current, value) {
                    (Folded::Known(a), Folded::Known(b)) => Folded::Known(self.known_binary(target, op, (target, a), (rhs, b))?),
                    (a, b) => self.unknown_binary(target, op, a, b),
                }
            }
        };
        let Some(root) = self.locals.get(&local).copied() else {
            return Err(ConstFault::Unsupported { expr, what: "an assignment into a local with no value".to_string() });
        };
        let mut positions = Vec::new();
        for step in steps {
            positions.push(match step {
                Step::Field(at) => *at,
                Step::Element(index) => match self.known(*index, bindings)? {
                    Constant::Int(at) if at >= 0 => at as usize,
                    _ => return Err(ConstFault::Unsupported { expr: *index, what: "an element index that is not a known non-negative integer".to_string() }),
                },
            });
        }
        let rebuilt = self.replaced(expr, root, &positions, assigned)?;
        self.locals.insert(local, rebuilt);
        Ok(())
    }

    /// `whole` with the part at `positions` replaced by `value`.
    fn replaced(&mut self, expr: ExprId, whole: Folded, positions: &[usize], value: Folded) -> Result<Folded, ConstFault> {
        let Some((&first, rest)) = positions.split_first() else { return Ok(value) };
        let (kind, mut parts) = self.parts_of(expr, whole, "an assignment into a value")?;
        let Some(part) = parts.get(first).copied() else {
            return Err(ConstFault::Panic { expr, what: format!("the index {first} is out of range for an array of {} elements (§1.5.25)", parts.len()) });
        };
        parts[first] = self.replaced(expr, part, rest, value)?;
        Ok(self.make(kind, parts))
    }

    // ---- patterns ----------------------------------------------------------------------------

    /// Whether `value` matches the pattern `pat` of a composite form (a tuple, a tuple struct, a
    /// struct, an array, `None`, a unit struct or a variant with fields); the names it binds are
    /// bound. `None` when the pattern is not one of those, so the scalar patterns decide.
    pub(super) fn matches_composite(&mut self, pat: PatId, value: Folded, at: ExprId, bindings: &Bindings) -> Option<Result<bool, ConstFault>> {
        let kind = self.design.pat_node(pat).kind.clone();
        match &kind {
            PatKind::Tuple(pats) => Some(self.matches_parts(pats, value, at, bindings)),
            PatKind::Array(pats) => Some(self.matches_array(pats, value, at, bindings)),
            PatKind::TupleStruct { path, pats } => {
                let name = self.builtin_name(*path);
                if name == Some("Some") {
                    return Some(self.matches_option(pats, value, at, bindings));
                }
                let variant = self.pattern_owner(*path);
                Some(self.matches_owner(variant, value, at, bindings, |this, parts| this.matches_each(pats, &parts, at, bindings)))
            }
            PatKind::Struct { path, fields, .. } => {
                let owner = self.pattern_owner(*path);
                let fields = fields.clone();
                Some(self.matches_owner(owner, value, at, bindings, |this, parts| {
                    for field in &fields {
                        let Resolution::Decl(decl) = field.res else { return Err(ConstFault::Unsupported { expr: at, what: "a field pattern that names no field".to_string() }) };
                        let layout = this.pattern_layout(owner);
                        let Some(position) = layout.iter().position(|f| *f == decl) else { return Err(ConstFault::Unsupported { expr: at, what: "a field the struct does not have".to_string() }) };
                        let part = parts[position];
                        match (field.pat, field.binding) {
                            (Some(sub), _) => {
                                if !this.matches(sub, part, at, bindings)? {
                                    return Ok(false);
                                }
                            }
                            (None, Some(binding)) => {
                                this.locals.insert(binding, part);
                            }
                            (None, None) => {}
                        }
                    }
                    Ok(true)
                }))
            }
            PatKind::Ident { res: Resolution::Builtin(id), .. } if id.info().name == "None" => Some(Ok(self.composite_of(value).is_some_and(|c| c.kind == CompositeKind::Option && c.parts.is_empty()))),
            PatKind::Path(path) if self.builtin_name(*path) == Some("None") => Some(Ok(self.composite_of(value).is_some_and(|c| c.kind == CompositeKind::Option && c.parts.is_empty()))),
            PatKind::Path(path) => {
                let owner = self.pattern_owner(*path);
                let Some(decl) = owner else { return None };
                if matches!(self.design.decl_data(decl).kind, DeclKind::Struct(_)) || (matches!(self.design.decl_data(decl).kind, DeclKind::Variant(_)) && self.enum_has_fields(decl)) {
                    return Some(self.matches_owner(Some(decl), value, at, bindings, |_, _| Ok(true)));
                }
                None
            }
            PatKind::Ident { res: Resolution::Decl(decl), .. } if matches!(self.design.decl_data(*decl).kind, DeclKind::Variant(_)) && self.enum_has_fields(*decl) => Some(self.matches_owner(Some(*decl), value, at, bindings, |_, _| Ok(true))),
            PatKind::Ident { res: Resolution::Decl(decl), .. } if matches!(self.design.decl_data(*decl).kind, DeclKind::Struct(_)) => Some(self.matches_owner(Some(*decl), value, at, bindings, |_, _| Ok(true))),
            _ => None,
        }
    }

    /// The struct or variant a pattern path names.
    fn pattern_owner(&self, path: crate::pom::PathId) -> Option<DeclId> {
        match self.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Struct(_) | DeclKind::Variant(_)) => Some(d),
            _ => None,
        }
    }

    /// The field declarations of the struct or variant `owner`.
    fn pattern_layout(&self, owner: Option<DeclId>) -> Vec<DeclId> {
        owner.and_then(|owner| self.types.fields(owner)).map(|fields| fields.iter().map(|f| f.decl).collect()).unwrap_or_default()
    }

    /// `value` against the struct or variant `owner`: the same variant (a struct has one shape), then
    /// `parts_match` over its parts.
    fn matches_owner(&mut self, owner: Option<DeclId>, value: Folded, at: ExprId, _bindings: &Bindings, parts_match: impl FnOnce(&mut Self, Vec<Folded>) -> Result<bool, ConstFault>) -> Result<bool, ConstFault> {
        let Some(owner) = owner else { return Err(ConstFault::Unsupported { expr: at, what: "a pattern that names no struct or variant".to_string() }) };
        if value == Folded::Unknown {
            return Err(ConstFault::Unknown { expr: at });
        }
        let (kind, parts) = self.parts_of(at, value, "a pattern over a value")?;
        match (self.design.decl_data(owner).kind.clone(), kind) {
            (DeclKind::Struct(_), CompositeKind::Struct) => parts_match(self, parts),
            (DeclKind::Variant(_), CompositeKind::Variant(position)) => match EnumLayout::variant_tag(self.design, owner) {
                Some(wanted) if wanted as u32 == position => parts_match(self, parts),
                _ => Ok(false),
            },
            _ => Ok(false),
        }
    }

    /// `(a, b, ..)` over a tuple.
    fn matches_parts(&mut self, pats: &[PatId], value: Folded, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        if value == Folded::Unknown {
            return Err(ConstFault::Unknown { expr: at });
        }
        let (_, parts) = self.parts_of(at, value, "a tuple pattern over a value")?;
        self.matches_each(pats, &parts, at, bindings)
    }

    /// Each sub-pattern against the part at its position; `..` skips what lies between.
    fn matches_each(&mut self, pats: &[PatId], parts: &[Folded], at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        let rest = pats.iter().position(|p| matches!(self.design.pat_node(*p).kind, PatKind::Range { lo: None, hi: None, .. }));
        let (before, after): (&[PatId], &[PatId]) = match rest {
            Some(at_rest) => (&pats[..at_rest], &pats[at_rest + 1..]),
            None => (pats, &[]),
        };
        if parts.len() < before.len() + after.len() {
            return Ok(false);
        }
        for (pat, part) in before.iter().zip(parts) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        for (pat, part) in after.iter().rev().zip(parts.iter().rev()) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `[a, .., z]`, `[h, rest @ ..]` over an array: with no `..`, the lengths agree.
    fn matches_array(&mut self, pats: &[PatId], value: Folded, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        if value == Folded::Unknown {
            return Err(ConstFault::Unknown { expr: at });
        }
        let (_, parts) = self.parts_of(at, value, "an array pattern over a value")?;
        let rest = pats.iter().position(|p| self.is_rest(*p));
        let Some(at_rest) = rest else {
            return if parts.len() == pats.len() { self.matches_each(pats, &parts, at, bindings) } else { Ok(false) };
        };
        let (before, after) = (&pats[..at_rest], &pats[at_rest + 1..]);
        if parts.len() < before.len() + after.len() {
            return Ok(false);
        }
        let middle = parts[before.len()..parts.len() - after.len()].to_vec();
        if let PatKind::Ident { binding, sub: Some(_), .. } = self.design.pat_node(pats[at_rest]).kind {
            let slice = self.make(CompositeKind::Array, middle);
            self.locals.insert(binding, slice);
        }
        for (pat, part) in before.iter().zip(&parts) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        for (pat, part) in after.iter().rev().zip(parts.iter().rev()) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether `pat` is `..` or `name @ ..`.
    fn is_rest(&self, pat: PatId) -> bool {
        match &self.design.pat_node(pat).kind {
            PatKind::Range { lo: None, hi: None, .. } => true,
            PatKind::Ident { sub: Some(sub), .. } => self.is_rest(*sub),
            _ => false,
        }
    }

    /// `Some(p)` over an `Option`.
    fn matches_option(&mut self, pats: &[PatId], value: Folded, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        if value == Folded::Unknown {
            return Err(ConstFault::Unknown { expr: at });
        }
        let (_, parts) = self.parts_of(at, value, "a `Some` pattern over a value")?;
        match (parts.first(), pats) {
            (Some(part), [inner]) => self.matches(*inner, *part, at, bindings),
            _ => Ok(false),
        }
    }

    // ---- sequences and their rows ------------------------------------------------------------

    /// The elements of the iterator, array or range `value` is, in order. A range without both
    /// bounds, or over floats, is not an iterator (H.6 rule 3).
    pub(super) fn elements_of(&self, expr: ExprId, value: Folded) -> Result<Vec<Folded>, ConstFault> {
        let (kind, parts) = self.parts_of(expr, value, "an iteration over a value")?;
        match kind {
            CompositeKind::Array => Ok(parts),
            CompositeKind::Range { low: true, high: true, inclusive } => match (parts[0], parts[1]) {
                (Folded::Known(Constant::Int(first)), Folded::Known(Constant::Int(last))) => {
                    let end = if inclusive { last + 1 } else { last };
                    if end.saturating_sub(first) > (1 << 24) {
                        return Err(ConstFault::Panic { expr, what: "an iteration over more than 2^24 values (§1 rule 11)".to_string() });
                    }
                    Ok((first..end).map(|v| Folded::Known(Constant::Int(v))).collect())
                }
                _ => Err(ConstFault::Unknown { expr }),
            },
            _ => Err(ConstFault::Unsupported { expr, what: "an iteration over a value that is not an array, a range or an iterator".to_string() }),
        }
    }

    /// A row of Appendix H.5 to H.7 over an array, a range or an iterator: `Some` result, `None`
    /// when `method` is not one of those rows.
    pub(super) fn sequence_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Option<Result<Folded, ConstFault>> {
        let is_row = matches!(
            method,
            Method::ArrayLen | Method::ArrayIsEmpty | Method::ArrayIter | Method::ArrayContains | Method::ArrayFirst | Method::ArrayLast | Method::ArrayGet | Method::RangeContains | Method::RangeIsEmpty | Method::IteratorMap | Method::IteratorFilter | Method::IteratorFold | Method::IteratorSum | Method::IteratorCount | Method::IteratorZip | Method::IteratorEnumerate | Method::IteratorRev | Method::IteratorAny | Method::IteratorAll
        );
        is_row.then(|| self.run_row(expr, method, recv, args, bindings))
    }

    /// The closure or function an argument of a row is.
    fn closure_argument(&mut self, expr: ExprId, arg: ExprId) -> Result<FnValue, ConstFault> {
        self.fn_value_of(arg).ok_or(ConstFault::Unsupported { expr, what: "an argument that is not a closure or a function".to_string() })
    }

    /// `function` called on `values`.
    fn call_with(&mut self, expr: ExprId, function: &FnValue, values: Vec<Folded>, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let arguments = values.into_iter().map(|value| Argument { value, function: None }).collect();
        self.apply_value(expr, function.clone(), arguments, bindings)
    }

    /// `function` called on `values`, its result a known `bool`.
    fn test_with(&mut self, expr: ExprId, function: &FnValue, values: Vec<Folded>, bindings: &Bindings) -> Result<bool, ConstFault> {
        match self.call_with(expr, function, values, bindings)? {
            Folded::Known(Constant::Bool(holds)) => Ok(holds),
            Folded::Known(_) => Err(ConstFault::Unsupported { expr, what: "a closure that does not give a `bool`".to_string() }),
            Folded::Unknown => Err(ConstFault::Unknown { expr }),
        }
    }

    /// The `Some`/`None` of an optional element.
    fn optional(&mut self, element: Option<Folded>) -> Folded {
        self.make(CompositeKind::Option, element.into_iter().collect())
    }

    /// One row, its receiver and arguments evaluated here.
    fn run_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let receiver = self.fold(recv, bindings)?;
        if receiver == Folded::Unknown {
            return Ok(Folded::Unknown);
        }
        let known = |value: bool| Ok(Folded::Known(Constant::Bool(value)));
        let count = |value: usize| Ok(Folded::Known(Constant::Int(value as i128)));
        match method {
            Method::ArrayLen => count(self.elements_of(expr, receiver)?.len()),
            Method::ArrayIsEmpty => known(self.elements_of(expr, receiver)?.is_empty()),
            Method::ArrayIter => {
                let elements = self.elements_of(expr, receiver)?;
                Ok(self.make(CompositeKind::Array, elements))
            }
            Method::ArrayContains => {
                let sought = self.fold(args[0], bindings)?;
                for element in self.elements_of(expr, receiver)? {
                    match self.equal(element, sought) {
                        Some(true) => return known(true),
                        Some(false) => {}
                        None => return Ok(Folded::Unknown),
                    }
                }
                known(false)
            }
            Method::ArrayFirst => {
                let elements = self.elements_of(expr, receiver)?;
                Ok(self.optional(elements.first().copied()))
            }
            Method::ArrayLast => {
                let elements = self.elements_of(expr, receiver)?;
                Ok(self.optional(elements.last().copied()))
            }
            Method::ArrayGet => {
                let elements = self.elements_of(expr, receiver)?;
                match self.fold(args[0], bindings)? {
                    Folded::Known(Constant::Int(at)) => Ok(self.optional(usize::try_from(at).ok().and_then(|at| elements.get(at)).copied())),
                    _ => Ok(Folded::Unknown),
                }
            }
            Method::RangeContains => self.range_contains(expr, receiver, args[0], bindings),
            Method::RangeIsEmpty => {
                let (kind, parts) = self.parts_of(expr, receiver, "`is_empty` of a value")?;
                match (kind, parts.as_slice()) {
                    (CompositeKind::Range { low: true, high: true, inclusive }, [lo, hi]) => match self.ordering(*lo, *hi) {
                        Some(Some(order)) => known(!(if inclusive { order.is_le() } else { order.is_lt() })),
                        Some(None) => known(true),
                        None => Ok(Folded::Unknown),
                    },
                    _ => Err(ConstFault::Unsupported { expr, what: "`is_empty` of a range with a missing bound".to_string() }),
                }
            }
            Method::IteratorMap | Method::IteratorFilter | Method::IteratorAny | Method::IteratorAll => {
                let function = self.closure_argument(expr, args[0])?;
                let elements = self.elements_of(expr, receiver)?;
                self.adapt(expr, method, &function, elements, bindings)
            }
            Method::IteratorFold => {
                let mut accumulator = self.fold(args[0], bindings)?;
                let function = self.closure_argument(expr, args[1])?;
                for element in self.elements_of(expr, receiver)? {
                    accumulator = self.call_with(expr, &function, vec![accumulator, element], bindings)?;
                }
                Ok(accumulator)
            }
            Method::IteratorSum => self.sum(expr, receiver),
            Method::IteratorCount => count(self.elements_of(expr, receiver)?.len()),
            Method::IteratorZip => {
                let other = self.fold(args[0], bindings)?;
                let (first, second) = (self.elements_of(expr, receiver)?, self.elements_of(expr, other)?);
                let pairs = first.into_iter().zip(second).map(|(a, b)| self.make(CompositeKind::Tuple, vec![a, b])).collect();
                Ok(self.make(CompositeKind::Array, pairs))
            }
            Method::IteratorEnumerate => {
                let elements = self.elements_of(expr, receiver)?;
                let pairs = elements.into_iter().enumerate().map(|(k, x)| self.make(CompositeKind::Tuple, vec![Folded::Known(Constant::Int(k as i128)), x])).collect();
                Ok(self.make(CompositeKind::Array, pairs))
            }
            Method::IteratorRev => {
                let mut elements = self.elements_of(expr, receiver)?;
                elements.reverse();
                Ok(self.make(CompositeKind::Array, elements))
            }
            _ => Err(ConstFault::Unsupported { expr, what: "a row outside Appendix H.5 to H.7".to_string() }),
        }
    }

    /// `map`, `filter`, `any` and `all`: the closure runs once per element in order; `any` and `all`
    /// stop at the element that decides (H.7 rule 10).
    fn adapt(&mut self, expr: ExprId, method: Method, function: &FnValue, elements: Vec<Folded>, bindings: &Bindings) -> Result<Folded, ConstFault> {
        match method {
            Method::IteratorMap => {
                let mut mapped = Vec::with_capacity(elements.len());
                for element in elements {
                    mapped.push(self.call_with(expr, function, vec![element], bindings)?);
                }
                Ok(self.make(CompositeKind::Array, mapped))
            }
            Method::IteratorFilter => {
                let mut kept = Vec::new();
                for element in elements {
                    if self.test_with(expr, function, vec![element], bindings)? {
                        kept.push(element);
                    }
                }
                Ok(self.make(CompositeKind::Array, kept))
            }
            Method::IteratorAny => {
                for element in elements {
                    if self.test_with(expr, function, vec![element], bindings)? {
                        return Ok(Folded::Known(Constant::Bool(true)));
                    }
                }
                Ok(Folded::Known(Constant::Bool(false)))
            }
            _ => {
                for element in elements {
                    if !self.test_with(expr, function, vec![element], bindings)? {
                        return Ok(Folded::Known(Constant::Bool(false)));
                    }
                }
                Ok(Folded::Known(Constant::Bool(true)))
            }
        }
    }

    /// `sum()`: the elements added with `+` from the first, starting from zero (H.7 rule 9).
    fn sum(&mut self, expr: ExprId, receiver: Folded) -> Result<Folded, ConstFault> {
        let elements = self.elements_of(expr, receiver)?;
        let zero = match self.types.ty(expr) {
            Some(Ty::F64) => Constant::Real(Real::new(0.0)),
            _ => Constant::Int(0),
        };
        let mut total = Folded::Known(zero);
        for element in elements {
            total = match (total, element) {
                (Folded::Known(a), Folded::Known(b)) => Folded::Known(self.known_binary(expr, BinOp::Add, (expr, a), (expr, b))?),
                (a, b) => self.unknown_binary(expr, BinOp::Add, a, b),
            };
        }
        Ok(total)
    }

    /// `r.contains(x)` (H.6 rule 1): each bound the range has is tested; no float is in a range
    /// with a NaN bound.
    fn range_contains(&mut self, expr: ExprId, range: Folded, sought: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let value = self.fold(sought, bindings)?;
        let (kind, parts) = self.parts_of(expr, range, "`contains` of a value")?;
        let CompositeKind::Range { low, high, inclusive } = kind else {
            return Err(ConstFault::Unsupported { expr, what: "`contains` of a value that is not a range".to_string() });
        };
        let mut holds = true;
        let mut bounds = parts.into_iter();
        if low {
            let lo = bounds.next().ok_or(ConstFault::Unsupported { expr, what: "a range with no low bound".to_string() })?;
            match self.ordering(lo, value) {
                None => return Ok(Folded::Unknown),
                Some(order) => holds &= order.is_some_and(|o| o.is_le()),
            }
        }
        if high {
            let hi = bounds.next().ok_or(ConstFault::Unsupported { expr, what: "a range with no high bound".to_string() })?;
            match self.ordering(value, hi) {
                None => return Ok(Folded::Unknown),
                Some(order) => holds &= order.is_some_and(|o| if inclusive { o.is_le() } else { o.is_lt() }),
            }
        }
        Ok(Folded::Known(Constant::Bool(holds)))
    }

    /// A method of `Option` that is not one of the sequence rows: `is_some`, `is_none`, `unwrap`,
    /// `expect`, `unwrap_or`, `unwrap_or_else`, `map`, `and_then` and `or` over a composite
    /// `Option`; `None` for any other method.
    pub(super) fn option_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Option<Result<Folded, ConstFault>> {
        if !matches!(method, Method::IsSome | Method::IsNone | Method::Unwrap | Method::Expect | Method::UnwrapOr | Method::UnwrapOrElse | Method::Map | Method::AndThen | Method::Or) || !matches!(self.types.ty(recv), Some(Ty::Option(_))) {
            return None;
        }
        Some(self.run_option_row(expr, method, recv, args, bindings))
    }

    /// One row of `Option`.
    fn run_option_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let receiver = self.fold(recv, bindings)?;
        if receiver == Folded::Unknown {
            return Ok(Folded::Unknown);
        }
        let (_, parts) = self.parts_of(expr, receiver, "an `Option` row over a value")?;
        let held = parts.first().copied();
        let known = |value: bool| Ok(Folded::Known(Constant::Bool(value)));
        match (method, held) {
            (Method::IsSome, held) => known(held.is_some()),
            (Method::IsNone, held) => known(held.is_none()),
            (Method::Unwrap | Method::Expect, Some(value)) => Ok(value),
            (Method::Unwrap | Method::Expect, None) => Err(ConstFault::Panic { expr, what: "`unwrap` of a `None` (H.4 rule 2)".to_string() }),
            (Method::UnwrapOr, Some(value)) => {
                self.fold(args[0], bindings)?;
                Ok(value)
            }
            (Method::UnwrapOr, None) => self.fold(args[0], bindings),
            (Method::UnwrapOrElse, Some(value)) => Ok(value),
            (Method::UnwrapOrElse, None) => {
                let function = self.closure_argument(expr, args[0])?;
                self.call_with(expr, &function, Vec::new(), bindings)
            }
            (Method::Map, Some(value)) => {
                let function = self.closure_argument(expr, args[0])?;
                let mapped = self.call_with(expr, &function, vec![value], bindings)?;
                Ok(self.make(CompositeKind::Option, vec![mapped]))
            }
            (Method::AndThen, Some(value)) => {
                let function = self.closure_argument(expr, args[0])?;
                self.call_with(expr, &function, vec![value], bindings)
            }
            (Method::Map | Method::AndThen, None) => Ok(self.make(CompositeKind::Option, Vec::new())),
            (Method::Or, Some(_)) => Ok(receiver),
            (Method::Or, None) => self.fold(args[0], bindings),
            _ => Err(ConstFault::Unsupported { expr, what: "this `Option` row in a const expression".to_string() }),
        }
    }
}
