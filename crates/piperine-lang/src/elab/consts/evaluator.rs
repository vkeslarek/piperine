//! The evaluator: [`ConstEval`], its state, the entry that folds an expression and the families of values it folds,
//! one file each below `evaluator/`.

mod aggregates;
mod comparison;
mod composite;
mod control;
mod dispatch;
mod floats;
mod generated;
mod methods;
mod names;
mod numbers;
mod options;
mod parsed;
mod patterns;
mod places;
mod sequences;
mod templates;

pub use composite::{CompositeKind, ConstTree};

use std::collections::BTreeMap;
use piperine_ir::Constant;
use crate::elab::value::compound::EnumLayout;
use crate::elab::value::files::IncludedFiles;
use crate::pom::{DeclId, Design};
use crate::types::Types;
use composite::Composite;
use control::FnValue;
use super::{Bindings, ConstFault, Exit};

/// The evaluator: one per elaboration run, so a `const` item is folded once per run.
pub struct ConstEval<'d, 't> {
    design: &'d Design,
    types: &'t Types<'d>,
    consts: BTreeMap<DeclId, Constant>,
    evaluations: usize,
    locals: BTreeMap<DeclId, Constant>,
    depth: usize,
    /// The module-body `let`s being evaluated, innermost last: a read of one of them is a cycle.
    open_lets: Vec<DeclId>,
    /// The function each local that holds one stands for (`control.rs`).
    fn_values: BTreeMap<DeclId, FnValue>,
    /// The value a `break` or `return` on its way to its target carries; `None` for none.
    leaving: Option<Constant>,
    /// The `const` items being folded, innermost last: a read of one of them is a cycle (§3.9.2).
    open_constants: Vec<DeclId>,
    /// The files `include_str!` has read, each once (§20 rule 3).
    files: IncludedFiles,
    /// The composite values the evaluation holds (`composite.rs`).
    composites: Vec<Composite>,
    /// The integers wider than 64 bits the evaluation holds, by handle (`numbers.rs`).
    wides: Vec<piperine_ir::WideData>,
    /// How many of them the elaborator has copied into the IR (`take_new_wides`).
    wides_taken: usize,
    /// What the generic parameters of the function instance being lowered are replaced by (generics
    /// T32): empty outside an instance. Every read of the type table goes through it.
    substitution: crate::types::Substitution,
    /// The values the module params of the frame being lowered take: an array length that reads a param (`[u8; n]`,
    /// §8.1.2) folds over them wherever a type of that frame is read.
    frame_params: Bindings,
    /// The templates the evaluation folded, interned by value: a template is a number, equal templates one (`templates.rs`).
    templates: crate::elab::structure::templates::Templates,
    /// The `impl`s of the design, for the calls through a bound (`dispatch.rs`).
    impls: crate::types::ImplTable<'d>,
}

use std::sync::Arc;

use crate::refusal::forms::ConstantForm;
use crate::pom::{ConstData, DeclKind, ExprId, ExprKind, Resolution, StructClass, TypeId};
use crate::types::{ConstTerm, GenericValue, Length, Ty};

/// The most nested calls an evaluation follows: a call past it is a panic (§1 rule 11, Q5).
const CALL_DEPTH: usize = 1024;

impl<'d, 't> ConstEval<'d, 't> {
    /// Reads the module params of the frame being lowered as `params` from here on; the previous ones come back.
    pub(in crate::elab) fn set_frame_params(&mut self, params: Bindings) -> Bindings {
        std::mem::replace(&mut self.frame_params, params)
    }

    /// The module params of the frame being lowered: the body's keys in its own module, the constants a hoisted placement bound
    /// in its frame (`enter_frame`). A template a callable param holds is read here (`placed_module`).
    pub(in crate::elab) fn frame_params(&self) -> &Bindings {
        &self.frame_params
    }

    /// What the generic parameters of the instance being lowered are replaced by.
    pub(in crate::elab) fn substitution(&self) -> &crate::types::Substitution {
        &self.substitution
    }

    /// Reads the generic parameters as `substitution` replaces them from here on; the previous one comes back.
    pub(in crate::elab) fn set_substitution(&mut self, substitution: crate::types::Substitution) -> crate::types::Substitution {
        std::mem::replace(&mut self.substitution, substitution)
    }

    /// `ty` under the substitution of the instance being lowered, an array length written as an expression over the
    /// const parameters (`[f64; N + 1]`) folded to the number it is for these arguments (§3.8.3); one that does not
    /// fold stays generic, and the obligation of the instantiation reports it.
    pub(super) fn resolved(&self, ty: &Ty) -> Ty {
        self.folded_lengths(&self.substitution.apply(ty))
    }

    /// The value of the integer const expression `expr` of the instance being lowered: its const parameters are
    /// the substitution's arguments. `None` when it does not fold.
    pub(in crate::elab) fn integer(&self, expr: ExprId) -> Option<u128> {
        let values: BTreeMap<DeclId, u128> = self
            .substitution
            .entries()
            .filter_map(|(parameter, value)| match value {
                GenericValue::Const(ConstTerm::Value(count)) => u128::try_from(*count).ok().map(|count| (parameter, count)),
                // a type argument stands for its level count, which `T::WIDTH` in a length reads (§2)
                GenericValue::Type(_) => self.parameter_width(parameter).map(|levels| (parameter, u128::from(levels))),
                _ => None,
            })
            .chain(self.frame_params.counts())
            .collect();
        self.types.fold_with(expr, values)
    }

    /// `ty` with the lengths that are const expressions folded over the const arguments of the substitution.
    pub(super) fn folded_lengths(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Array(element, Length::Generic(ConstTerm::Expr(expr))) => {
                let element = Box::new(self.folded_lengths(element));
                match self.integer(*expr).and_then(|count| u64::try_from(count).ok()) {
                    Some(count) => Ty::Array(element, Length::Fixed(count)),
                    None => Ty::Array(element, Length::Generic(ConstTerm::Expr(*expr))),
                }
            }
            // `[bool; T::WIDTH]` of an instance is the level count of the type `T` is given (§2)
            Ty::Array(element, Length::Generic(ConstTerm::Width(parameter))) => {
                let element = Box::new(self.folded_lengths(element));
                match self.parameter_width(*parameter) {
                    Some(levels) => Ty::Array(element, Length::Fixed(u64::from(levels))),
                    None => Ty::Array(element, Length::Generic(ConstTerm::Width(*parameter))),
                }
            }
            Ty::Array(element, length) => Ty::Array(Box::new(self.folded_lengths(element)), length.clone()),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|item| self.folded_lengths(item)).collect()),
            Ty::Option(inner) => Ty::Option(Box::new(self.folded_lengths(inner))),
            Ty::Fn(params, ret) => Ty::Fn(params.iter().map(|param| self.folded_lengths(param)).collect(), Box::new(self.folded_lengths(ret))),
            other => other.clone(),
        }
    }

    /// The type the checker gave `expr` under the substitution of the instance being lowered.
    pub(in crate::elab) fn typed(&self, expr: ExprId) -> Option<Ty> {
        self.types.ty(expr).map(|ty| self.resolved(ty))
    }

    /// The type of the port or signal `decl` when it is a carrier or an array of them, under the substitution.
    pub(in crate::elab) fn typed_carrier(&self, decl: DeclId) -> Option<Ty> {
        self.types.carrier(decl).map(|ty| self.resolved(ty))
    }

    /// The type of the port or signal `decl` when it is an array of carriers, under the substitution.
    pub(in crate::elab) fn typed_carrier_array(&self, decl: DeclId) -> Option<Ty> {
        self.types.carrier_array(decl).map(|ty| self.resolved(ty))
    }

    /// The type of the port or signal `decl` when it is an array of nodes, under the substitution.
    pub(in crate::elab) fn typed_node_array(&self, decl: DeclId) -> Option<Ty> {
        self.types.node_array(decl).map(|ty| self.resolved(ty))
    }

    /// The type of the binding `binding` under the substitution.
    pub(in crate::elab) fn typed_local(&self, binding: DeclId) -> Option<Ty> {
        self.types.local(binding).map(|ty| self.resolved(ty))
    }

    /// The type written at `ty` as the checker typed it, under no substitution: what names an instance the same way
    /// wherever it is asked (`impl_qualified_name`).
    pub(in crate::elab) fn written_as_declared(&self, ty: TypeId) -> Option<Ty> {
        self.types.written(ty).cloned()
    }

    /// The type the type node `ty` writes under the substitution.
    pub(in crate::elab) fn typed_written(&self, ty: TypeId) -> Option<Ty> {
        self.types.written(ty).map(|ty| self.resolved(ty))
    }

    pub fn new(design: &'d Design, types: &'t Types<'d>) -> Self {
        ConstEval { design, types, consts: BTreeMap::new(), evaluations: 0, locals: BTreeMap::new(), depth: 0, open_lets: Vec::new(), fn_values: BTreeMap::new(), leaving: None, open_constants: Vec::new(), files: IncludedFiles::new(), composites: Vec::new(), wides: Vec::new(), wides_taken: 0, substitution: crate::types::Substitution::new(), frame_params: Bindings::new(), templates: crate::elab::structure::templates::Templates::default(), impls: crate::types::ImplTable::new(design) }
    }

    /// How many `const` items this evaluator has folded, each counted once however many times
    /// it was read.
    pub fn evaluations(&self) -> usize {
        self.evaluations
    }

    /// How many files `include_str!` has opened, one per distinct resolved path (§20 rule 3).
    pub fn files_read(&self) -> usize {
        self.files.reads()
    }

    /// The text of the `const` item `decl` whose initializer is `include_str!("path")`: the file
    /// beside the source that holds the `const`, read the first time (§20 rules 1 to 4). An
    /// initializer of another form is [`ConstFault::Refused`]: the subset has no other text.
    pub fn text(&mut self, decl: DeclId) -> Result<Arc<str>, ConstFault> {
        let data = self.design.decl_data(decl);
        let DeclKind::Const(ConstData { value: Some(expr), .. }) = &data.kind else {
            return Err(ConstFault::NoValue { decl });
        };
        let expr = *expr;
        let ExprKind::IncludeStr(literal) = &self.design.expr_node(expr).kind else {
            return Err(ConstFault::refused(expr, ConstantForm::TextValueOtherThanIncludeStr));
        };
        let Some(path) = literal.str_value() else {
            return Err(ConstFault::refused(expr, ConstantForm::IncludeStrPathWithUndefinedEscape));
        };
        let source = self.design.file_data(data.file).path.clone();
        self.files.text(source.as_deref(), &path).map_err(|fault| ConstFault::File { expr, fault })
    }

    /// The value of a `const` item, folded the first time it is asked for.
    pub fn constant(&mut self, decl: DeclId) -> Result<Constant, ConstFault> {
        let value = self.fold_constant(decl)?;
        if self.composite_of(value).is_some() {
            return Err(match &self.design.decl_data(decl).kind {
                DeclKind::Const(ConstData { value: Some(expr), .. }) => ConstFault::refused(*expr, ConstantForm::CompositeValueWhereScalarIsRequired),
                _ => ConstFault::NoValue { decl },
            });
        }
        Ok(value)
    }

    /// What a `const` item folds to, a composite included, folded the first time it is asked for.
    pub(super) fn fold_constant(&mut self, decl: DeclId) -> Result<Constant, ConstFault> {
        if let Some(value) = self.consts.get(&decl) {
            return Ok(*value);
        }
        let DeclKind::Const(ConstData { value: Some(expr), .. }) = &self.design.decl_data(decl).kind else {
            return Err(ConstFault::NoValue { decl });
        };
        let expr = *expr;
        let outer = std::mem::take(&mut self.locals);
        let outer_functions = std::mem::take(&mut self.fn_values);
        self.open_constants.push(decl);
        let value = self.fold(expr, &Bindings::new());
        self.open_constants.pop();
        self.locals = outer;
        self.fn_values = outer_functions;
        let value = value.map_err(|fault| match fault {
            ConstFault::Dynamic { expr, what } => ConstFault::NotConstant { expr, constant: decl, what },
            other => other,
        })?;
        self.evaluations += 1;
        self.consts.insert(decl, value);
        Ok(value)
    }

    /// `value` as a scalar: a composite is refused, because its handle names an entry of this
    /// evaluator and means nothing outside it (`tree` gives a composite out).
    pub(super) fn scalar(&self, expr: ExprId, value: Constant) -> Result<Constant, ConstFault> {
        match self.composite_of(value) {
            Some(_) => Err(ConstFault::refused(expr, ConstantForm::CompositeValueWhereScalarIsRequired)),
            None => Ok(value),
        }
    }

    /// The value of `expr` with the params of `bindings` bound.
    pub fn value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.fold(expr, bindings)?;
        self.scalar(expr, value)
    }

    /// What `expr` folds to, a composite included: the evaluator's own recursion.
    pub(super) fn fold(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let kind = self.design.expr_node(expr).kind.clone();
        match &kind {
            ExprKind::Lit(lit) => self.literal(expr, &lit.kind),
            ExprKind::Paren(inner) => self.fold(*inner, bindings),
            ExprKind::Path(_) => match self.option_value(expr, bindings) {
                Some(none) => none,
                None => self.path(expr, bindings),
            },
            ExprKind::Tuple(items) => self.tuple_value(items, bindings),
            ExprKind::Array(items) => self.array_value(items, bindings),
            ExprKind::ArrayRepeat { elem, len } => self.repeat_value(expr, *elem, *len, bindings),
            ExprKind::Struct { class: StructClass::Template(module), fields, .. } => self.template_literal(expr, *module, fields, bindings),
            ExprKind::Struct { path, fields, .. } => self.struct_value(expr, *path, fields, bindings),
            ExprKind::Field { expr: recv, name, .. } => self.field_value(expr, *recv, &name.text, bindings),
            ExprKind::Index { expr: base, index } => self.index_value(expr, *base, *index, bindings),
            ExprKind::Range { lo, hi, inclusive } => self.range_value(*lo, *hi, *inclusive, bindings),
            ExprKind::Unary { op, expr: operand } => self.unary(expr, *op, *operand, bindings),
            ExprKind::Binary { op, lhs, rhs } => self.binary(expr, *op, *lhs, *rhs, bindings),
            // the forms that may have no value, or that leave (`control.rs`)
            ExprKind::If { .. } | ExprKind::Block(_) | ExprKind::While { .. } | ExprKind::Loop { .. } | ExprKind::For { .. } | ExprKind::Assign { .. } | ExprKind::Match { .. } | ExprKind::Break(_) | ExprKind::Continue | ExprKind::Return(_) => self.exec_value(expr, bindings),
            ExprKind::Call { .. } if let Some(some) = self.option_value(expr, bindings) => some,
            ExprKind::Call { callee, args, .. } if let Some(owner) = self.constructed(*callee) => self.constructor_value(expr, owner, &args.iter().map(|a| a.value).collect::<Vec<_>>(), bindings),
            ExprKind::Call { .. } if let Some(ty) = self.types.default_of(expr).cloned() => self.default_value(expr, &ty),
            ExprKind::Call { args, .. } => self.call(expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), bindings),
            ExprKind::MethodCall { recv, args, .. } => self.method(expr, *recv, &args.iter().map(|a| a.value).collect::<Vec<_>>(), bindings),
            ExprKind::Cast { expr: operand, .. } => self.cast(expr, *operand, bindings),
            _ => Err(ConstFault::refused(expr, ConstantForm::FormInConstExpression)),
        }
    }

    /// The integer type the checker gave `expr`, as the width and sign the evaluator wraps to.
    pub(super) fn width(&self, expr: ExprId) -> Option<Width> {
        self.typed(expr).and_then(|ty| Width::of(&ty))
    }

    /// The level count of `T::WIDTH` inside an instance: `T` is a type parameter the substitution replaces by a
    /// type with a `Repr` (§2). `None` when `expr` is no such path or `T` has no argument here.
    pub(in crate::elab) fn parameter_levels(&self, expr: ExprId) -> Option<u32> {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { return None };
        let segments = &self.design.path_node(*path).segments;
        let [owner, member] = segments.as_slice() else { return None };
        if member.name.text != "WIDTH" {
            return None;
        }
        let Resolution::Decl(parameter) = owner.res else { return None };
        self.parameter_width(parameter)
    }

    /// The level count of the type the substitution gives the type parameter `parameter`.
    pub(in crate::elab) fn parameter_width(&self, parameter: DeclId) -> Option<u32> {
        match self.substitution.get(parameter)? {
            GenericValue::Type(ty) => self.types.repr_levels(ty).or_else(|| ty.levels(self.design)),
            _ => None,
        }
    }

}

/// The width and sign of an integer type: what a result wraps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Width {
    pub(super) bits: u32,
    pub(super) signed: bool,
}

impl Width {
    /// The width of an integer type of at most 64 bits; a wider one is a `DynamicInteger` (`numbers.rs`).
    pub(super) fn of(ty: &Ty) -> Option<Width> {
        match *ty {
            Ty::UInt(n) if n <= 64 => Some(Width { bits: n, signed: false }),
            Ty::Int(n) if n <= 64 => Some(Width { bits: n, signed: true }),
            _ => None,
        }
    }

    /// `v` modulo `2^bits` (§1 rule 6), held as the compiled code holds it: below `2^bits`
    /// unsigned, sign-extended signed.
    pub(super) fn wrap(self, v: i128) -> Constant {
        let modulus = 1i128 << self.bits;
        let low = v.rem_euclid(modulus);
        Constant::Int(if self.signed && low >= modulus / 2 { low - modulus } else { low })
    }

    /// The value of `v` modulo `2^bits` as an `i128`, for a running product.
    pub(super) fn wrapped(self, v: i128) -> i128 {
        match self.wrap(v) {
            Constant::Int(w) => w,
            _ => v,
        }
    }

    /// `v` clamped to the range of this type (`saturating_*`).
    pub(super) fn clamp(self, v: i128) -> i128 {
        let (min, max) = if self.signed { (-(1i128 << (self.bits - 1)), (1i128 << (self.bits - 1)) - 1) } else { (0, (1i128 << self.bits) - 1) };
        v.clamp(min, max)
    }

    /// `x as` this type (G4): truncated toward zero, saturated to the type's range, NaN as 0.
    pub(super) fn saturate(self, x: f64) -> Constant {
        if x.is_nan() {
            return Constant::Int(0);
        }
        let (min, max) = if self.signed { (-(1i128 << (self.bits - 1)), (1i128 << (self.bits - 1)) - 1) } else { (0, (1i128 << self.bits) - 1) };
        // `as i128` truncates toward zero and saturates; the range of 64 bits fits in it.
        Constant::Int((x as i128).clamp(min, max))
    }
}
