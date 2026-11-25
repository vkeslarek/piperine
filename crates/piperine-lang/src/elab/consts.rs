//! `ConstEval`: a const expression of the subset evaluated to a [`Constant`] at elaboration.
//!
//! A second small evaluator beside the interpreter's, on purpose: this one reads POM
//! expressions, has no duals and no solver. `f64` is IEEE 754 throughout (`1.0 / 0.0` is `inf`,
//! never an error). Integers follow the rules the compiled code follows (`piperine-transpile`
//! `rt::int`): arithmetic wraps modulo `2^N` (§1 rule 6), `as` from `f64` saturates and maps NaN
//! to 0 (ROADMAP G4), a shift by `N` or more gives 0, or the sign for a signed `>>` (G5). An
//! integer division or remainder by zero, a shift by a negative amount and a `clamp` with its
//! bounds crossed are panics (§1 rule 11): `E0104` at elaboration.
//!
//! A value either has a constant at elaboration or it does not: every value is a value (§3.3.1),
//! so there is no third state. Where a read finds none (a probe, a carrier, an environment
//! function, a function-valued argument read as a number) the fault says so at that read
//! ([`ConstFault::Dynamic`], `E0321` at the position that needed a constant).
//!
//! A `const` item is evaluated once and remembered. A param is read through the caller's
//! [`Bindings`]; a read with no binding, an environment function, a probe or a node is *not
//! constant*, reported to the caller as such so that it can pick the code of its position
//! (`E0321`, or `P0001` for a param of the parent).
//!
//! The forms of phase A (mutation, loops, exits, patterns, closures, recursion) are `control.rs`:
//! a `const`, a `const fn`, a param default and a `where` evaluate them with their run-time value.
//! The aggregates of phase B (tuples, arrays, structs, enum variants with fields, `Option`s, ranges
//! and iterators, with their patterns, comparisons, field and element assignment and the rows of
//! Appendix H) are `composite.rs`; the public `folded`, `value` and `constant` give a scalar only, a
//! composite is given out as a [`ConstTree`] (`tree`, `constant_tree`). A module-body `let` written in generation folds
//! per iteration, and to the array of its iterations' values outside its loops (`generated.rs`).

mod composite;
mod control;
mod dispatch;
mod floats;
mod generated;
mod imported;
mod integers;
mod numbers;
mod parsed;
mod templates;

pub(in crate::elab) use integers::DynamicInteger;

use std::collections::BTreeMap;
use std::sync::Arc;

use piperine_ir::{Constant, Real};

use super::compound::EnumLayout;
use super::files::{FileFault, IncludedFiles};
use crate::msl::lexer;
use crate::pom::{BinOp, ConstData, Design, DeclId, DeclKind, ExprId, ExprKind, LitKind, PatKind, Resolution, StructClass, TypeId, UnOp};
use crate::types::{Callee, ConstTerm, GenericValue, Intrinsic, Length, Method, Nominal, Ty, Types};
pub use composite::{CompositeKind, ConstTree};
use composite::Composite;
use control::{Argument, FnValue};

/// The most nested calls an evaluation follows: a call past it is a panic (§1 rule 11, Q5).
const CALL_DEPTH: usize = 1024;

/// The bound value of each param declaration an evaluation may read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings {
    values: BTreeMap<DeclId, Constant>,
    /// The value of each composite param, taken apart: a `Constant::Aggregate` is an entry of the
    /// IR's arena, which this evaluator cannot read, so it reads the tree.
    trees: BTreeMap<DeclId, ConstTree>,
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
    pub(super) fn counts(&self) -> BTreeMap<DeclId, u128> {
        self.values.iter().filter_map(|(decl, value)| match value {
            Constant::Int(count) => u128::try_from(*count).ok().map(|count| (*decl, count)),
            _ => None,
        }).collect()
    }

    /// These bindings with those of `other` added: `other` wins where both bind a name.
    pub(super) fn overlaid(&self, other: &Bindings) -> Bindings {
        let mut merged = self.clone();
        merged.values.extend(other.values.iter().map(|(decl, value)| (*decl, *value)));
        merged.trees.extend(other.trees.iter().map(|(decl, tree)| (*decl, tree.clone())));
        merged
    }

    /// Forgets the value of `decl`; the value it had, if any.
    pub(super) fn unbind(&mut self, decl: DeclId) -> Option<Constant> {
        self.trees.remove(&decl);
        self.values.remove(&decl)
    }
}

/// Why an expression has no constant value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstFault {
    /// A read of a param with no binding: the caller says whether it is a param of the parent
    /// (`P0001`) or a param no value was given to (`E0321`).
    UnboundParam { expr: ExprId, param: DeclId },
    /// Something only a run can give: a probe, an environment function, a node, a module.
    Dynamic { expr: ExprId, what: String },
    /// A form of the subset the evaluator does not fold.
    Unsupported { expr: ExprId, what: String },
    /// A `const` item with no value (a declaration in a trait): nothing to fold.
    NoValue { decl: DeclId },
    /// A panic of the evaluation (§1 rule 11): an integer division or remainder by zero, a shift
    /// by a negative amount, a `clamp` whose bounds are crossed. `E0104` at elaboration.
    Panic { expr: ExprId, what: String },
    /// The initializer of the `const` item `constant` reads, at `expr`, something only a run gives
    /// (`what`): the `const` is not a const expression (§3.9.2, `E0188`).
    NotConstant { expr: ExprId, constant: DeclId, what: String },
    /// A `const` read, at `expr`, while its own initializer is being folded: it depends on itself
    /// (§3.9.2, `E0188`).
    Cyclic { expr: ExprId, constant: DeclId },
    /// The file an `include_str!` at `expr` names has no text (§20 errors: `E0612`, `E0613`).
    File { expr: ExprId, fault: FileFault },
    /// The text `parse_table` or `parse_hex` reads does not fit the `const`'s type (`E0614`) or is not in the
    /// format (`E0706`, §22.6.4): `message` names the source and the row.
    Table { expr: ExprId, code: &'static str, message: String },
    /// A read of a `let` written in a generation arm the evaluation does not take (§8.2.24, `E0342`): `message` names it.
    Ungenerated { expr: ExprId, message: String },
    /// A `break`, `continue` or `return` on its way to the loop or function it leaves; the value it
    /// carries waits in the evaluator. Never reaches a caller of [`ConstEval::value`].
    Leaves { expr: ExprId, exit: Exit },
}

/// Which exit a [`ConstFault::Leaves`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// `break`, with or without a value.
    Break,
    /// `continue`.
    Continue,
    /// `return`, with or without a value.
    Return,
}

impl ConstFault {
    /// The expression the fault sits on; none for a `const` item without a value.
    pub fn expr(&self) -> Option<ExprId> {
        match self {
            ConstFault::UnboundParam { expr, .. } | ConstFault::Dynamic { expr, .. } | ConstFault::Unsupported { expr, .. } | ConstFault::Panic { expr, .. } | ConstFault::Cyclic { expr, .. } | ConstFault::NotConstant { expr, .. } | ConstFault::File { expr, .. } | ConstFault::Table { expr, .. } | ConstFault::Ungenerated { expr, .. } | ConstFault::Leaves { expr, .. } => Some(*expr),
            ConstFault::NoValue { .. } => None,
        }
    }
}

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
    templates: super::templates::Templates,
    /// The `impl`s of the design, for the calls through a bound (`dispatch.rs`).
    impls: crate::types::ImplTable<'d>,
}

impl<'d, 't> ConstEval<'d, 't> {
    /// Reads the module params of the frame being lowered as `params` from here on; the previous ones come back.
    pub(super) fn set_frame_params(&mut self, params: Bindings) -> Bindings {
        std::mem::replace(&mut self.frame_params, params)
    }

    /// The module params of the frame being lowered: the body's keys in its own module, the constants a hoisted placement bound
    /// in its frame (`enter_frame`). A template a callable param holds is read here (`placed_module`).
    pub(super) fn frame_params(&self) -> &Bindings {
        &self.frame_params
    }

    /// What the generic parameters of the instance being lowered are replaced by.
    pub(super) fn substitution(&self) -> &crate::types::Substitution {
        &self.substitution
    }

    /// Reads the generic parameters as `substitution` replaces them from here on; the previous one comes back.
    pub(super) fn set_substitution(&mut self, substitution: crate::types::Substitution) -> crate::types::Substitution {
        std::mem::replace(&mut self.substitution, substitution)
    }

    /// `ty` under the substitution of the instance being lowered, an array length written as an expression over the
    /// const parameters (`[f64; N + 1]`) folded to the number it is for these arguments (§3.8.3); one that does not
    /// fold stays generic, and the obligation of the instantiation reports it.
    fn resolved(&self, ty: &Ty) -> Ty {
        self.folded_lengths(&self.substitution.apply(ty))
    }

    /// The value of the integer const expression `expr` of the instance being lowered: its const parameters are
    /// the substitution's arguments. `None` when it does not fold.
    pub(super) fn integer(&self, expr: ExprId) -> Option<u128> {
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
    fn folded_lengths(&self, ty: &Ty) -> Ty {
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
    pub(super) fn typed(&self, expr: ExprId) -> Option<Ty> {
        self.types.ty(expr).map(|ty| self.resolved(ty))
    }

    /// The type of the port or signal `decl` when it is a carrier or an array of them, under the substitution.
    pub(super) fn typed_carrier(&self, decl: DeclId) -> Option<Ty> {
        self.types.carrier(decl).map(|ty| self.resolved(ty))
    }

    /// The type of the port or signal `decl` when it is an array of carriers, under the substitution.
    pub(super) fn typed_carrier_array(&self, decl: DeclId) -> Option<Ty> {
        self.types.carrier_array(decl).map(|ty| self.resolved(ty))
    }

    /// The type of the port or signal `decl` when it is an array of nodes, under the substitution.
    pub(super) fn typed_node_array(&self, decl: DeclId) -> Option<Ty> {
        self.types.node_array(decl).map(|ty| self.resolved(ty))
    }

    /// The type of the binding `binding` under the substitution.
    pub(super) fn typed_local(&self, binding: DeclId) -> Option<Ty> {
        self.types.local(binding).map(|ty| self.resolved(ty))
    }

    /// The type written at `ty` as the checker typed it, under no substitution: what names an instance the same way
    /// wherever it is asked (`impl_qualified_name`).
    pub(super) fn written_as_declared(&self, ty: TypeId) -> Option<Ty> {
        self.types.written(ty).cloned()
    }

    /// The type the type node `ty` writes under the substitution.
    pub(super) fn typed_written(&self, ty: TypeId) -> Option<Ty> {
        self.types.written(ty).map(|ty| self.resolved(ty))
    }

    pub fn new(design: &'d Design, types: &'t Types<'d>) -> Self {
        ConstEval { design, types, consts: BTreeMap::new(), evaluations: 0, locals: BTreeMap::new(), depth: 0, open_lets: Vec::new(), fn_values: BTreeMap::new(), leaving: None, open_constants: Vec::new(), files: IncludedFiles::new(), composites: Vec::new(), wides: Vec::new(), wides_taken: 0, substitution: crate::types::Substitution::new(), frame_params: Bindings::new(), templates: super::templates::Templates::default(), impls: crate::types::ImplTable::new(design) }
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
    /// initializer of another form is [`ConstFault::Unsupported`]: the subset has no other text.
    pub fn text(&mut self, decl: DeclId) -> Result<Arc<str>, ConstFault> {
        let data = self.design.decl_data(decl);
        let DeclKind::Const(ConstData { value: Some(expr), .. }) = &data.kind else {
            return Err(ConstFault::NoValue { decl });
        };
        let expr = *expr;
        let ExprKind::IncludeStr(literal) = &self.design.expr_node(expr).kind else {
            return Err(ConstFault::Unsupported { expr, what: "a text value other than `include_str!`".to_string() });
        };
        let Some(path) = literal.str_value() else {
            return Err(ConstFault::Unsupported { expr, what: "an `include_str!` path with an undefined escape".to_string() });
        };
        let source = self.design.file_data(data.file).path.clone();
        self.files.text(source.as_deref(), &path).map_err(|fault| ConstFault::File { expr, fault })
    }

    /// The value of a `const` item, folded the first time it is asked for.
    pub fn constant(&mut self, decl: DeclId) -> Result<Constant, ConstFault> {
        let value = self.fold_constant(decl)?;
        if self.composite_of(value).is_some() {
            return Err(match &self.design.decl_data(decl).kind {
                DeclKind::Const(ConstData { value: Some(expr), .. }) => ConstFault::Unsupported { expr: *expr, what: "a composite value where a scalar is required".to_string() },
                _ => ConstFault::NoValue { decl },
            });
        }
        Ok(value)
    }

    /// What a `const` item folds to, a composite included, folded the first time it is asked for.
    fn fold_constant(&mut self, decl: DeclId) -> Result<Constant, ConstFault> {
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
    fn scalar(&self, expr: ExprId, value: Constant) -> Result<Constant, ConstFault> {
        match self.composite_of(value) {
            Some(_) => Err(ConstFault::Unsupported { expr, what: "a composite value where a scalar is required".to_string() }),
            None => Ok(value),
        }
    }

    /// The value of `expr` with the params of `bindings` bound.
    pub fn value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.fold(expr, bindings)?;
        self.scalar(expr, value)
    }

    /// What `expr` folds to, a composite included: the evaluator's own recursion.
    fn fold(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
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
            _ => Err(ConstFault::Unsupported { expr, what: "this form in a const expression".to_string() }),
        }
    }

    /// The `f64` of a value, or the fault of an ill-typed read (the type checker rules it out).
    fn real(expr: ExprId, value: Constant) -> Result<f64, ConstFault> {
        match value {
            Constant::Real(r) => Ok(r.get()),
            Constant::Bool(_) => Err(ConstFault::Unsupported { expr, what: "a `bool` where an `f64` is required".to_string() }),
            Constant::Int(_) => Err(ConstFault::Unsupported { expr, what: "an integer where an `f64` is required".to_string() }),
            Constant::Aggregate(_) => Err(ConstFault::Unsupported { expr, what: "a composite value where an `f64` is required".to_string() }),
            Constant::FloatBits(_) | Constant::Wide(_) => Err(ConstFault::Unsupported { expr, what: "a number-width value where an `f64` is required".to_string() }),
        }
    }

    fn boolean(expr: ExprId, value: Constant) -> Result<bool, ConstFault> {
        match value {
            Constant::Bool(b) => Ok(b),
            Constant::Real(_) => Err(ConstFault::Unsupported { expr, what: "an `f64` where a `bool` is required".to_string() }),
            Constant::Int(_) => Err(ConstFault::Unsupported { expr, what: "an integer where a `bool` is required".to_string() }),
            Constant::Aggregate(_) => Err(ConstFault::Unsupported { expr, what: "a composite value where a `bool` is required".to_string() }),
            Constant::FloatBits(_) | Constant::Wide(_) => Err(ConstFault::Unsupported { expr, what: "a number-width value where a `bool` is required".to_string() }),
        }
    }

    /// The integer type the checker gave `expr`, as the width and sign the evaluator wraps to.
    fn width(&self, expr: ExprId) -> Option<Width> {
        self.typed(expr).and_then(|ty| Width::of(&ty))
    }

    /// A literal as the constant it is: a number of its type's width, a bool, a real.
    fn literal(&mut self, expr: ExprId, kind: &LitKind) -> Result<Constant, ConstFault> {
        let known = |value: Constant| Ok(value);
        if let Some(number) = self.number_literal(expr, kind) {
            return number;
        }
        match kind {
            LitKind::Float { value, .. } => known(Constant::Real(Real::new(*value))),
            LitKind::Bool(b) => known(Constant::Bool(*b)),
            LitKind::Int(text) if text.ends_with("f64") => {
                let digits: String = text.trim_end_matches("f64").chars().filter(|c| *c != '_').collect();
                match digits.parse::<f64>() {
                    Ok(value) => known(Constant::Real(Real::new(value))),
                    Err(_) => Err(ConstFault::Unsupported { expr, what: "an integer literal".to_string() }),
                }
            }
            LitKind::Int(text) => match (self.width(expr), lexer::int_value(text)) {
                // An `i8` literal `128` is the operand of a unary `-` (the checker allowed it): it
                // wraps to -128 here and the minus wraps it back.
                (Some(width), Some(value)) => known(width.wrap(value as i128)),
                _ => Err(ConstFault::Unsupported { expr, what: "an integer literal".to_string() }),
            },
            LitKind::Str { .. } => Err(ConstFault::Unsupported { expr, what: "this literal".to_string() }),
        }
    }

    /// The level count of `T::WIDTH` inside an instance: `T` is a type parameter the substitution replaces by a
    /// type with a `Repr` (§2). `None` when `expr` is no such path or `T` has no argument here.
    pub(super) fn parameter_levels(&self, expr: ExprId) -> Option<u32> {
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
    pub(super) fn parameter_width(&self, parameter: DeclId) -> Option<u32> {
        match self.substitution.get(parameter)? {
            GenericValue::Type(ty) => self.types.repr_levels(ty).or_else(|| ty.levels(self.design)),
            _ => None,
        }
    }

    /// A name as the constant it is bound to (a `const`, a param, a binding of the call), or the
    /// level count of a `T::WIDTH`; a built-in name is dynamic.
    fn path(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        // `T::WIDTH`: the level count of `T`, which the checker recorded (§2, §22.1)
        if let Some(levels) = self.types.width(expr).or_else(|| self.parameter_levels(expr)) {
            return Ok(Constant::Int(i128::from(levels)));
        }
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else {
            return Err(ConstFault::Unsupported { expr, what: "a name".to_string() });
        };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            return Err(ConstFault::Dynamic { expr, what: "a built-in name".to_string() });
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) if self.open_constants.contains(&decl) => Err(ConstFault::Cyclic { expr, constant: decl }),
            DeclKind::Const(_) => self.fold_constant(decl).map_err(|fault| match fault {
                // A trait's `const` without a value is outside the subset; anywhere else it is `E0188`.
                ConstFault::NoValue { decl } if self.is_trait_item(decl) => ConstFault::Unsupported { expr, what: "a `const` without a value".to_string() },
                other => other,
            }),
            // a const parameter of the generic item being instantiated is bound to its argument (generics T33)
            // a const parameter of the instance being evaluated is the argument the call gave it (§3.8.3)
            DeclKind::GenericParam(_) => match (bindings.get(decl), self.substitution.get(decl)) {
                (Some(bound), _) => Ok(bound),
                (None, Some(GenericValue::Const(ConstTerm::Value(count)))) => Ok(Constant::Int(*count)),
                (None, Some(GenericValue::Const(ConstTerm::Bool(flag)))) => Ok(Constant::Bool(*flag)),
                _ => Err(ConstFault::UnboundParam { expr, param: decl }),
            },
            DeclKind::Param(_) => match bindings.get(decl) {
                // a composite param is an IR constant, which this evaluator's composites are not (`composite.rs`)
                Some(Constant::Aggregate(_)) => match bindings.tree(decl).cloned() {
                    Some(tree) => Ok(self.adopt(&tree)),
                    None => Err(ConstFault::Unsupported { expr, what: "a composite param in a const expression".to_string() }),
                },
                Some(value) => Ok(value),
                None => Err(ConstFault::UnboundParam { expr, param: decl }),
            },
            // a module's bare name is the template `Name {}`, every param at its default (§8.4.1 amendment)
            DeclKind::Module(_) => self.template_literal(expr, decl, &[], bindings),
            // A unit struct, and a unit variant of an enum with fields, are held with their parts (`composite.rs`).
            DeclKind::Struct(_) | DeclKind::Variant(_) if let Some(value) = self.unit_value(decl) => Ok(value),
            // A unit variant is its tag (§2 rule 5).
            DeclKind::Variant(_) => EnumLayout::variant_tag(self.design, decl).map(|tag| Constant::Int(tag)).ok_or(ConstFault::Unsupported { expr, what: "a variant outside its enum".to_string() }),
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match (self.locals.get(&decl).copied(), bindings.get(decl)) {
                (Some(value), _) => Ok(value),
                // the loop variable of an unrolled analog loop, or the parameter of an unrolled
                // recursion, bound to its constant for this iteration or call (`unroll.rs`)
                (None, Some(Constant::Aggregate(_))) => Err(ConstFault::Unsupported { expr, what: "a composite value bound from outside the evaluation".to_string() }),
                (None, Some(value)) => Ok(value),
                (None, None) => self.body_let(expr, decl, bindings),
            },
            // A value input the caller binds to a constant (`structural.rs`, a selector through an input).
            DeclKind::Port(_) if let Some(Constant::Aggregate(_)) = bindings.get(decl) => Err(ConstFault::Unsupported { expr, what: "a composite value input in a const expression".to_string() }),
            DeclKind::Port(_) if let Some(value) = bindings.get(decl) => Ok(value),
            DeclKind::Port(_) | DeclKind::Signal(_) if self.types.carrier(decl).is_some() => Err(ConstFault::Dynamic { expr, what: "a carrier".to_string() }),
            DeclKind::Port(_) | DeclKind::Signal(_) => Err(ConstFault::Dynamic { expr, what: "a node".to_string() }),
            _ => Err(ConstFault::Unsupported { expr, what: "this name in a const expression".to_string() }),
        }
    }

    /// Whether `decl` is declared in a `trait` (the one place a `const` has no value, §3.9.2).
    pub fn is_trait_item(&self, decl: DeclId) -> bool {
        self.design.decl_data(decl).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Trait(_)))
    }

    /// The value of the module-body `let` name `decl` read at `expr`: its value expression evaluated
    /// over the same params (§8.1.19), so a param value may name a `let` that computes it. A `let`
    /// that reads a carrier, a probe, the environment or the result of an instance has no constant
    /// value: the fault names the `let`. A `let` that reads itself is one too.
    fn body_let(&mut self, expr: ExprId, decl: DeclId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let Some(value) = Self::body_let_value(self.design, decl) else {
            return Err(ConstFault::Dynamic { expr, what: "a name with no value yet".to_string() });
        };
        let name = self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if self.open_lets.contains(&decl) {
            return Err(ConstFault::Dynamic { expr, what: format!("the `let` `{name}`, which reads itself") });
        }
        self.open_lets.push(decl);
        let outer = std::mem::take(&mut self.locals);
        // a `let` written in generation: the iteration's value, or the array of them outside its loops (`generated.rs`)
        let result = self.generated_let(expr, decl, value, bindings);
        self.locals = outer;
        self.open_lets.pop();
        result.map_err(|fault| match fault {
            ConstFault::Dynamic { what, .. } => ConstFault::Dynamic { expr, what: format!("the `let` `{name}`, which reads {what}") },
            other => other,
        })
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier pattern,
    /// the matching component of a tuple pattern over a tuple expression.
    pub(super) fn body_let_value(design: &Design, binding: DeclId) -> Option<ExprId> {
        let data = design.decl_data(binding);
        let DeclKind::Binding(b) = &data.kind else { return None };
        if b.origin != crate::pom::BindingOrigin::BodyLet {
            return None;
        }
        let DeclKind::Let(l) = &design.decl_data(data.parent?).kind else { return None };
        match &design.pat_node(l.pat).kind {
            PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding => Some(l.value),
            PatKind::Tuple(items) => {
                let at = items.iter().position(|p| matches!(&design.pat_node(*p).kind, PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding))?;
                let mut whole = l.value;
                while let ExprKind::Paren(inner) = &design.expr_node(whole).kind {
                    whole = *inner;
                }
                match &design.expr_node(whole).kind {
                    ExprKind::Tuple(values) if values.len() == items.len() => Some(values[at]),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// A unary operator over `operand`, wrapped to the width of an integer result.
    fn unary(&mut self, expr: ExprId, op: UnOp, operand: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.fold(operand, bindings)?;
        if let Some(number) = self.number_unary(expr, op, operand, value) {
            return number;
        }
        if let (Constant::Int(v), Some(width)) = (value, self.width(expr)) {
            return match op {
                UnOp::Neg => Ok(width.wrap(v.wrapping_neg())),
                UnOp::Not => Ok(width.wrap(!v)),
            };
        }
        match op {
            UnOp::Neg => Ok(Constant::Real(Real::new(-Self::real(operand, value)?))),
            UnOp::Not => Ok(Constant::Bool(!Self::boolean(operand, value)?)),
        }
    }

    /// A binary operator over `lhs` and `rhs`: `&&` and `||` stop at a left operand that decides.
    fn binary(&mut self, expr: ExprId, op: BinOp, lhs: ExprId, rhs: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let left = self.fold(lhs, bindings)?;
        // `&&` and `||` short-circuit on the left operand, as in the source: the right side is not read.
        match op {
            BinOp::And if !Self::boolean(lhs, left)? => return Ok(Constant::Bool(false)),
            BinOp::Or if Self::boolean(lhs, left)? => return Ok(Constant::Bool(true)),
            _ => {}
        }
        let right = self.fold(rhs, bindings)?;
        if self.composite_of(left).is_some() || self.composite_of(right).is_some() {
            return self.composite_binary(expr, op, left, right);
        }
        self.known_binary(expr, op, (lhs, left), (rhs, right))
    }

    /// An operator over two operands, `(expression, value)` each.
    fn known_binary(&mut self, expr: ExprId, op: BinOp, (lhs, left): (ExprId, Constant), (rhs, right): (ExprId, Constant)) -> Result<Constant, ConstFault> {
        if let Some(number) = self.number_binary(expr, op, (lhs, left), (rhs, right)) {
            return number;
        }
        if let (Constant::Int(a), Constant::Int(b)) = (left, right) {
            return self.integer_binary(expr, op, a, b);
        }
        let real = |value: f64| Ok(Constant::Real(Real::new(value)));
        match op {
            BinOp::And | BinOp::Or => Ok(Constant::Bool(Self::boolean(rhs, right)?)),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                let (a, b) = (Self::boolean(lhs, left)?, Self::boolean(rhs, right)?);
                Ok(Constant::Bool(match op {
                    BinOp::BitAnd => a & b,
                    BinOp::BitOr => a | b,
                    _ => a ^ b,
                }))
            }
            BinOp::Eq | BinOp::Ne => {
                let equal = match (left, right) {
                    (Constant::Real(a), Constant::Real(b)) => a.get() == b.get(),
                    (Constant::Bool(a), Constant::Bool(b)) => a == b,
                    _ => return Err(ConstFault::Unsupported { expr, what: "a comparison of an `f64` and a `bool`".to_string() }),
                };
                Ok(Constant::Bool(equal == (op == BinOp::Eq)))
            }
            _ => {
                let (a, b) = (Self::real(lhs, left)?, Self::real(rhs, right)?);
                match op {
                    BinOp::Add => real(a + b),
                    BinOp::Sub => real(a - b),
                    BinOp::Mul => real(a * b),
                    BinOp::Div => real(a / b),
                    BinOp::Lt => Ok(Constant::Bool(a < b)),
                    BinOp::Le => Ok(Constant::Bool(a <= b)),
                    BinOp::Gt => Ok(Constant::Bool(a > b)),
                    BinOp::Ge => Ok(Constant::Bool(a >= b)),
                    _ => Err(ConstFault::Unsupported { expr, what: "this operator".to_string() }),
                }
            }
        }
    }

    /// An operator over two integers. The operands have the result's type, a shift amount any
    /// integer type; the result wraps to the type the checker gave `expr`.
    fn integer_binary(&self, expr: ExprId, op: BinOp, a: i128, b: i128) -> Result<Constant, ConstFault> {
        let compare = |holds: bool| Ok(Constant::Bool(holds));
        match op {
            BinOp::Lt => return compare(a < b),
            BinOp::Le => return compare(a <= b),
            BinOp::Gt => return compare(a > b),
            BinOp::Ge => return compare(a >= b),
            BinOp::Eq => return compare(a == b),
            BinOp::Ne => return compare(a != b),
            _ => {}
        }
        let Some(width) = self.width(expr) else {
            return Err(ConstFault::Unsupported { expr, what: "an integer operator outside an integer type".to_string() });
        };
        let panic = |what: &str| Err(ConstFault::Panic { expr, what: what.to_string() });
        let value = match op {
            BinOp::Add => a.wrapping_add(b),
            BinOp::Sub => a.wrapping_sub(b),
            BinOp::Mul => a.wrapping_mul(b),
            BinOp::Div if b == 0 => return panic("an integer division by zero"),
            BinOp::Rem if b == 0 => return panic("an integer remainder by zero"),
            BinOp::Div => a.wrapping_div(b),
            BinOp::Rem => a.wrapping_rem(b),
            BinOp::BitAnd => a & b,
            BinOp::BitOr => a | b,
            BinOp::BitXor => a ^ b,
            BinOp::Shl | BinOp::Shr if b < 0 => return panic("a shift by a negative amount"),
            // G5: a shift by the width or more is the mathematical result truncated to the width.
            BinOp::Shl if b >= i128::from(width.bits) => 0,
            BinOp::Shr if b >= i128::from(width.bits) => {
                if a < 0 {
                    -1
                } else {
                    0
                }
            }
            BinOp::Shl => a << b,
            // `a` is held sign-extended, so `>>` is arithmetic for a signed type and logical for
            // an unsigned one (whose value is never negative).
            BinOp::Shr => a >> b,
            _ => return Err(ConstFault::Unsupported { expr, what: "this operator on integers".to_string() }),
        };
        Ok(width.wrap(value))
    }

    /// `operand as T`: among the integer types by truncation and sign extension (§1 rule 1),
    /// from `f64` saturating with NaN as 0 (G4), from `bool` as 0 or 1; to `f64` exactly as the
    /// compiled code converts.
    fn cast(&mut self, expr: ExprId, operand: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.fold(operand, bindings)?;
        // An enum value is folded to its variant's position (held with its parts when a variant has fields); `as` reads its discriminant (§1.3.7).
        let value = match value {
            Constant::Aggregate(_) => match self.composite_of(value).map(|c| c.kind) {
                Some(CompositeKind::Variant(position)) => Constant::Int(i128::from(position)),
                _ => return Err(ConstFault::Unsupported { expr, what: "a cast of a composite value".to_string() }),
            },
            other => other,
        };
        let value = match (self.typed(operand), value) {
            (Some(Ty::Enum(Nominal { decl, .. })), Constant::Int(position)) => match self.types.enum_tag(decl).and_then(|t| t.discriminants.get(position as usize)) {
                Some(discriminant) => Constant::Int(*discriminant),
                None => return Err(ConstFault::Unsupported { expr, what: "a cast of a variant outside its enum".to_string() }),
            },
            (_, value) => value,
        };
        if let Some(source) = self.typed(operand)
            && let Some(number) = self.number_cast(expr, &source, value)
        {
            return number;
        }
        let converted = match (self.typed(expr), value) {
            (Some(Ty::F64), Constant::Int(v)) => Constant::Real(Real::new(v as f64)),
            (Some(Ty::F64), Constant::Real(r)) => Constant::Real(r),
            (Some(to), value) => match (Width::of(&to), value) {
                (Some(width), Constant::Int(v)) => width.wrap(v),
                (Some(width), Constant::Real(r)) => width.saturate(r.get()),
                (Some(width), Constant::Bool(b)) => width.wrap(i128::from(b)),
                (None, _) | (Some(_), Constant::Aggregate(_) | Constant::FloatBits(_) | Constant::Wide(_)) => return Err(ConstFault::Unsupported { expr, what: "this cast".to_string() }),
            },
            (None, _) => return Err(ConstFault::Unsupported { expr, what: "this cast".to_string() }),
        };
        Ok(converted)
    }

    /// A call of a function of the reachable set, or of a function a local holds: its body
    /// evaluated over its arguments. Every other callee needs a run.
    fn call(&mut self, expr: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        if let Some(Callee::Intrinsic(parser @ (Intrinsic::ParseTable | Intrinsic::ParseHex))) = self.types.callee(expr) {
            return self.parse_array(expr, parser, args, bindings);
        }
        if let Some(Callee::Intrinsic(Intrinsic::Logic)) = self.types.callee(expr) {
            return self.logic_literal(expr, args);
        }
        let function = match self.types.callee(expr) {
            Some(Callee::Function(function)) => FnValue::Named(function),
            _ => match self.called_function_value(expr) {
                Some(function) => function,
                None => {
                    let what = match &self.design.expr_node(expr).kind {
                        ExprKind::Call { class: crate::pom::CallClass::Probe, .. } => "a probe",
                        ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. } => "the result of an instance",
                        _ => "a call that is not a function",
                    };
                    return Err(ConstFault::Dynamic { expr, what: what.to_string() });
                }
            },
        };
        let mut arguments = Vec::new();
        for arg in args {
            arguments.push(match self.fn_value_of(*arg) {
                Some(function) => Argument { value: None, function: Some(function) },
                None => Argument { value: Some(self.fold(*arg, bindings)?), function: None },
            });
        }
        // a call of a generic function evaluates its body for the arguments the checker recorded, read under the
        // instance being evaluated: its const parameters are values in the body (GN-01, GN-14)
        let outer = match self.types.instantiation(expr).cloned() {
            Some(instantiation) => {
                // a type argument can nest as deep as the instantiation chain does (D7): the walk over it has its own stack
                let substitution = crate::stack::Stack::wide(|| self.instantiation_substitution(&instantiation));
                Some(self.set_substitution(substitution))
            }
            None => None,
        };
        let result = self.apply_value(expr, function, arguments, bindings);
        if let Some(outer) = outer {
            self.set_substitution(outer);
        }
        result
    }

    /// What the parameters of the item of `instantiation` are replaced by: each argument under the substitution of the
    /// instance being evaluated, a const expression folded to its value.
    fn instantiation_substitution(&self, instantiation: &crate::types::Instantiation) -> crate::types::Substitution {
        let arguments: Vec<GenericValue> = instantiation
            .arguments
            .iter()
            .map(|argument| match self.substitution.apply_value(argument) {
                GenericValue::Const(ConstTerm::Expr(expr)) => self.integer(expr).and_then(|count| i128::try_from(count).ok()).map_or(GenericValue::Const(ConstTerm::Expr(expr)), |count| GenericValue::Const(ConstTerm::Value(count))),
                other => other,
            })
            .collect();
        crate::types::Substitution::of(&crate::types::GenericSignature::of(self.design, instantiation.item), &arguments)
    }

    /// The body of the function `function` evaluated for `arguments`; a function reads no param of
    /// the caller.
    fn apply_function(&mut self, expr: ExprId, function: DeclId, arguments: Vec<Argument>) -> Result<Constant, ConstFault> {
        let data = self.design.decl_data(function);
        let DeclKind::Fn(f) = &data.kind else {
            return Err(ConstFault::Unsupported { expr, what: "a call of a name that is not a function".to_string() });
        };
        let Some(body) = &f.body else {
            return Err(ConstFault::Unsupported { expr, what: "a function without a body".to_string() });
        };
        self.enter(expr)?;
        let params = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        let (mut frame, mut functions) = (BTreeMap::new(), BTreeMap::new());
        for (param, argument) in params.zip(arguments) {
            if let Some(value) = argument.value {
                frame.insert(param, value);
            }
            if let Some(function) = argument.function {
                functions.insert(param, function);
            }
        }
        let outer = std::mem::replace(&mut self.locals, frame);
        let outer_functions = std::mem::replace(&mut self.fn_values, functions);
        let result = crate::stack::Stack::deep(|| self.exec_block(expr, &body.stmts, &Bindings::new()));
        self.depth -= 1;
        self.locals = outer;
        self.fn_values = outer_functions;
        self.returned(expr, result)
    }

    /// A method call: a row of the Appendix H catalogue applied to
    /// its receiver and arguments.
    fn method(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        // a method of the project, or the `impl` a bound picks for the concrete receiver: the function's body over the receiver,
        // then the arguments (generics T35, `dispatch.rs`)
        let called = match self.types.callee(expr) {
            Some(Callee::Method(function)) => Some(function),
            Some(Callee::Bound { item }) => self.typed(recv).and_then(|receiver| self.bound_implementation(expr, item, &receiver)),
            _ => None,
        };
        if let Some(function) = called {
            let mut arguments = Vec::new();
            for operand in std::iter::once(&recv).chain(args) {
                arguments.push(match self.fn_value_of(*operand) {
                    Some(function) => Argument { value: None, function: Some(function) },
                    None => Argument { value: Some(self.fold(*operand, bindings)?), function: None },
                });
            }
            return self.apply_value(expr, FnValue::Named(function), arguments, bindings);
        }
        if let Some(method) = self.types.method(expr) {
            // the rows over arrays, ranges, iterators and `Option`s take closures and composites (`composite.rs`)
            if let Some(result) = self.sequence_row(expr, method, recv, args, bindings) {
                return result;
            }
            if let Some(result) = self.option_row(expr, method, recv, args, bindings) {
                return result;
            }
        }
        if let Some(levels) = self.encoded_levels(expr, recv, args, bindings) {
            return levels;
        }
        let receiver = self.fold(recv, bindings);
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.fold(*arg, bindings));
        }
        if self.types.builtin(expr).is_some() {
            return Err(ConstFault::Unsupported { expr, what: "this method in a const expression".to_string() });
        }
        let Some(method) = self.types.method(expr) else {
            return Err(ConstFault::Unsupported { expr, what: "a method outside the catalogue".to_string() });
        };
        let (receiver, values) = (receiver?, values.into_iter().collect::<Result<Vec<_>, _>>()?);
        self.known_method(expr, recv, args, method, receiver, values)
    }

    /// `x.encode()` of a `T: Repr` parameter at `bool`, an integer or `f64` (§2 rules 2 and 3): the levels of the receiver,
    /// least significant first, as an array of `bool`. `None` when the call is no such `encode`.
    fn encoded_levels(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Option<Result<Constant, ConstFault>> {
        let ExprKind::MethodCall { method, .. } = &self.design.expr_node(expr).kind else { return None };
        if method.text != "encode" || !args.is_empty() || self.types.method(expr).is_some() || self.types.builtin(expr).is_some() {
            return None;
        }
        let width = match self.typed(recv)? {
            Ty::Bool => 1,
            Ty::UInt(bits) | Ty::Int(bits) => u32::from(bits),
            Ty::F64 => 64,
            _ => return None,
        };
        let word = match self.fold(recv, bindings) {
            Ok(Constant::Bool(high)) => i128::from(high),
            Ok(Constant::Int(value)) => value,
            Ok(Constant::Real(real)) => i128::from(real.get().to_bits()),
            Ok(_) => return Some(Err(ConstFault::Unsupported { expr, what: "an `encode` of a value that is no scalar".to_string() })),
            Err(fault) => return Some(Err(fault)),
        };
        let levels = (0..width).map(|level| Constant::Bool((word >> level) & 1 == 1)).collect();
        Some(Ok(self.make(CompositeKind::Array, levels)))
    }

    /// A catalogue row applied to constants.
    fn known_method(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], method: Method, receiver: Constant, values: Vec<Constant>) -> Result<Constant, ConstFault> {
        if let Some(number) = self.number_method(expr, recv, args, method, receiver, &values) {
            return number;
        }
        let crossed = || Err(ConstFault::Panic { expr, what: "a `clamp` whose lower bound is above its upper bound".to_string() });
        if let Constant::Int(x) = receiver {
            let ints: Vec<i128> = values.iter().filter_map(|v| if let Constant::Int(i) = v { Some(*i) } else { None }).collect();
            return match (method, ints.as_slice()) {
                (Method::Min, [y]) => Ok(Constant::Int(x.min(*y))),
                (Method::Max, [y]) => Ok(Constant::Int(x.max(*y))),
                (Method::Clamp, [lo, hi]) if lo > hi => crossed(),
                (Method::Clamp, [lo, hi]) => Ok(Constant::Int(x.max(*lo).min(*hi))),
                (Method::RemEuclid, [0]) => Err(ConstFault::Panic { expr, what: "a `rem_euclid` by zero".to_string() }),
                (Method::RemEuclid, [d]) => Ok(Constant::Int(x.rem_euclid(*d))),
                (Method::IntAbs | Method::Signum | Method::Pow | Method::SaturatingAdd | Method::SaturatingSub | Method::SaturatingMul, _) => {
                    let Some(width) = self.width(expr) else { return Err(ConstFault::Unsupported { expr, what: "this method of an integer".to_string() }) };
                    match (method, ints.as_slice()) {
                        // Arithmetic wraps (§1): `(-128i8).abs()` is `-128`, `2u8.pow(9)` is 0.
                        (Method::IntAbs, []) => Ok(width.wrap(x.abs())),
                        (Method::Signum, []) => Ok(Constant::Int(x.signum())),
                        (Method::Pow, [e]) => {
                            let (mut base, mut acc, mut e) = (x, 1i128, *e);
                            while e > 0 {
                                if e & 1 == 1 {
                                    acc = width.wrapped(acc.wrapping_mul(base));
                                }
                                base = width.wrapped(base.wrapping_mul(base));
                                e >>= 1;
                            }
                            Ok(width.wrap(acc))
                        }
                        (Method::SaturatingAdd, [y]) => Ok(Constant::Int(width.clamp(x + *y))),
                        (Method::SaturatingSub, [y]) => Ok(Constant::Int(width.clamp(x - *y))),
                        (Method::SaturatingMul, [y]) => Ok(Constant::Int(width.clamp(x.saturating_mul(*y)))),
                        _ => Err(ConstFault::Unsupported { expr, what: "this method of an integer".to_string() }),
                    }
                }
                _ => Err(ConstFault::Unsupported { expr, what: "this method of an integer".to_string() }),
            };
        }
        let x = Self::real(recv, receiver)?;
        let mut reals = Vec::with_capacity(values.len());
        for (arg, value) in args.iter().zip(values) {
            reals.push(Self::real(*arg, value)?);
        }
        let result = match (method, reals.as_slice()) {
            (Method::Exp, []) => x.exp(),
            (Method::Ln, []) => x.ln(),
            (Method::Sqrt, []) => x.sqrt(),
            (Method::Abs, []) => x.abs(),
            (Method::Floor, []) => x.floor(),
            (Method::Min, [y]) => x.min(*y),
            (Method::Max, [y]) => x.max(*y),
            // `f64::clamp` panics on crossed or NaN bounds, as the compiled code does.
            (Method::Clamp, [lo, hi]) if lo.is_nan() || hi.is_nan() || lo > hi => return crossed(),
            (Method::Clamp, [lo, hi]) => x.clamp(*lo, *hi),
            // Constant from msl-digital T27 on, with the `Option` pair.
            _ => return Err(ConstFault::Unsupported { expr, what: "this method in a const expression".to_string() }),
        };
        Ok(Constant::Real(Real::new(result)))
    }
}

/// The width and sign of an integer type: what a result wraps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Width {
    bits: u32,
    signed: bool,
}

impl Width {
    /// The width of an integer type of at most 64 bits; a wider one is a `DynamicInteger` (`numbers.rs`).
    fn of(ty: &Ty) -> Option<Width> {
        match *ty {
            Ty::UInt(n) if n <= 64 => Some(Width { bits: n, signed: false }),
            Ty::Int(n) if n <= 64 => Some(Width { bits: n, signed: true }),
            _ => None,
        }
    }

    /// `v` modulo `2^bits` (§1 rule 6), held as the compiled code holds it: below `2^bits`
    /// unsigned, sign-extended signed.
    fn wrap(self, v: i128) -> Constant {
        let modulus = 1i128 << self.bits;
        let low = v.rem_euclid(modulus);
        Constant::Int(if self.signed && low >= modulus / 2 { low - modulus } else { low })
    }

    /// The value of `v` modulo `2^bits` as an `i128`, for a running product.
    fn wrapped(self, v: i128) -> i128 {
        match self.wrap(v) {
            Constant::Int(w) => w,
            _ => v,
        }
    }

    /// `v` clamped to the range of this type (`saturating_*`).
    fn clamp(self, v: i128) -> i128 {
        let (min, max) = if self.signed { (-(1i128 << (self.bits - 1)), (1i128 << (self.bits - 1)) - 1) } else { (0, (1i128 << self.bits) - 1) };
        v.clamp(min, max)
    }

    /// `x as` this type (G4): truncated toward zero, saturated to the type's range, NaN as 0.
    fn saturate(self, x: f64) -> Constant {
        if x.is_nan() {
            return Constant::Int(0);
        }
        let (min, max) = if self.signed { (-(1i128 << (self.bits - 1)), (1i128 << (self.bits - 1)) - 1) } else { (0, (1i128 << self.bits) - 1) };
        // `as i128` truncates toward zero and saturates; the range of 64 bits fits in it.
        Constant::Int((x as i128).clamp(min, max))
    }
}
