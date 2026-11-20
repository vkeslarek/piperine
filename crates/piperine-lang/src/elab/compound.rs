//! Compound values lowered to nodes the IR already has (design "From a source form to IR nodes
//! the contract already has").
//!
//! - An enum of `V` unit variants is its tag (§2 rule 5): `UInt<ceil(log2 V)>`, variant `k` (in
//!   declaration order, from 0) the literal `k`. No IR type names the enum.
//! - A `match` is an `if` chain over its scrutinee, read once: bound to a local unless it already
//!   is a local, a param or a literal. In code that has no local (a drive) the scrutinee is read
//!   again at each test: an expression of the IR has no effect, so the value is the same. Each
//!   arm's pattern is a comparison with the scrutinee, an or-pattern the disjunction of its
//!   alternatives (`|` when the scrutinee may be unknown, `||` when it is known); the last arm of the exhaustive `match` is the final `else`, and so is an
//!   arm that matches everything.
//! - An `Option<T>` is never an IR type: its value is a pair, a `bool` flag `is` and a payload
//!   `val` of `T` (`None` is `(false, zero)`, `Some(e)` is `(true, e)`, `x.known()` is
//!   `(x.is_known(), x.known_or(zero))`). A `let` binds the pair to two locals, `o.is` and
//!   `o.val`; any other position writes the two expressions where they are read, so
//!   `if let Some(d) = x.known() { .. }` is `if x.is_known() { let d = x.known_or(0); .. }`.
//!   `is_some`, `is_none`, `unwrap_or`, `if let` and `match` read the pair; `unwrap_or` of a
//!   literal `None` or `Some` is its value.
//! - `let (a, b) = (x, y);` is one local per component.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Local, Method as IrMethod, Origin, Stmt, StmtId, UnaryOp, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{Arm, DeclId, DeclKind, Design, ExprId, ExprKind, FileId, LitKind, PatId, PatKind, Resolution};
use crate::types::{Builtin, Method, Ty};

/// The scrutinee of a `match`, as each test reads it.
enum Subject {
    /// A local, a param or a literal: lowered again for each test.
    Reread(ExprId),
    /// Bound to this local of this type.
    Local(u32, ValueType),
    /// An `Option`, bound to its flag local and its payload local.
    Pair { is: (u32, ValueType), val: (u32, ValueType) },
    /// An `Option` in code with no local (a drive): its pair lowered again for each test.
    RereadPair(ExprId),
}

/// An `Option` value as the IR holds it: its flag and its payload. `literal` is the flag when
/// the source wrote `None` or `Some(..)`.
struct Pair {
    is: LoweredExpr,
    val: LoweredExpr,
    literal: Option<bool>,
}

/// The tag of an enum: its width and its variants in declaration order.
pub(super) struct EnumLayout {
    variants: Vec<DeclId>,
}

impl EnumLayout {
    pub(super) fn of(design: &Design, decl: DeclId) -> EnumLayout {
        let variants = design.decl_data(decl).children.iter().copied().filter(|v| matches!(design.decl_data(*v).kind, DeclKind::Variant(_))).collect();
        EnumLayout { variants }
    }

    /// `ceil(log2 V)`: 0 for an enum of one variant, whose tag has no level.
    pub(super) fn width(&self) -> u32 {
        let v = self.variants.len().max(1) as u64;
        u64::BITS - (v - 1).leading_zeros()
    }

    /// The tag's IR type; `None` for an enum of one variant.
    pub(super) fn ty(&self) -> Option<ValueType> {
        let width = self.width();
        (width > 0).then_some(ValueType::UInt(width))
    }

    /// The tag of `variant`, its position among the enum's variants.
    pub(super) fn tag_of(&self, variant: DeclId) -> Option<i128> {
        self.variants.iter().position(|v| *v == variant).map(|k| k as i128)
    }

    /// The tag of a variant declaration, through its enum.
    pub(super) fn variant_tag(design: &Design, variant: DeclId) -> Option<i128> {
        let parent = design.decl_data(variant).parent?;
        EnumLayout::of(design, parent).tag_of(variant)
    }
}

impl Elaboration<'_, '_> {
    /// `match scrutinee { arms }`, whose value has the base type `base`.
    pub(super) fn lower_match(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[Arm], base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        // Read once: a local, a param or a literal is read again where a test needs it; anything
        // else is bound to a local first.
        let option = matches!(self.types.ty(scrutinee), Some(Ty::Option(_)));
        let (subject, binding) = if option && scope.no_locals {
            (Subject::RereadPair(scrutinee), None)
        } else if option {
            let pair = self.lower_option(scope, scrutinee)?;
            let k = Self::match_locals(scope);
            let (is_ty, val_ty) = (self.type_of(pair.is), self.type_of(pair.val));
            let is = scope.locals.len() as u32;
            scope.locals.push(Local { name: format!("match${k}.is"), ty: is_ty.clone() });
            scope.locals.push(Local { name: format!("match${k}.val"), ty: val_ty.clone() });
            let first = self.stmt(Stmt::Let { local: is, init: pair.is });
            let second = self.stmt(Stmt::Let { local: is + 1, init: pair.val });
            (Subject::Pair { is: (is, is_ty), val: (is + 1, val_ty) }, Some(vec![first, second]))
        } else if scope.no_locals || self.simple_read(scrutinee) {
            (Subject::Reread(scrutinee), None)
        } else {
            let lowered = self.lower_expr(scope, scrutinee)?;
            let ty = self.type_of(lowered);
            let index = scope.locals.len() as u32;
            let name = format!("match${}", scope.locals.iter().filter(|l| l.name.starts_with("match$")).count());
            scope.locals.push(Local { name, ty: ty.clone() });
            (Subject::Local(index, ty), Some(vec![self.stmt(Stmt::Let { local: index, init: lowered })]))
        };
        scope.nesting += 1;
        let chain = self.arms_chain(scope, &subject, arms, &base, &origin);
        scope.nesting -= 1;
        let chain = chain?;
        Some(match binding {
            Some(stmts) => {
                let ty = self.type_of(chain);
                self.emit(Expr::Block { stmts, value: Some(chain) }, ty, origin)
            }
            None => chain,
        })
    }

    /// How many scrutinees of `match` the owner has bound so far: the `k` of the next `match$k`.
    fn match_locals(scope: &Scope) -> usize {
        scope.locals.iter().filter(|l| l.name.starts_with("match$") && !l.name.ends_with(".val")).count()
    }

    /// A fresh read of the scrutinee.
    fn read_subject(&mut self, scope: &mut Scope, subject: &Subject, origin: Origin) -> Option<LoweredExpr> {
        match subject {
            Subject::Reread(expr) => self.lower_expr(scope, *expr),
            Subject::Local(index, ty) => Some(self.emit(Expr::Local(*index), ty.clone(), origin)),
            // An `Option` is read through its patterns (`option_test`), never as one value.
            Subject::Pair { .. } | Subject::RereadPair(_) => None,
        }
    }

    /// A fresh read of a local.
    fn read_local(&mut self, (index, ty): &(u32, ValueType), origin: Origin) -> LoweredExpr {
        self.emit(Expr::Local(*index), ty.clone(), origin)
    }

    /// A fresh read of the flag (`payload` false) or of the payload of an `Option` subject.
    fn read_pair(&mut self, scope: &mut Scope, subject: &Subject, payload: bool, origin: Origin) -> Option<LoweredExpr> {
        match subject {
            Subject::Pair { is, val } => Some(self.read_local(if payload { val } else { is }, origin)),
            Subject::RereadPair(expr) => {
                let pair = self.lower_option(scope, *expr)?;
                Some(if payload { pair.val } else { pair.is })
            }
            Subject::Reread(_) | Subject::Local(..) => None,
        }
    }

    /// The test of a pattern over an `Option` scrutinee: `None` is `!is`; `Some(p)` is `is`, and
    /// `is` and `val == v` when `p` names a value `v`.
    fn option_test(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId) -> Option<LoweredExpr> {
        let node = self.design.pat_node(pat).clone();
        let origin = self.locator.origin(scope.file, node.span);
        let flag_is = self.read_pair(scope, subject, false, origin.clone())?;
        let res = match &node.kind {
            PatKind::Path(path) => self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res),
            PatKind::Ident { res, .. } => *res,
            _ => Resolution::Unresolved,
        };
        if matches!(res, Resolution::Builtin(id) if id.info().name == "None") {
            let ty = self.flag(flag_is).wrap(ValueType::Bool);
            return Some(self.emit(Expr::Unary { op: UnaryOp::Not, expr: flag_is }, ty, origin));
        }
        let PatKind::TupleStruct { pats, .. } = &node.kind else {
            self.unsupported(scope.file, node.span, "this pattern of an `Option`");
            return None;
        };
        let [inner] = pats.as_slice() else { return None };
        let inner_node = self.design.pat_node(*inner).clone();
        let value = match &inner_node.kind {
            PatKind::Wild | PatKind::Ident { res: Resolution::Unresolved, sub: None, .. } => return Some(flag_is),
            PatKind::Lit(expr) => self.lower_expr(scope, *expr)?,
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                let base = self.payload_base(scope, subject, origin.clone())?;
                self.path_value(scope, res, &base, inner_node.span, origin.clone())?
            }
            PatKind::Ident { res: res @ Resolution::Decl(_), sub: None, .. } => {
                let base = self.payload_base(scope, subject, origin.clone())?;
                self.path_value(scope, *res, &base, inner_node.span, origin.clone())?
            }
            _ => {
                self.unsupported(scope.file, inner_node.span, "this pattern inside `Some`");
                return None;
            }
        };
        let payload = self.read_pair(scope, subject, true, origin.clone())?;
        let equal = self.equals(payload, value, origin.clone());
        let flag = self.flags(&[flag_is, equal]);
        let op = if flag == Flag::Flagged { BinaryOp::BitAnd } else { BinaryOp::And };
        Some(self.emit(Expr::Binary { op, lhs: flag_is, rhs: equal }, flag.wrap(ValueType::Bool), origin))
    }

    /// The type of the payload of an `Option` subject, as `val` has it.
    fn payload_base(&mut self, scope: &mut Scope, subject: &Subject, origin: Origin) -> Option<ValueType> {
        match subject {
            Subject::Pair { val, .. } => Some(val.1.clone()),
            _ => {
                let val = self.read_pair(scope, subject, true, origin)?;
                Some(self.type_of(val))
            }
        }
    }

    /// The body of a `match` arm; under `Some(x)` over an `Option`, `let x = val;` first.
    fn arm_body(&mut self, scope: &mut Scope, subject: &Subject, arm: &Arm) -> Option<LoweredExpr> {
        if let Subject::RereadPair(_) = subject
            && let PatKind::TupleStruct { pats, .. } = &self.design.pat_node(arm.pat).kind
            && let [inner] = pats.as_slice()
            && let PatKind::Ident { res: Resolution::Unresolved, sub: None, .. } = self.design.pat_node(*inner).kind
        {
            let span = self.design.pat_node(*inner).span;
            self.unsupported(scope.file, span, "a binding inside `Some` in a definition");
            return None;
        }
        let Subject::Pair { val, .. } = subject else { return self.lower_expr(scope, arm.body) };
        let binding = match &self.design.pat_node(arm.pat).kind {
            PatKind::TupleStruct { pats, .. } => match pats.as_slice() {
                [inner] => match self.design.pat_node(*inner).kind {
                    PatKind::Ident { binding, res: Resolution::Unresolved, sub: None, .. } => Some(binding),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let Some(binding) = binding else { return self.lower_expr(scope, arm.body) };
        let origin = self.expr_origin(arm.body);
        let read = self.read_local(val, origin.clone());
        let local = self.bind_local(scope, binding)?;
        scope.locals[local as usize].ty = self.type_of(read);
        let stmt = self.stmt(Stmt::Let { local, init: read });
        let body = self.lower_expr(scope, arm.body)?;
        let ty = self.type_of(body);
        Some(self.emit(Expr::Block { stmts: vec![stmt], value: Some(body) }, ty, origin))
    }

    /// The `if` chain of `arms` over `subject`, from the last arm back: the last arm, or the
    /// first that matches everything, is the final value.
    fn arms_chain(&mut self, scope: &mut Scope, subject: &Subject, arms: &[Arm], base: &ValueType, origin: &Origin) -> Option<LoweredExpr> {
        let mut tests = Vec::new();
        let mut bodies = Vec::new();
        let mut ok = true;
        for (k, arm) in arms.iter().enumerate() {
            let last = k + 1 == arms.len();
            let test = if last { None } else { self.pattern_test(scope, subject, arm.pat) };
            let everything = !last && test.is_none() && self.matches_all(arm.pat);
            if !last && test.is_none() && !everything {
                ok = false;
            }
            match self.arm_body(scope, subject, arm) {
                Some(body) => bodies.push(body),
                None => ok = false,
            }
            tests.push(test);
            if everything {
                break;
            }
        }
        if !ok {
            return None;
        }
        let mut value = bodies.pop()?;
        tests.pop();
        while let (Some(body), Some(test)) = (bodies.pop(), tests.pop()) {
            let test = test?;
            let ty = self.flags(&[test, body, value]).wrap(base.clone());
            value = self.emit(Expr::If { cond: test, then: body, otherwise: value }, ty, origin.clone());
        }
        Some(value)
    }

    /// Whether pattern `pat` matches every value (`_`, or an or-pattern holding one), so no arm
    /// after it is reached.
    fn matches_all(&self, pat: PatId) -> bool {
        match &self.design.pat_node(pat).kind {
            PatKind::Wild => true,
            PatKind::Or(alternatives) => alternatives.iter().any(|a| self.matches_all(*a)),
            _ => false,
        }
    }

    /// The test pattern `pat` makes of `subject`: a `bool`, flagged when `subject` is; `None` for
    /// a pattern that matches everything, or after a diagnostic.
    fn pattern_test(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId) -> Option<LoweredExpr> {
        let node = self.design.pat_node(pat).clone();
        let origin = self.locator.origin(scope.file, node.span);
        if self.matches_all(pat) {
            return None;
        }
        if let Subject::Pair { .. } | Subject::RereadPair(_) = subject {
            return self.option_test(scope, subject, pat);
        }
        if let PatKind::Or(alternatives) = &node.kind {
            let mut tests = Vec::new();
            for alt in alternatives {
                tests.push(self.pattern_test(scope, subject, *alt)?);
            }
            let flag = self.flags(&tests);
            let op = if flag == Flag::Flagged { BinaryOp::BitOr } else { BinaryOp::Or };
            let mut tests = tests.into_iter();
            let first = tests.next()?;
            return Some(tests.fold(first, |lhs, rhs| {
                let ty = self.flags(&[lhs, rhs]).wrap(ValueType::Bool);
                self.emit(Expr::Binary { op, lhs, rhs }, ty, origin.clone())
            }));
        }
        let subject = self.read_subject(scope, subject, origin.clone())?;
        let base = match self.type_of(subject) {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        let flag = self.flag(subject);
        match &node.kind {
            // `true` is the subject itself, `false` its negation.
            PatKind::Lit(expr) => match self.bool_literal(*expr) {
                Some(true) => Some(subject),
                Some(false) => Some(self.emit(Expr::Unary { op: UnaryOp::Not, expr: subject }, flag.wrap(ValueType::Bool), origin)),
                None => {
                    let value = self.lower_expr(scope, *expr)?;
                    Some(self.equals(subject, value, origin))
                }
            },
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                let value = self.path_value(scope, res, &base, node.span, origin.clone())?;
                Some(self.equals(subject, value, origin))
            }
            PatKind::Ident { res: res @ (Resolution::Decl(_) | Resolution::Builtin(_)), sub: None, .. } => {
                let value = self.path_value(scope, *res, &base, node.span, origin.clone())?;
                Some(self.equals(subject, value, origin))
            }
            _ => {
                self.unsupported(scope.file, node.span, "this pattern");
                None
            }
        }
    }

    /// The value a path pattern names: a unit variant's tag or a constant, of the subject's type.
    fn path_value(&mut self, scope: &Scope, res: Resolution, base: &ValueType, span: crate::msl::Span, origin: Origin) -> Option<LoweredExpr> {
        match res {
            Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Variant(_)) => {
                let tag = EnumLayout::variant_tag(self.design, d)?;
                Some(self.emit(Expr::Int(tag), base.clone(), origin))
            }
            Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Const(_)) => match self.consts.constant(d) {
                Ok(value) => Some(self.constant_expr(value, base.clone(), origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const` pattern");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, span, "this name as a pattern");
                None
            }
        }
    }

    /// `subject == value`, flagged when `subject` is.
    fn equals(&mut self, subject: LoweredExpr, value: LoweredExpr, origin: Origin) -> LoweredExpr {
        let ty = self.flags(&[subject, value]).wrap(ValueType::Bool);
        self.emit(Expr::Binary { op: BinaryOp::Eq, lhs: subject, rhs: value }, ty, origin)
    }

    /// The value of a `bool` literal pattern; `None` for any other literal.
    fn bool_literal(&self, expr: ExprId) -> Option<bool> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => match lit.kind {
                LitKind::Bool(b) => Some(b),
                _ => None,
            },
            ExprKind::Paren(inner) => self.bool_literal(*inner),
            _ => None,
        }
    }

    /// Whether `expr` reads a local, a param or a literal: reading it again costs nothing and
    /// gives the same value.
    fn simple_read(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.simple_read(*inner),
            ExprKind::Lit(lit) => matches!(lit.kind, LitKind::Int(_) | LitKind::Float { .. } | LitKind::Bool(_)),
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Param(_) | DeclKind::FnParam(_) | DeclKind::Binding(_) | DeclKind::Const(_) | DeclKind::Variant(_))),
            _ => false,
        }
    }

    // ---- Option and tuples ----------------------------------------------------------------

    /// The type of the payload of the `Option` expression `expr`.
    fn payload_type(&self, expr: ExprId) -> Option<ValueType> {
        match self.types.ty(expr) {
            Some(Ty::Option(inner)) => self.value_type(inner),
            // An argument read as `Some` of itself (§3.2.3) keeps its own type.
            Some(other) if self.types.implicit_some(expr) => self.value_type(other),
            _ => None,
        }
    }

    /// The known zero of `ty`: the payload of `None`.
    fn zero(&mut self, ty: &ValueType, origin: Origin) -> Option<LoweredExpr> {
        let kind = match ty {
            ValueType::F64 => Expr::Real(piperine_ir::Real::new(0.0)),
            ValueType::Bool => Expr::Bool(false),
            ValueType::UInt(_) | ValueType::Int(_) => Expr::Int(0),
            _ => return None,
        };
        Some(self.emit(kind, ty.clone(), origin))
    }

    /// The two arguments an `Option<T>` parameter takes in the IR, `is` then `val`: the pair of the
    /// bound argument, or `(false, zero)` for one left out (§3.2.4). `None` after a diagnostic.
    pub(super) fn lower_option_slot(&mut self, scope: &mut Scope, slot: Option<ExprId>, payload: &Ty, at: ExprId) -> Option<[LoweredExpr; 2]> {
        if let Some(arg) = slot {
            let pair = self.lower_option(scope, arg)?;
            return Some([pair.is, pair.val]);
        }
        let origin = self.expr_origin(at);
        let Some(payload) = self.value_type(payload) else {
            self.unsupported(scope.file, self.design.expr_node(at).span, "an `Option` parameter whose payload has no value in the IR");
            return None;
        };
        let is = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
        let Some(val) = self.zero(&payload, origin) else {
            self.unsupported(scope.file, self.design.expr_node(at).span, "an `Option` parameter whose payload has no zero in the IR");
            return None;
        };
        Some([is, val])
    }

    /// The pair an `Option` expression lowers to. `None` after a diagnostic.
    fn lower_option(&mut self, scope: &mut Scope, expr: ExprId) -> Option<Pair> {
        let node = self.design.expr_node(expr).clone();
        let origin = self.expr_origin(expr);
        let (file, span) = (scope.file, node.span);
        let unsupported = |this: &mut Self| {
            this.unsupported(file, span, "this form of an `Option` value");
            None
        };
        if self.types.implicit_some(expr) {
            let val = self.lower_expr(scope, expr)?;
            let is = self.emit(Expr::Bool(true), ValueType::Bool, origin);
            return Some(Pair { is, val, literal: Some(true) });
        }
        let Some(payload) = self.payload_type(expr) else {
            self.unsupported(scope.file, node.span, "an `Option` whose payload has no value in the IR");
            return None;
        };
        match &node.kind {
            ExprKind::Paren(inner) => self.lower_option(scope, *inner),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
                Resolution::Builtin(id) if id.info().name == "None" => {
                    let is = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
                    let Some(val) = self.zero(&payload, origin) else { return unsupported(self) };
                    Some(Pair { is, val, literal: Some(false) })
                }
                Resolution::Decl(decl) => match (scope.options.get(&decl).copied(), self.input_of(scope, decl)) {
                    (Some((is, val)), _) => {
                        let (is_ty, val_ty) = (scope.locals[is as usize].ty.clone(), scope.locals[val as usize].ty.clone());
                        let is = self.emit(Expr::Local(is), is_ty, origin.clone());
                        let val = self.emit(Expr::Local(val), val_ty, origin);
                        Some(Pair { is, val, literal: None })
                    }
                    // An `Option` value input of a hoisted module: the caller's argument, in the
                    // caller's frame (an `f64` given there is `Some` of it, §3.2.3).
                    (None, Some(Some(given))) => self.in_caller(scope, |this, scope| this.lower_option(scope, given)),
                    // A trailing `Option` input left out reads as `None` (DIG-10 AC 3).
                    (None, Some(None)) => {
                        let is = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
                        let Some(val) = self.zero(&payload, origin) else { return unsupported(self) };
                        Some(Pair { is, val, literal: Some(false) })
                    }
                    // A body `let` or a `const` of `Option` type reads its initializer where it is
                    // read: it holds only literals and consts, so every read has the same levels
                    // (§3.3.9)
                    (None, None) => match self.body_let_value(decl).or_else(|| self.const_value(decl)) {
                        Some(value) => self.lower_option(scope, value),
                        None => unsupported(self),
                    },
                },
                _ => unsupported(self),
            },
            ExprKind::Call { args, .. } if matches!(self.types.ty(expr), Some(Ty::Option(_))) && self.is_some_call(expr) => {
                let [arg] = args.as_slice() else { return unsupported(self) };
                let val = self.lower_expr(scope, arg.value)?;
                let is = self.emit(Expr::Bool(true), ValueType::Bool, origin);
                Some(Pair { is, val, literal: Some(true) })
            }
            // `x.known()`: the two flag-total reads of `x`, each reading `x` where it stands.
            ExprKind::MethodCall { recv, .. } if self.types.builtin(expr) == Some(Builtin::Known) => {
                let read = self.lower_expr(scope, *recv)?;
                let is = self.emit(Expr::IsKnown(read), ValueType::Bool, origin.clone());
                let read = self.lower_expr(scope, *recv)?;
                let Some(zero) = self.zero(&payload, origin.clone()) else { return unsupported(self) };
                let val = self.emit(Expr::KnownOr { value: read, default: zero }, payload, origin);
                Some(Pair { is, val, literal: None })
            }
            // `a.checked_add(b)`: `None` when the wrapping operation overflowed (H.1), so the flag is
            // `!overflows(a, b)` and the payload is the wrapping result, each reading `a` and `b`
            // where they stand.
            ExprKind::MethodCall { recv, args, .. } if matches!(self.types.method(expr), Some(Method::CheckedAdd | Method::CheckedSub | Method::CheckedMul)) => {
                let (overflows, op) = match self.types.method(expr)? {
                    Method::CheckedAdd => (IrMethod::AddOverflows, BinaryOp::Add),
                    Method::CheckedSub => (IrMethod::SubOverflows, BinaryOp::Sub),
                    _ => (IrMethod::MulOverflows, BinaryOp::Mul),
                };
                let [arg] = args.as_slice() else { return unsupported(self) };
                let (a, b) = (self.lower_expr(scope, *recv)?, self.lower_expr(scope, arg.value)?);
                let flags = self.flags(&[a, b]);
                let overflowed = self.emit(Expr::Method { method: overflows, receiver: a, args: vec![b] }, flags.wrap(ValueType::Bool), origin.clone());
                let is = self.emit(Expr::Unary { op: UnaryOp::Not, expr: overflowed }, flags.wrap(ValueType::Bool), origin.clone());
                let (a, b) = (self.lower_expr(scope, *recv)?, self.lower_expr(scope, arg.value)?);
                let val = self.emit(Expr::Binary { op, lhs: a, rhs: b }, flags.wrap(payload), origin);
                Some(Pair { is, val, literal: None })
            }
            // `o.map(|x| e)`, `o.and_then(|x| p)`, `a.or(b)`, `c.then(|| v)`, `c.then_some(v)` (H.3, H.4).
            ExprKind::MethodCall { recv, args, .. } if matches!(self.types.method(expr), Some(Method::Map | Method::AndThen | Method::Or | Method::Then | Method::ThenSome)) => {
                let method = self.types.method(expr)?;
                let [arg] = args.as_slice() else { return unsupported(self) };
                self.lower_option_row(scope, method, *recv, arg.value, payload, origin)
            }
            // `if c { a } else { b }` of `Option`s: the flag and the payload each an `if` on `c`.
            ExprKind::If { cond, then, else_: Some(else_) } if !matches!(self.design.expr_node(*cond).kind, ExprKind::Let { .. }) => {
                let [stmt] = then.stmts.as_slice() else { return unsupported(self) };
                let crate::pom::StmtKind::Expr { expr: then_value, semi: false } = self.design.stmt_node(*stmt).kind.clone() else { return unsupported(self) };
                let first = self.lower_expr(scope, *cond)?;
                scope.nesting += 1;
                let (then, otherwise) = (self.lower_option(scope, then_value), self.lower_option(scope, *else_));
                scope.nesting -= 1;
                let (then, otherwise) = (then?, otherwise?);
                let second = self.lower_expr(scope, *cond)?;
                let is_ty = self.flags(&[first, then.is, otherwise.is]).wrap(ValueType::Bool);
                let val_ty = self.flags(&[second, then.val, otherwise.val]).wrap(payload);
                let is = self.emit(Expr::If { cond: first, then: then.is, otherwise: otherwise.is }, is_ty, origin.clone());
                let val = self.emit(Expr::If { cond: second, then: then.val, otherwise: otherwise.val }, val_ty, origin);
                Some(Pair { is, val, literal: None })
            }
            ExprKind::Block(block) => match block.stmts.as_slice() {
                [stmt] => match self.design.stmt_node(*stmt).kind.clone() {
                    crate::pom::StmtKind::Expr { expr: value, semi: false } => self.lower_option(scope, value),
                    _ => unsupported(self),
                },
                _ => unsupported(self),
            },
            _ => unsupported(self),
        }
    }

    /// The initializer of the `const` item `decl`; `None` for any other declaration or a `const`
    /// without a value.
    fn const_value(&self, decl: DeclId) -> Option<ExprId> {
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(data) => data.value,
            _ => None,
        }
    }

    /// Whether `expr` is a call of the built-in `Some`.
    pub(super) fn is_some_call(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Call { callee, .. } => matches!(&self.design.expr_node(*callee).kind, ExprKind::Path(path) if matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Builtin(id)) if id.info().name == "Some")),
            _ => false,
        }
    }

    /// `o.is_some()`, `o.is_none()`, `o.unwrap_or(d)` over the pair of `o`.
    pub(super) fn lower_option_method(&mut self, scope: &mut Scope, method: Method, recv: ExprId, args: &[ExprId], base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let pair = self.lower_option(scope, recv)?;
        match method {
            Method::IsSome => Some(pair.is),
            Method::IsNone => {
                let ty = self.flag(pair.is).wrap(ValueType::Bool);
                Some(self.emit(Expr::Unary { op: UnaryOp::Not, expr: pair.is }, ty, origin))
            }
            Method::UnwrapOr => {
                let [default] = args else { return None };
                match pair.literal {
                    // A literal `Some(v)` is `v`, a literal `None` the default.
                    Some(true) => Some(pair.val),
                    Some(false) => self.lower_expr(scope, *default),
                    None => {
                        scope.nesting += 1;
                        let default = self.lower_expr(scope, *default);
                        scope.nesting -= 1;
                        let default = default?;
                        let ty = self.flags(&[pair.is, pair.val, default]).wrap(base);
                        Some(self.emit(Expr::If { cond: pair.is, then: pair.val, otherwise: default }, ty, origin))
                    }
                }
            }
            Method::Unwrap | Method::Expect => {
                let message = match (method, args) {
                    (Method::Expect, [message]) => match &self.design.expr_node(*message).kind {
                        ExprKind::Lit(lit) => lit.str_value(),
                        _ => None,
                    },
                    _ => None,
                };
                if method == Method::Expect && message.is_none() {
                    self.unsupported(scope.file, self.design.expr_node(recv).span, "`expect` with a message that is not a string literal");
                    return None;
                }
                if self.flag(pair.is) == Flag::Flagged {
                    self.unsupported(scope.file, self.design.expr_node(recv).span, "`unwrap` of an `Option` whose presence may be unknown");
                    return None;
                }
                match pair.literal {
                    // A literal `Some(v)` is `v`; a literal `None` always faults, so it stays an `Unwrap`.
                    Some(true) => Some(pair.val),
                    _ => {
                        let ty = self.type_of(pair.val);
                        Some(self.emit(Expr::Unwrap { is: pair.is, value: pair.val, message }, ty, origin))
                    }
                }
            }
            Method::UnwrapOrElse => {
                let [closure] = args else { return None };
                let (_, body) = self.closure_parts(scope, *closure)?;
                match pair.literal {
                    Some(true) => Some(pair.val),
                    _ => {
                        scope.nesting += 1;
                        let default = self.lower_expr(scope, body);
                        scope.nesting -= 1;
                        let default = default?;
                        let ty = self.flags(&[pair.is, pair.val, default]).wrap(base);
                        Some(self.emit(Expr::If { cond: pair.is, then: pair.val, otherwise: default }, ty, origin))
                    }
                }
            }
            _ => None,
        }
    }

    /// The parameter binding and the body of the closure literal `closure` (`|x| body` gives
    /// `Some(x)`, `|| body` gives `None`).
    fn closure_parts(&mut self, scope: &mut Scope, closure: ExprId) -> Option<(Option<DeclId>, ExprId)> {
        let node = self.design.expr_node(closure).clone();
        let ExprKind::Closure { params, body, .. } = node.kind else {
            self.unsupported(scope.file, node.span, "a closure argument that is not a closure literal");
            return None;
        };
        match params.as_slice() {
            [] => Some((None, body)),
            [param] => match self.design.pat_node(param.pat).kind.clone() {
                PatKind::Ident { binding, sub: None, .. } => Some((Some(binding), body)),
                PatKind::Wild => Some((None, body)),
                _ => {
                    self.unsupported(scope.file, node.span, "a closure parameter that is not a name");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, node.span, "a closure of more than one parameter");
                None
            }
        }
    }

    /// `let binding = value;` followed by `lower(scope)`, as one block: the closure body of a
    /// row, its parameter bound to the payload.
    fn closure_block(&mut self, scope: &mut Scope, binding: Option<DeclId>, local: Option<u32>, value: LoweredExpr, span: crate::msl::Span, lower: impl FnOnce(&mut Self, &mut Scope) -> Option<LoweredExpr>) -> Option<LoweredExpr> {
        let origin = self.locator.origin(scope.file, span);
        let mut stmts = Vec::new();
        if binding.is_some() {
            let local = local?;
            stmts.push(self.stmt(Stmt::Let { local, init: value }));
        }
        scope.nesting += 1;
        let body = lower(self, scope);
        scope.nesting -= 1;
        let body = body?;
        let ty = self.type_of(body);
        Some(self.emit(Expr::Block { stmts, value: Some(body) }, ty, origin))
    }

    /// The pair an `Option`-valued row lowers to. The closure of a row runs only when its
    /// condition holds (Rust runs it for `Some` only): its body sits under an `if` on the
    /// flag, `None`'s payload being the zero.
    fn lower_option_row(&mut self, scope: &mut Scope, method: Method, recv: ExprId, arg: ExprId, payload: ValueType, origin: Origin) -> Option<Pair> {
        let span = self.design.expr_node(arg).span;
        match method {
            Method::Or => {
                let (a, b) = (self.lower_option(scope, recv)?, self.lower_option(scope, arg)?);
                let yes = self.emit(Expr::Bool(true), ValueType::Bool, origin.clone());
                let is_ty = self.flags(&[a.is, b.is]).wrap(ValueType::Bool);
                let is = self.emit(Expr::If { cond: a.is, then: yes, otherwise: b.is }, is_ty, origin.clone());
                let val_ty = self.flags(&[a.is, a.val, b.val]).wrap(payload);
                let val = self.emit(Expr::If { cond: a.is, then: a.val, otherwise: b.val }, val_ty, origin);
                Some(Pair { is, val, literal: None })
            }
            Method::ThenSome => {
                // `(c, v)`: `v` is evaluated as Rust evaluates an argument.
                let is = self.lower_expr(scope, recv)?;
                let val = self.lower_expr(scope, arg)?;
                Some(Pair { is, val, literal: None })
            }
            Method::Then => {
                let (_, body) = self.closure_parts(scope, arg)?;
                let is = self.lower_expr(scope, recv)?;
                scope.nesting += 1;
                let value = self.lower_expr(scope, body);
                scope.nesting -= 1;
                let value = value?;
                let Some(zero) = self.zero(&payload, origin.clone()) else {
                    self.unsupported(scope.file, span, "an `Option` whose payload has no zero in the IR");
                    return None;
                };
                let val_ty = self.flags(&[is, value]).wrap(payload);
                let val = self.emit(Expr::If { cond: is, then: value, otherwise: zero }, val_ty, origin);
                Some(Pair { is, val, literal: None })
            }
            _ => {
                // `map` and `and_then`: the closure's parameter is a local bound to the payload.
                let pair = self.lower_option(scope, recv)?;
                let (binding, body) = self.closure_parts(scope, arg)?;
                let local = match binding {
                    Some(binding) => {
                        let local = self.bind_local(scope, binding)?;
                        scope.locals[local as usize].ty = self.type_of(pair.val);
                        Some(local)
                    }
                    None => None,
                };
                let zero_of = |this: &mut Self, ty: &ValueType| this.zero(ty, origin.clone());
                if method == Method::Map {
                    let value = self.closure_block(scope, binding, local, pair.val, span, |this, scope| this.lower_expr(scope, body))?;
                    let guarded = match pair.literal {
                        Some(true) => value,
                        _ => {
                            let Some(zero) = zero_of(self, &payload) else {
                                self.unsupported(scope.file, span, "an `Option` whose payload has no zero in the IR");
                                return None;
                            };
                            let ty = self.flags(&[pair.is, value]).wrap(payload);
                            self.emit(Expr::If { cond: pair.is, then: value, otherwise: zero }, ty, origin)
                        }
                    };
                    return Some(Pair { is: pair.is, val: guarded, literal: pair.literal });
                }
                // `and_then`: the pair of the closure's `Option`, each half under the binding, and
                // both only when the receiver is `Some`.
                let is_inner = self.closure_block(scope, binding, local, pair.val, span, |this, scope| this.lower_option(scope, body).map(|p| p.is))?;
                let val_inner = self.closure_block(scope, binding, local, pair.val, span, |this, scope| this.lower_option(scope, body).map(|p| p.val))?;
                let (Some(no), Some(zero)) = (Some(self.emit(Expr::Bool(false), ValueType::Bool, origin.clone())), zero_of(self, &payload)) else {
                    self.unsupported(scope.file, span, "an `Option` whose payload has no zero in the IR");
                    return None;
                };
                let is_ty = self.flags(&[pair.is, is_inner]).wrap(ValueType::Bool);
                let is = self.emit(Expr::If { cond: pair.is, then: is_inner, otherwise: no }, is_ty, origin.clone());
                let val_ty = self.flags(&[pair.is, val_inner]).wrap(payload);
                let val = self.emit(Expr::If { cond: pair.is, then: val_inner, otherwise: zero }, val_ty, origin);
                Some(Pair { is, val, literal: None })
            }
        }
    }

    /// `let o = e;` of type `Option<T>`: the flag to the local `o.is`, the payload to `o.val`.
    pub(super) fn lower_option_let(&mut self, scope: &mut Scope, out: &mut Vec<StmtId>, binding: DeclId, init: ExprId) -> bool {
        let name = self.design.decl_data(binding).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(pair) = self.lower_option(scope, init) else { return false };
        let is = scope.locals.len() as u32;
        scope.locals.push(Local { name: format!("{name}.is"), ty: self.type_of(pair.is) });
        scope.locals.push(Local { name: format!("{name}.val"), ty: self.type_of(pair.val) });
        out.push(self.stmt(Stmt::Let { local: is, init: pair.is }));
        out.push(self.stmt(Stmt::Let { local: is + 1, init: pair.val }));
        scope.options.insert(binding, (is, is + 1));
        true
    }

    /// `let (a, b) = (x, y);`: one local per component, in order.
    pub(super) fn lower_tuple_let(&mut self, scope: &mut Scope, out: &mut Vec<StmtId>, pats: &[PatId], init: ExprId, file: FileId, span: crate::msl::Span) -> bool {
        let ExprKind::Tuple(items) = self.design.expr_node(self.peel_parens(init)).kind.clone() else {
            self.unsupported(file, span, "a tuple `let` whose value is not a tuple expression");
            return false;
        };
        if items.len() != pats.len() {
            self.unsupported(file, span, "a tuple `let` of another number of components");
            return false;
        }
        let mut ok = true;
        for (pat, item) in pats.iter().zip(items) {
            match self.design.pat_node(*pat).kind.clone() {
                PatKind::Ident { binding, sub: None, .. } => {
                    let local = self.bind_local(scope, binding);
                    let init = self.lower_expr(scope, item);
                    match (local, init) {
                        (Some(local), Some(init)) => {
                            scope.locals[local as usize].ty = self.type_of(init);
                            out.push(self.stmt(Stmt::Let { local, init }));
                        }
                        _ => ok = false,
                    }
                }
                _ => {
                    let at = self.design.pat_node(*pat).span;
                    self.unsupported(file, at, "a tuple `let` component that is not a name");
                    ok = false;
                }
            }
        }
        ok
    }

    /// `if let Some(p) = o { A } else { B }`: `if o.is { let p = o.val; A } else { B }`.
    pub(super) fn lower_if_let(&mut self, scope: &mut Scope, cond: ExprId, then: &crate::pom::BlockNode, else_: Option<ExprId>, base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let ExprKind::Let { pat, expr } = self.design.expr_node(cond).kind.clone() else { return None };
        let pair = self.lower_option(scope, expr)?;
        let binding = self.some_binding(scope.file, pat)?;
        scope.nesting += 1;
        let then = self.bound_block(scope, binding, pair.val, &then.stmts, then.span);
        let otherwise = match else_ {
            Some(else_) => self.lower_expr(scope, else_),
            None => Some(self.emit(Expr::Unit, ValueType::Unit, origin.clone())),
        };
        scope.nesting -= 1;
        let (then, otherwise) = (then?, otherwise?);
        let ty = self.flags(&[pair.is, then, otherwise]).wrap(base);
        Some(self.emit(Expr::If { cond: pair.is, then, otherwise }, ty, origin))
    }

    /// The binding of `Some(p)`: `Some(Some(decl))` for a name, `Some(None)` for `_`; `None`
    /// after a diagnostic.
    fn some_binding(&mut self, file: FileId, pat: PatId) -> Option<Option<DeclId>> {
        let node = self.design.pat_node(pat).clone();
        let inner = match &node.kind {
            PatKind::TupleStruct { pats, .. } if pats.len() == 1 => pats[0],
            _ => {
                self.unsupported(file, node.span, "an `if let` pattern other than `Some(p)`");
                return None;
            }
        };
        match self.design.pat_node(inner).kind.clone() {
            PatKind::Ident { binding, sub: None, res: Resolution::Unresolved, .. } => Some(Some(binding)),
            PatKind::Wild => Some(None),
            _ => {
                let span = self.design.pat_node(inner).span;
                self.unsupported(file, span, "a pattern inside `Some` that is not a name or `_`");
                None
            }
        }
    }

    /// The block `stmts` with `let binding = value;` before its statements.
    fn bound_block(&mut self, scope: &mut Scope, binding: Option<DeclId>, value: LoweredExpr, stmts: &[crate::pom::StmtId], span: crate::msl::Span) -> Option<LoweredExpr> {
        let origin = self.locator.origin(scope.file, span);
        let mut lowered = Vec::new();
        if let Some(binding) = binding {
            let local = self.bind_local(scope, binding)?;
            scope.locals[local as usize].ty = self.type_of(value);
            lowered.push(self.stmt(Stmt::Let { local, init: value }));
        }
        let (rest, result) = self.lower_statements(scope, stmts)?;
        lowered.extend(rest);
        let ty = result.map_or(ValueType::Unit, |v| self.type_of(v));
        Some(self.emit(Expr::Block { stmts: lowered, value: result }, ty, origin))
    }
}
