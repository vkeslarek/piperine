//! The result table of a check, and the entry point that makes it.

use std::collections::{BTreeMap, BTreeSet};

use crate::msl::{Diagnostic, Span};
use crate::pom::{BuiltinId, DeclId, Design, DesignDiagnostic, ExprId, ExprKind, FileId, LitKind, Reflect, TypeId};

use super::check;
use super::model::ModelCheck;
use super::{GenericValue, Intrinsic, Nominal, Ty};

/// One instantiation of a generic item: the item and the arguments it is given, in the order of its
/// parameters (§3.8.10: two uses with equal arguments are one instance).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instantiation {
    /// The generic struct, enum or function.
    pub item: DeclId,
    /// The argument for each parameter, defaults filled in.
    pub arguments: Vec<GenericValue>,
}

/// What a `Deferred` method call of the reachable set resolves to: a row of the Appendix H
/// catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Exp,
    Ln,
    Sqrt,
    Abs,
    /// `a.min(b)` on an integer or `f64`.
    Min,
    /// `a.max(b)` on an integer or `f64`.
    Max,
    /// `x.clamp(lo, hi)` on an integer or `f64`.
    Clamp,
    /// `x.floor()` on `f64`.
    Floor,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sinh,
    Cosh,
    Tanh,
    Powf,
    /// `x.powi(e: i32)` on `f64`.
    Powi,
    Log10,
    Log2,
    Ceil,
    Round,
    Trunc,
    Fract,
    /// `x.signum()` on `f64` or a signed integer.
    Signum,
    /// `x.rem_euclid(d)` on `f64` or an integer.
    RemEuclid,
    Hypot,
    IsNan,
    IsFinite,
    IsInfinite,
    /// `x.abs()` on a signed integer.
    IntAbs,
    /// `x.pow(e: u32)` on an integer.
    Pow,
    SaturatingAdd,
    SaturatingSub,
    SaturatingMul,
    /// `a.checked_add(b)`: an `Option` of the wrapping sum.
    CheckedAdd,
    CheckedSub,
    CheckedMul,
    /// `o.is_some()` on an `Option`.
    IsSome,
    /// `o.is_none()` on an `Option`.
    IsNone,
    /// `o.unwrap_or(d)` on an `Option`.
    UnwrapOr,
    /// `o.unwrap()`: a fault on `None`.
    Unwrap,
    /// `o.expect("msg")`: a fault with `msg` on `None`.
    Expect,
    /// `o.unwrap_or_else(|| d)`.
    UnwrapOrElse,
    /// `o.map(|x| e)`.
    Map,
    /// `o.and_then(|x| p)`.
    AndThen,
    /// `a.or(b)` on `Option`s.
    Or,
    /// `c.then(|| v)` on a `bool`.
    Then,
    /// `c.then_some(v)` on a `bool`.
    ThenSome,
    /// `a.len()` on an array (H.5).
    ArrayLen,
    /// `a.is_empty()` on an array.
    ArrayIsEmpty,
    /// `a.iter()`: the elements of an array as an iterator.
    ArrayIter,
    /// `a.contains(x)` on an array of a `PartialEq` type.
    ArrayContains,
    /// `a.first()` on an array: `Some` of element 0.
    ArrayFirst,
    /// `a.last()` on an array: `Some` of the last element.
    ArrayLast,
    /// `a.get(i)` on an array: `None` past the end.
    ArrayGet,
    /// `r.contains(x)` on a range (H.6).
    RangeContains,
    /// `r.is_empty()` on a range.
    RangeIsEmpty,
    /// `it.map(f)` on an iterator (H.7).
    IteratorMap,
    /// `it.filter(f)`.
    IteratorFilter,
    /// `it.fold(init, f)`.
    IteratorFold,
    /// `it.sum()` of an integer or float element type.
    IteratorSum,
    /// `it.count()`.
    IteratorCount,
    /// `it.zip(other)`.
    IteratorZip,
    /// `it.enumerate()`.
    IteratorEnumerate,
    /// `it.rev()` on a reversible iterator.
    IteratorRev,
    /// `it.any(f)`.
    IteratorAny,
    /// `it.all(f)`.
    IteratorAll,
}

impl Method {
    /// Whether the row is one of H.5 to H.7 the elaborator lowers as a loop over a sequence and
    /// reads as a value (`first`, `last` and `get` are `Option`s, lowered on their pair).
    pub fn is_sequence_row(self) -> bool {
        matches!(
            self,
            Method::ArrayLen | Method::ArrayIsEmpty | Method::ArrayContains | Method::RangeContains | Method::RangeIsEmpty | Method::IteratorSum | Method::IteratorCount | Method::IteratorFold | Method::IteratorAny | Method::IteratorAll
        )
    }
}

/// A method the checker treats as part of the language because it resolved to the stdlib's own
/// item (part I §3.3.7): the edges of `Bit` and the history of a
/// carrier. A project's replacement is `P0001` at its use, never one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// `c.changed() -> Event`.
    Changed,
    /// `c.prev() -> T`: the value before the most recent change.
    Prev,
    /// `c.rise() -> Event` on a `Bit`.
    Rise,
    /// `c.fall() -> Event` on a `Bit`.
    Fall,
    /// `p.inflow() -> f64`: the flow entering the module at its port `p` (§6 rule 5).
    Inflow,
}

/// Whether an expression reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or
/// `analysis(Static)`, directly or through a `let`, a function argument or a result (part III §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuity {
    Discrete,
    Continuous,
}

/// What a call or a placement of the reachable set refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    Function(DeclId),
    Core(BuiltinId),
    Module(DeclId),
    /// A method of the project called on a value, by the function it resolves to (§3.7.8).
    Method(DeclId),
    /// A method a type parameter's bound gives, by the trait's function: the instance a call reaches is
    /// the implementation for the concrete type, found where the function is instantiated.
    Bound {
        /// The trait's function.
        item: DeclId,
    },
    /// A function of §22.1 typed by its contract at the concrete type (`merge`, `same`, `cast`,
    /// `logic`, `matches_bits`), until `generics` lets it compile from its stdlib source.
    Intrinsic(Intrinsic),
}

/// The types of everything one top module reaches.
///
/// Equality compares the tables, not the borrowed design.
#[derive(Debug, Clone)]
pub struct Types<'d> {
    pub(super) design: &'d Design,
    pub(super) exprs: Vec<Option<Ty>>,
    pub(super) locals: BTreeMap<DeclId, Ty>,
    pub(super) methods: BTreeMap<ExprId, Method>,
    pub(super) callees: BTreeMap<ExprId, Callee>,
    pub(super) bindings: BTreeMap<ExprId, Vec<Option<ExprId>>>,
    pub(super) reached: Vec<DeclId>,
    pub(super) somes: BTreeSet<ExprId>,
    pub(super) carriers: BTreeMap<DeclId, Ty>,
    /// The array carriers whose element drives only the elaborator can judge, per instance (`DeferredCoverage`).
    pub(super) deferred_coverage: super::coverage::DeferredCoverage,
    pub(super) builtins: BTreeMap<ExprId, Builtin>,
    pub(super) event_reads: BTreeSet<ExprId>,
    pub(super) widths: BTreeMap<ExprId, u32>,
    /// The type `T` of each `T::default()` call of the reachable set (H.8 rule 4).
    pub(super) defaults: BTreeMap<ExprId, Ty>,
    /// The typed fields of every struct and every enum variant the check reached.
    pub(super) layouts: BTreeMap<DeclId, Vec<super::FieldType>>,
    /// The tag of every enum the check reached.
    pub(super) enum_tags: BTreeMap<DeclId, super::EnumTag>,
    pub(super) written: BTreeMap<TypeId, Ty>,
    pub(super) obligations: BTreeMap<DeclId, Vec<super::Obligation>>,
    /// The parameters of the functions of the check that are handles (`handles.rs`).
    pub(super) handles: std::collections::BTreeSet<DeclId>,
    /// The arguments of the trait bound each call through a bound names (`bounds.rs`).
    pub(super) bound_arguments: BTreeMap<ExprId, Vec<Ty>>,
    pub(super) instantiations: BTreeMap<ExprId, Instantiation>,
    pub(super) continuous: BTreeSet<ExprId>,
    pub(super) emits: BTreeMap<DeclId, BTreeSet<String>>,
    /// The locals some assignment of a checked body writes (§1.5.12): the elaborator makes exactly these assignable.
    pub(super) assigned: BTreeSet<DeclId>,
}

impl PartialEq for Types<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.exprs == other.exprs && self.locals == other.locals && self.methods == other.methods && self.callees == other.callees && self.bindings == other.bindings && self.reached == other.reached && self.somes == other.somes && self.carriers == other.carriers && self.builtins == other.builtins && self.event_reads == other.event_reads && self.widths == other.widths && self.defaults == other.defaults && self.layouts == other.layouts && self.enum_tags == other.enum_tags && self.written == other.written && self.continuous == other.continuous && self.emits == other.emits && self.assigned == other.assigned
    }
}

impl<'d> Types<'d> {
    /// Check `top`, a module of the root crate, and everything it reaches.
    ///
    /// A design that already holds diagnostics gives them back and nothing is checked; so does
    /// a top that is not a module of the root crate (one diagnostic naming it).
    pub fn check(design: &'d Design, top: &str) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        if !design.diagnostics().is_empty() {
            return Err(design.diagnostics().to_vec());
        }
        let Some(module) = design.module(top).filter(|m| m.krate().is_root()) else {
            let diagnostic = Diagnostic::new("P0003", format!("`{top}` is not a module of the root crate"), Span::new(0, 0), "no such top module");
            return Err(vec![DesignDiagnostic { file: FileId(0), diagnostic }]);
        };
        check::Checker::new(design).run(module.decl().id())
    }

    /// Check every module, function, `const` and `impl` function of every crate but `std`, reached
    /// from a top or not, and give back only the diagnostics (S3, GN-08): the table of a top is
    /// [`Types::check`]'s. A design that already holds diagnostics gives them back.
    pub fn check_model(design: &Design) -> Result<(), Vec<DesignDiagnostic>> {
        if !design.diagnostics().is_empty() {
            return Err(design.diagnostics().to_vec());
        }
        ModelCheck::new(design).run()
    }

    /// The design the table was built over.
    pub fn design(&self) -> &'d Design {
        self.design
    }

    /// Whether some assignment of a checked body writes the local `decl` (§1.5.12).
    pub fn is_assigned(&self, decl: DeclId) -> bool {
        self.assigned.contains(&decl)
    }

    /// The type of an expression; `None` outside the reachable set.
    pub fn ty(&self, expr: ExprId) -> Option<&Ty> {
        self.exprs.get(expr.index())?.as_ref()
    }

    /// The integer type an integer literal of the reachable set was given: its suffix, the type
    /// its position expects, else `i32` (§1 rule 7). `None` for any other expression.
    pub fn literal(&self, expr: ExprId) -> Option<&Ty> {
        let is_integer_literal = matches!(&self.design.expr_node(expr).kind, ExprKind::Lit(lit) if matches!(&lit.kind, LitKind::Int(text) if !text.ends_with("f64")));
        self.ty(expr).filter(|ty| is_integer_literal && ty.is_integer())
    }

    /// Whether `expr`, an argument of type `T` for a parameter of type `Option<T>`, is read as
    /// `Some` of it (§3.2.3).
    pub fn implicit_some(&self, expr: ExprId) -> bool {
        self.somes.contains(&expr)
    }

    /// The catalogue row a `Deferred` method call resolved to.
    pub fn method(&self, expr: ExprId) -> Option<Method> {
        self.methods.get(&expr).copied()
    }

    /// The built-in a method call resolved to (`prev`, `cast`, ..), set only when the name
    /// resolved to the stdlib's own item.
    pub fn builtin(&self, expr: ExprId) -> Option<Builtin> {
        self.builtins.get(&expr).copied()
    }

    /// Whether `expr`, an `Event` in a guard or a reaction body, is read as a `bool` that says
    /// whether it fired in the microstep (§13 rules 2 and 9). Its type is `bool`.
    pub fn event_read(&self, expr: ExprId) -> bool {
        self.event_reads.contains(&expr)
    }

    /// The value of the const expression `expr` of a generic item for an instantiation: `values` gives each
    /// const parameter its argument. `None` when it does not fold (an underflow, an overflow, a division by zero).
    pub fn fold_with(&self, expr: ExprId, values: BTreeMap<DeclId, u128>) -> Option<u128> {
        super::lengths::IntegerFolder::with_values(self.design, values).fold(expr, 16)
    }

    /// The level count a `T::WIDTH` path of the reachable set reads (§2, §22.1): 1 for `bool`, `N`
    /// for `uN` and `iN`, 64 for `f64`, the tag width of an enum. `None` for any other expression.
    pub fn width(&self, expr: ExprId) -> Option<u32> {
        self.widths.get(&expr).copied()
    }

    /// The type `T` of a `T::default()` call of the reachable set (H.8 rule 4), whose value is the
    /// default of `T`. `None` for any other expression.
    pub fn default_of(&self, expr: ExprId) -> Option<&Ty> {
        self.defaults.get(&expr)
    }

    /// The structs and enums the check reached, in declaration order of the design: the types a
    /// value of the design may have.
    pub fn aggregate_types(&self) -> Vec<DeclId> {
        let structs = self.layouts.keys().copied().filter(|d| matches!(self.design.decl_data(*d).kind, crate::pom::DeclKind::Struct(_)));
        let mut all: Vec<DeclId> = structs.chain(self.enum_tags.keys().copied()).collect();
        all.sort();
        all
    }

    /// Every instance of a generic struct or enum a type of the check holds, with all its arguments given, each after
    /// the instances inside its arguments: the arena entries the bodies that are not generic need (GN-06).
    pub fn generic_instances(&self) -> Vec<Nominal> {
        let mut found = Vec::new();
        self.exprs.iter().flatten().chain(self.locals.values()).chain(self.written.values()).for_each(|ty| ty.generic_instances(&mut found));
        found
    }

    /// The fields of `owner` (a generic struct, or one of its variants) as the instance `nominal` has them: the
    /// declared field types with the instance's arguments put in the place of the parameters.
    pub fn instance_fields(&self, owner: DeclId, nominal: &Nominal) -> Vec<super::FieldType> {
        let substitution = super::Substitution::of(&super::GenericSignature::of(self.design, nominal.decl), &nominal.arguments);
        self.fields(owner).unwrap_or(&[]).iter().map(|field| super::FieldType { ty: substitution.apply(&field.ty), ..field.clone() }).collect()
    }

    /// The typed fields of the struct or enum variant `owner`, in declaration order.
    pub fn fields(&self, owner: DeclId) -> Option<&[super::FieldType]> {
        self.layouts.get(&owner).map(Vec::as_slice)
    }

    /// The value of the const expression `expr` of an array length or a range bound: a non-negative
    /// integer built of literals, `const` names and integer arithmetic (§1.2.5). The elaborator's
    /// own constant evaluator needs a typed literal, which a length does not have.
    pub fn integer_constant(&self, expr: ExprId) -> Option<u128> {
        super::lengths::IntegerFolder::of(self.design).fold(expr, 16)
    }

    /// The tag of the enum `decl`.
    pub fn enum_tag(&self, decl: DeclId) -> Option<&super::EnumTag> {
        self.enum_tags.get(&decl)
    }

    /// Whether the struct or enum `decl` writes `#[derive(.., name, ..)]`.
    pub fn derives(&self, decl: DeclId, name: &str) -> bool {
        super::derives::DerivedTrait::is_listed(self.design, decl, name)
    }

    /// Whether an expression of the reachable set is discrete or continuous (part III §8.2);
    /// `None` outside it.
    pub fn continuity(&self, expr: ExprId) -> Option<Continuity> {
        self.ty(expr)?;
        Some(if self.continuous.contains(&expr) { Continuity::Continuous } else { Continuity::Discrete })
    }

    /// The streams a function of the reachable set writes, directly or through the functions it
    /// calls; `None` for a function that reaches no `emit!` (part VI §19).
    pub fn emits(&self, function: DeclId) -> Option<&BTreeSet<String>> {
        self.emits.get(&function)
    }

    /// The type of a discrete port or signal declaration, `Ty::Carrier`; `None` for a node, a
    /// param or a declaration outside the reachable set.
    pub fn carrier(&self, decl: DeclId) -> Option<&Ty> {
        self.carriers.get(&decl)
    }

    /// Whether the elements of the carrier `decl` no initial value covers are judged per instance (`E0187`): a length is
    /// not known yet and a constant element names some of them, or a drive is written inside generation.
    pub fn defers_missing_initial_value(&self, decl: DeclId) -> bool {
        self.deferred_coverage.missing_initial.contains(&decl)
    }

    /// Whether the array carrier `decl` has an initial value and an `out` port of a child holds some of its elements, with a
    /// length not known yet: `E0223` is judged per instance, once every element is handed.
    pub fn defers_handed_initial_value(&self, decl: DeclId) -> bool {
        self.deferred_coverage.handed_initial.contains(&decl)
    }

    /// Whether the array carrier `decl` has an initial value and undelayed definitions name some of its elements, with a
    /// length not known yet: `E0338` is judged per instance, once every element is defined.
    pub fn defers_defined_initial_value(&self, decl: DeclId) -> bool {
        self.deferred_coverage.defined_initial.contains(&decl)
    }

    /// Whether a drive of the carrier `decl` reaches elements only the elaborator folds (a generic level, a generation
    /// index, a drive written inside generation): two definitions sharing an element (`E0335`), a definition and a commit
    /// sharing one (`E0221`) and a second drive of an element handed to an `out` port (`E0220`) are judged per instance.
    pub fn defers_drive_overlaps(&self, decl: DeclId) -> bool {
        self.deferred_coverage.overlaps.contains(&decl)
    }

    /// Whether the carrier `decl` (or each carrier of an array of them) has a one-driver nature (§5.2): a loop of
    /// definitions through it is an error, where one through resolved carriers only settles by relaxation (§8.2.3).
    pub fn is_one_driver(&self, decl: DeclId) -> bool {
        match self.carriers.get(&decl).map(Ty::innermost) {
            Some(Ty::Carrier { nature, .. }) => super::nature_rules::NatureRules::of(self.design, *nature).one_driver,
            _ => false,
        }
    }

    /// The type of a port or signal declared as an array of discrete carriers (`[Bit<u8>; 4]`, nested included): an
    /// array whose innermost element is a `Ty::Carrier` (CV-23).
    pub fn carrier_array(&self, decl: DeclId) -> Option<&Ty> {
        self.carriers.get(&decl).filter(|ty| matches!(ty, Ty::Array(..)) && matches!(ty.innermost(), Ty::Carrier { .. }))
    }

    /// The type of a port or signal declared as an array of nodes (`[Electrical; 4]`): an array whose innermost
    /// element is a `Ty::Node` (CV-23).
    pub fn node_array(&self, decl: DeclId) -> Option<&Ty> {
        self.carriers.get(&decl).filter(|ty| matches!(ty, Ty::Array(..)) && matches!(ty.innermost(), Ty::Node(_)))
    }

    /// What a call or a placement refers to.
    pub fn callee(&self, expr: ExprId) -> Option<Callee> {
        self.callees.get(&expr).copied()
    }

    /// The arguments of a call of a function, one slot per parameter in order: the argument bound
    /// to it, or `None` for an optional parameter left out (§3.2).
    pub fn binding(&self, expr: ExprId) -> Option<&[Option<ExprId>]> {
        self.bindings.get(&expr).map(Vec::as_slice)
    }

    /// The instantiation a call of a generic function of the reachable set makes: the function and
    /// the argument of each parameter, inferred or written, defaults filled in.
    pub fn instantiation(&self, call: ExprId) -> Option<&Instantiation> {
        self.instantiations.get(&call)
    }

    /// The arguments the bound writes for its trait in the call `call` of a method through a bound (`C: Convert<f64>`
    /// gives `[f64]`); empty for a trait with none. The call's `impl` is the one of that argument list (GN-04).
    pub fn bound_arguments(&self, call: ExprId) -> &[Ty] {
        self.bound_arguments.get(&call).map_or(&[], Vec::as_slice)
    }

    /// Whether the function parameter `param` is a handle: it stands for the carrier the caller passes (§3.1.5).
    pub fn handle(&self, param: DeclId) -> bool {
        self.handles.contains(&param)
    }

    /// What the definition of the generic function `item` defers to each instantiation: its
    /// `where`, the lengths it could only accept and its const expressions (Q1 (c)).
    pub fn obligations(&self, item: DeclId) -> &[super::Obligation] {
        self.obligations.get(&item).map_or(&[], Vec::as_slice)
    }

    /// The instantiation a type node of the reachable set writes when it is a use of a generic
    /// struct or enum (`Pair<f64>`); `None` for any other type.
    pub fn written_instantiation(&self, ty: TypeId) -> Option<Instantiation> {
        match self.written.get(&ty)? {
            Ty::Struct(Nominal { decl, arguments }) | Ty::Enum(Nominal { decl, arguments }) if !arguments.is_empty() => Some(Instantiation { item: *decl, arguments: arguments.clone() }),
            _ => None,
        }
    }

    /// The type a type node of the reachable set writes (a signature, an annotation, a cast).
    pub fn written(&self, ty: TypeId) -> Option<&Ty> {
        self.written.get(&ty)
    }

    /// Whether the written type `ty` is the name of a nature parameter of a generic item: a node of the nature a use gives.
    pub fn is_nature_parameter_type(&self, design: &Design, ty: TypeId) -> bool {
        matches!(self.written.get(&ty), Some(Ty::Param(param)) if super::generics::GenericSignature::is_nature_parameter(design, *param))
    }

    /// The type of a `Binding` declaration.
    pub fn local(&self, binding: DeclId) -> Option<&Ty> {
        self.locals.get(&binding)
    }

    /// The arguments of the generic `impl` that holds the method `function`, found from the receiver's type
    /// `receiver` by taking the `impl`'s self type apart (`impl<T: Shape> Describe for T` meeting `Sq` gives `T = Sq`,
    /// `impl<T> Window<T>` meeting `Window<f64>` gives `T = f64`), in the order of the `impl`'s parameters. `None` when
    /// the method is not of a generic `impl` or a parameter stays open.
    pub fn impl_arguments(&self, function: DeclId, receiver: &Ty) -> Option<Vec<GenericValue>> {
        let owner = self.design.decl_data(function).parent.filter(|owner| matches!(self.design.decl_data(*owner).kind, crate::pom::DeclKind::Impl(_)) && self.design.decl_data(*owner).generics_span.is_some())?;
        let first = self.design.decl_data(function).children.iter().copied().find(|child| matches!(self.design.decl_data(*child).kind, crate::pom::DeclKind::FnParam(_)))?;
        let signature = super::GenericSignature::of(self.design, owner);
        let mut inference = super::inference::Inference::new(&signature);
        inference.unify(self.local(first)?, receiver);
        signature.parameters().iter().map(|parameter| inference.found().get(parameter.decl).cloned()).collect()
    }

    /// The modules, functions and constants checked, in first-reach order.
    pub fn reached(&self) -> &[DeclId] {
        &self.reached
    }
}
