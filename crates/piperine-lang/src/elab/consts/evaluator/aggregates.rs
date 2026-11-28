//! The aggregates of a const expression: tuples, arrays, structs and enum variants made and read by field and index (§3.2).

use piperine_ir::{Constant, Real};
use crate::elab::value::compound::EnumLayout;
use super::{CompositeKind, Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm, StructureForm, ValueForm};
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FieldInit, Resolution};
use crate::types::{Length, Nominal, Ty};

impl ConstEval<'_, '_> {
    /// `(a, b, ..)`: its operands evaluated in the order written.
    pub(super) fn tuple_value(&mut self, items: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let parts = items.iter().map(|item| self.fold(*item, bindings)).collect::<Result<Vec<_>, _>>()?;
        Ok(self.make(CompositeKind::Tuple, parts))
    }

    /// `[a, b, ..]`.
    pub(super) fn array_value(&mut self, items: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let parts = items.iter().map(|item| self.fold(*item, bindings)).collect::<Result<Vec<_>, _>>()?;
        Ok(self.make(CompositeKind::Array, parts))
    }

    /// `[x; N]`: `x` evaluated once, `N` copies of it.
    pub(super) fn repeat_value(&mut self, expr: ExprId, element: ExprId, length: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let count = match self.integer(length) {
            Some(count) => count as usize,
            None => match self.fold(length, bindings)? {
                Constant::Int(count) if count >= 0 => count as usize,
                _ => return Err(ConstFault::refused(expr, ConstantForm::ArrayLengthThatIsNotNonNegative)),
            },
        };
        let value = self.fold(element, bindings)?;
        Ok(self.make(CompositeKind::Array, vec![value; count]))
    }

    /// `Name { field: value, .. }` of a struct or an enum variant: its fields in declaration order,
    /// evaluated in the order written (§1.3.11).
    pub(super) fn struct_value(&mut self, expr: ExprId, path: crate::pom::PathId, inits: &[FieldInit], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let owner = match self.typed(expr) {
            Some(Ty::Struct(Nominal { decl, .. })) => decl,
            _ => match self.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(variant)) if matches!(self.design.decl_data(variant).kind, DeclKind::Variant(_)) => variant,
                _ => return Err(ConstFault::refused(expr, ValueForm::StructExpressionThatNamesNeitherStructNor)),
            },
        };
        let layout: Vec<DeclId> = self.types.fields(owner).map(|fields| fields.iter().map(|f| f.decl).collect()).unwrap_or_default();
        let mut parts: Vec<Option<Constant>> = vec![None; layout.len()];
        for init in inits {
            let at = match init.res {
                Resolution::Decl(field) => layout.iter().position(|f| *f == field),
                _ => None,
            };
            let Some(at) = at else {
                return Err(ConstFault::refused(expr, ConstantForm::FieldStructDoesNotHave));
            };
            parts[at] = Some(self.fold(init.value, bindings)?);
        }
        let Some(parts) = parts.into_iter().collect::<Option<Vec<_>>>() else {
            return Err(ConstFault::refused(expr, ConstantForm::StructExpressionThatLeavesFieldOut));
        };
        self.owned(expr, owner, parts)
    }

    /// `Name(a, b)` of a tuple struct or a variant with positional fields.
    pub(super) fn constructor_value(&mut self, expr: ExprId, owner: DeclId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let parts = args.iter().map(|arg| self.fold(*arg, bindings)).collect::<Result<Vec<_>, _>>()?;
        self.owned(expr, owner, parts)
    }

    /// The value of the struct or variant `owner` with `parts`.
    pub(super) fn owned(&mut self, expr: ExprId, owner: DeclId, parts: Vec<Constant>) -> Result<Constant, ConstFault> {
        match self.design.decl_data(owner).kind {
            DeclKind::Struct(_) => Ok(self.make(CompositeKind::Struct, parts)),
            DeclKind::Variant(_) => match EnumLayout::variant_tag(self.design, owner) {
                Some(position) => Ok(self.make(CompositeKind::Variant(position as u32), parts)),
                None => Err(ConstFault::refused(expr, ValueForm::VariantOutsideItsEnum)),
            },
            _ => Err(ConstFault::refused(expr, ConstantForm::ValueBuiltFromNameThatIsNeither)),
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
    pub(super) fn unit_value(&mut self, decl: DeclId) -> Option<Constant> {
        match self.design.decl_data(decl).kind {
            DeclKind::Struct(_) => Some(self.make(CompositeKind::Struct, Vec::new())),
            DeclKind::Variant(_) if self.enum_has_fields(decl) => {
                let position = EnumLayout::variant_tag(self.design, decl)?;
                Some(self.make(CompositeKind::Variant(position as u32), Vec::new()))
            }
            _ => None,
        }
    }

    /// The name of the built-in a path resolves to.
    pub(super) fn builtin_name(&self, path: crate::pom::PathId) -> Option<&'static str> {
        match self.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Builtin(id)) => Some(id.info().name),
            _ => None,
        }
    }

    /// The name of the built-in a callee expression names.
    pub(super) fn builtin_callee(&self, callee: ExprId) -> Option<&'static str> {
        match &self.design.expr_node(callee).kind {
            ExprKind::Path(path) => self.builtin_name(*path),
            ExprKind::Paren(inner) => self.builtin_callee(*inner),
            _ => None,
        }
    }

    /// `lo..hi`, `lo..=hi`, `lo..`, `..hi`: a range value.
    pub(super) fn range_value(&mut self, lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let mut parts = Vec::new();
        for bound in lo.iter().chain(hi.iter()) {
            parts.push(self.fold(*bound, bindings)?);
        }
        Ok(self.make(CompositeKind::Range { low: lo.is_some(), high: hi.is_some(), inclusive }, parts))
    }

    /// `logic("10xx")` (§22.1.4) written in a constant context: the `Logic<N>` struct whose two planes the literal pattern
    /// names. A pattern that is no literal is a fault (it needs the text known where it is read).
    pub(super) fn logic_literal(&mut self, expr: ExprId, args: &[ExprId]) -> Result<Constant, ConstFault> {
        let text = match args.first().map(|arg| &self.design.expr_node(*arg).kind) {
            Some(ExprKind::Lit(lit)) => lit.str_value(),
            _ => None,
        };
        let Some(text) = text else { return Err(ConstFault::refused(expr, ConstantForm::LogicPatternThatIsNoLiteral)) };
        let pattern = crate::types::LogicPattern::parse(&text).map_err(|_| ConstFault::refused(expr, ConstantForm::LogicPatternWithCharacterThatNamesNo))?;
        let (binary, value) = pattern.planes();
        Ok(self.make(CompositeKind::Struct, vec![Constant::Int(binary as i128), Constant::Int(value as i128)]))
    }

    /// `T::default()` (H.8 rule 4): `false`, zero, `None`, and the tuple, array or struct of defaults.
    pub(super) fn default_value(&mut self, expr: ExprId, ty: &Ty) -> Result<Constant, ConstFault> {
        match ty {
            Ty::F64 => Ok(Constant::Real(Real::new(0.0))),
            Ty::Bool => Ok(Constant::Bool(false)),
            Ty::UInt(_) | Ty::Int(_) => Ok(Constant::Int(0)),
            Ty::Option(_) => Ok(self.make(CompositeKind::Option, Vec::new())),
            Ty::Tuple(items) => {
                let parts = items.iter().map(|item| self.default_value(expr, item)).collect::<Result<Vec<_>, _>>()?;
                Ok(self.make(CompositeKind::Tuple, parts))
            }
            Ty::Array(element, Length::Fixed(count)) => {
                let part = self.default_value(expr, element)?;
                Ok(self.make(CompositeKind::Array, vec![part; *count as usize]))
            }
            Ty::Struct(Nominal { decl, .. }) => {
                let fields: Vec<Ty> = self.types.fields(*decl).map(|fields| fields.iter().map(|f| f.ty.clone()).collect()).unwrap_or_default();
                let parts = fields.iter().map(|field| self.default_value(expr, field)).collect::<Result<Vec<_>, _>>()?;
                Ok(self.make(CompositeKind::Struct, parts))
            }
            _ => Err(ConstFault::refused(expr, ConstantForm::DefaultOfThisType)),
        }
    }

    /// `e.name` and `e.0` over a tuple or a struct.
    pub(super) fn field_value(&mut self, expr: ExprId, recv: ExprId, name: &str, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let whole = self.fold(recv, bindings)?;
        // `self.vth0` of a template: the param it binds, else the module's default (`templates.rs`)
        if matches!(self.typed(recv), Some(Ty::Module(_))) {
            return self.template_param(expr, whole, name);
        }
        let (_, parts) = self.parts_of(expr, whole, "a field read of a value")?;
        let at = self.field_position(recv, name).ok_or(ConstFault::refused(expr, ConstantForm::FieldValueDoesNotHave))?;
        parts.get(at).copied().ok_or(ConstFault::refused(expr, ConstantForm::FieldOutOfRange))
    }

    /// The position of the field `name` of the tuple or struct `recv` is.
    pub(super) fn field_position(&self, recv: ExprId, name: &str) -> Option<usize> {
        match self.typed(recv)? {
            Ty::Tuple(_) => name.parse().ok(),
            Ty::Struct(Nominal { decl, .. }) => {
                let fields = self.types.fields(decl)?;
                match name.parse::<usize>() {
                    Ok(position) if fields.iter().all(|f| f.name.is_none()) => Some(position),
                    _ => fields.iter().position(|f| f.name.as_deref() == Some(name)),
                }
            }
            _ => None,
        }
    }

    /// `a[i]` and `a[i..j]` over an array: an index out of range is a panic naming the expression
    /// (§1 rule 11, §1.5.25);
    pub(super) fn index_value(&mut self, expr: ExprId, base: ExprId, index: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let whole = self.fold(base, bindings)?;
        let (_, elements) = self.parts_of(expr, whole, "an index into a value")?;
        let index_node = self.design.expr_node(index).kind.clone();
        if let ExprKind::Range { lo, hi, inclusive } = index_node {
            return self.slice_value(expr, &elements, (lo, hi, inclusive), bindings);
        }
        match self.fold(index, bindings)? {
            Constant::Int(at) => match usize::try_from(at).ok().and_then(|at| elements.get(at)) {
                Some(element) => Ok(*element),
                None => Err(ConstFault::Panic { expr, what: format!("the index {at} is out of range for an array of {} elements (§1.5.25)", elements.len()) }),
            },
            _ => Err(ConstFault::refused(expr, StructureForm::IndexThatIsNotInteger)),
        }
    }

    /// `a[i..j]`: the array of the elements `i` to `j - 1`; a missing bound is the array's end.
    pub(super) fn slice_value(&mut self, expr: ExprId, elements: &[Constant], (lo, hi, inclusive): (Option<ExprId>, Option<ExprId>, bool), bindings: &Bindings) -> Result<Constant, ConstFault> {
        let bound = |this: &mut Self, bound: Option<ExprId>, default: usize| -> Result<usize, ConstFault> {
            match bound {
                None => Ok(default),
                // a bound written as a literal or a const name has no type of its own (§1.2.5, as an array length)
                Some(bound) if let Some(at) = this.integer(bound) => Ok(at as usize),
                Some(bound) => match this.fold(bound, bindings)? {
                    Constant::Int(at) if at >= 0 => Ok(at as usize),
                    _ => Err(ConstFault::refused(expr, ConstantForm::RangeIndexBoundThatIsNotNon)),
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
}
