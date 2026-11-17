//! The result table of a check, and the entry point that makes it.

use std::collections::{BTreeMap, BTreeSet};

use crate::msl::{Diagnostic, Span};
use crate::pom::{BuiltinId, DeclId, Design, DesignDiagnostic, ExprId, ExprKind, FileId, LitKind, Reflect, TypeId};

use super::check;
use super::Ty;

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
}

/// A method the checker treats as part of the language because it resolved to the stdlib's own
/// item (part I §3.3.7): the four flag-total reads, the edges of `Bit` and the history of a
/// carrier. A project's replacement is `P0001` at its use, never one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// `x.known() -> Option<T>`.
    Known,
    /// `x.known_or(d) -> T`.
    KnownOr,
    /// `x.is_known() -> bool`.
    IsKnown,
    /// `c.high() -> bool` on a `Bit`.
    High,
    /// `c.changed() -> Event`.
    Changed,
    /// `c.prev() -> T`: the value before the most recent change.
    Prev,
    /// `c.rise() -> Event` on a `Bit`.
    Rise,
    /// `c.fall() -> Event` on a `Bit`.
    Fall,
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
    pub(super) builtins: BTreeMap<ExprId, Builtin>,
    pub(super) written: BTreeMap<TypeId, Ty>,
    pub(super) continuous: BTreeSet<ExprId>,
    pub(super) emits: BTreeMap<DeclId, BTreeSet<String>>,
}

impl PartialEq for Types<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.exprs == other.exprs && self.locals == other.locals && self.methods == other.methods && self.callees == other.callees && self.bindings == other.bindings && self.reached == other.reached && self.somes == other.somes && self.carriers == other.carriers && self.builtins == other.builtins && self.written == other.written && self.continuous == other.continuous && self.emits == other.emits
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

    /// The design the table was built over.
    pub fn design(&self) -> &'d Design {
        self.design
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

    /// The built-in a method call resolved to (`known_or`, `prev`, ..), set only when the name
    /// resolved to the stdlib's own item.
    pub fn builtin(&self, expr: ExprId) -> Option<Builtin> {
        self.builtins.get(&expr).copied()
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

    /// What a call or a placement refers to.
    pub fn callee(&self, expr: ExprId) -> Option<Callee> {
        self.callees.get(&expr).copied()
    }

    /// The arguments of a call of a function, one slot per parameter in order: the argument bound
    /// to it, or `None` for an optional parameter left out (§3.2).
    pub fn binding(&self, expr: ExprId) -> Option<&[Option<ExprId>]> {
        self.bindings.get(&expr).map(Vec::as_slice)
    }

    /// The type a type node of the reachable set writes (a signature, an annotation, a cast).
    pub fn written(&self, ty: TypeId) -> Option<&Ty> {
        self.written.get(&ty)
    }

    /// The type of a `Binding` declaration.
    pub fn local(&self, binding: DeclId) -> Option<&Ty> {
        self.locals.get(&binding)
    }

    /// The modules, functions and constants checked, in first-reach order.
    pub fn reached(&self) -> &[DeclId] {
        &self.reached
    }
}
