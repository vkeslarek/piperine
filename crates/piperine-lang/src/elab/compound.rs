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

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, LevelPart, Local, Method as IrMethod, Origin, Stmt, StmtId, UnaryOp, ValueType};

use super::elaborator::Elaboration;
use super::patterns::Subject;
use super::selectors::ArmCondition;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{Arm, DeclId, DeclKind, Design, ExprId, ExprKind, FileId, LitKind, PatId, PatKind, Resolution};
use crate::types::{Builtin, Method, Ty};

/// What one `match` arm asks: its test, guard included, and the names it binds.
struct ArmTest {
    test: Option<LoweredExpr>,
    names: Vec<(u32, LoweredExpr)>,
}

/// An `Option` value as the IR holds it: its flag and its payload. `literal` is the flag when
/// the source wrote `None` or `Some(..)`.
pub(super) struct Pair {
    pub(super) is: LoweredExpr,
    pub(super) val: LoweredExpr,
    pub(super) literal: Option<bool>,
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

    /// The tag of every variant, in declaration order: the patterns a cast to the enum decodes.
    pub(super) fn tags(&self) -> Vec<i128> {
        (0..self.variants.len() as i128).collect()
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
    /// The tests that select the arm of a `match` on `scrutinee` whose pattern is `pat` (`None` for
    /// the last arm, which has no test) after the arms with the patterns `earlier` did not: one
    /// `bool` per test, read in `scope`, a scope with no local, so the scrutinee is read again at
    /// each test (`selectors.rs`, a guarded site's `enabled`). `None` after a diagnostic.
    pub(super) fn standalone_arm_tests(&mut self, scope: &mut Scope, scrutinee: ExprId, earlier: &[PatId], pat: Option<PatId>) -> Option<Vec<LoweredExpr>> {
        let subject = if matches!(self.types.ty(scrutinee), Some(Ty::Option(_))) { Subject::RereadPair(scrutinee) } else { Subject::Reread(scrutinee) };
        let mut tests = Vec::new();
        for before in earlier {
            if let Some(test) = self.pattern_test(scope, &subject, *before) {
                let ty = self.flag(test).wrap(ValueType::Bool);
                tests.push(self.emit(Expr::Unary { op: UnaryOp::Not, expr: test }, ty, self.expr_origin(scrutinee)));
            }
        }
        if let Some(pat) = pat
            && let Some(test) = self.pattern_test(scope, &subject, pat)
        {
            tests.push(test);
        }
        Some(tests)
    }

    /// Whether `expr` reads a local, a param or a literal: reading it again costs nothing and
    /// gives the same value.
    pub(super) fn simple_read(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.simple_read(*inner),
            ExprKind::Lit(lit) => matches!(lit.kind, LitKind::Int(_) | LitKind::Float { .. } | LitKind::Bool(_)),
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Param(_) | DeclKind::FnParam(_) | DeclKind::Binding(_) | DeclKind::Const(_) | DeclKind::Variant(_))),
            _ => false,
        }
    }

    /// `match scrutinee { arms }`, whose value has the base type `base`.
    pub(super) fn lower_match(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[Arm], base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let (subject, binding) = self.subject_of(scope, scrutinee, false)?;
        let mut binding = binding;
        let chain = self.arms_chain(scope, scrutinee, &subject, arms, &base, &origin, &mut binding)?;
        if binding.is_empty() {
            return Some(chain);
        }
        let ty = self.type_of(chain);
        Some(self.emit(Expr::Block { stmts: binding, value: Some(chain) }, ty, origin))
    }

    /// The test of an arm, guard included, and the names its pattern binds; `None` after a
    /// diagnostic. The guard reads the names, which the `match` binds before its first test.
    fn test_arm(&mut self, scope: &mut Scope, subject: &Subject, arm: &Arm) -> Option<ArmTest> {
        let matched = self.test_pattern(scope, subject, arm.pat)?;
        let names = self.bind_names(scope, &matched.bindings)?;
        let test = match arm.guard {
            None => matched.test,
            Some(guard) => {
                let origin = self.expr_origin(guard);
                let held = self.lower_expr(scope, guard)?;
                Some(match matched.test {
                    Some(test) => self.both(test, held, origin),
                    None => held,
                })
            }
        };
        Some(ArmTest { test, names })
    }

    /// The body of an arm under the `let`s of the names its pattern binds.
    fn arm_body(&mut self, scope: &mut Scope, names: &[(u32, LoweredExpr)], body: ExprId) -> Option<LoweredExpr> {
        let lets = self.binding_lets(names);
        let lowered = self.lower_expr(scope, body)?;
        if lets.is_empty() {
            return Some(lowered);
        }
        let (ty, origin) = (self.type_of(lowered), self.expr_origin(body));
        let block = self.emit(Expr::Block { stmts: lets.clone(), value: Some(lowered) }, ty, origin);
        self.note_block(block, &lets, Some(lowered));
        Some(block)
    }

    /// The `if` chain of `arms` over `subject`, from the last arm back: the last arm, or the
    /// first that matches everything, is the final value. The names a guarded arm's pattern binds
    /// are `let`s pushed to `prelude`, before any test: each is a read of the subject.
    fn arms_chain(&mut self, scope: &mut Scope, scrutinee: ExprId, subject: &Subject, arms: &[Arm], base: &ValueType, origin: &Origin, prelude: &mut Vec<StmtId>) -> Option<LoweredExpr> {
        let mut tests = Vec::new();
        let mut bodies = Vec::new();
        let mut ok = true;
        // The body of arm `k` runs when no test before it held and its own does: the conditions
        // of the `if` chain this lowers to (§11, `Body::contributions`).
        let enclosing = scope.conditions.len();
        for (k, arm) in arms.iter().enumerate() {
            let last = k + 1 == arms.len();
            let Some(tested) = self.test_arm(scope, subject, arm) else {
                ok = false;
                continue;
            };
            if last && arm.guard.is_some() {
                let span = self.design.expr_node(arm.body).span;
                self.unsupported(scope.file, span, "a `match` whose last arm has a guard");
                ok = false;
                continue;
            }
            let test = if last { None } else { tested.test };
            let everything = !last && test.is_none();
            let own = Self::enter_arm(scope, test, true);
            let condition = match arm.guard {
                Some(_) => ArmCondition::Guarded,
                None => ArmCondition::MatchArm { scrutinee, earlier: arms[..k].iter().map(|a| a.pat).collect(), pat: (!last).then_some(arm.pat) },
            };
            scope.guards.push(condition);
            // The names of a guarded arm are bound before the first test, so its guard reads them;
            // those of any other arm are bound at the head of its body.
            let body = if arm.guard.is_some() {
                prelude.extend(self.binding_lets(&tested.names));
                self.lower_expr(scope, arm.body)
            } else {
                self.arm_body(scope, &tested.names, arm.body)
            };
            match body {
                Some(body) => bodies.push(self.fit_to(body, base)),
                None => ok = false,
            }
            scope.guards.pop();
            Self::leave_arm(scope, own);
            Self::enter_arm(scope, test, false);
            tests.push(test);
            if everything {
                break;
            }
        }
        scope.conditions.truncate(enclosing);
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

    /// The test patterns make of a `match` scrutinee, for `selectors.rs`: `None` for a pattern that
    /// matches everything or after a diagnostic.
    fn pattern_test(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId) -> Option<LoweredExpr> {
        self.test_pattern(scope, subject, pat).and_then(|matched| matched.test)
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
    pub(super) fn zero(&mut self, ty: &ValueType, origin: Origin) -> Option<LoweredExpr> {
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
    pub(super) fn lower_option(&mut self, scope: &mut Scope, expr: ExprId) -> Option<Pair> {
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
            // A call of a function that returns an `Option`: the pair of its two halves (`returns.rs`).
            ExprKind::Call { .. } if let Some(function) = self.calls_option_function(expr) => self.lower_option_call(scope, expr, function),
            // `x.known()`: the two flag-total reads of `x`, each reading `x` where it stands.
            ExprKind::MethodCall { recv, .. } if self.types.builtin(expr) == Some(Builtin::Known) => {
                let read = self.lower_expr(scope, *recv)?;
                let is = self.emit(Expr::IsKnown(read), ValueType::Bool, origin.clone());
                let read = self.lower_expr(scope, *recv)?;
                let Some(zero) = self.zero(&payload, origin.clone()) else { return unsupported(self) };
                let val = self.emit(Expr::KnownOr { value: read, default: zero }, payload, origin);
                Some(Pair { is, val, literal: None })
            }
            // `x.level(k)`: the pair `(is, value)` of the `Option<bool>` (§3.3.4), each half read
            // from the receiver and the index where they stand. Both halves are flagged only when
            // the index may be unknown (§3.3.13, §3.3.14).
            ExprKind::MethodCall { recv, args, .. } if self.types.builtin(expr) == Some(Builtin::Level) => {
                let [index] = args.as_slice() else { return unsupported(self) };
                let mut half = |this: &mut Self, part: LevelPart| {
                    let value = this.lower_expr(scope, *recv)?;
                    let index = this.lower_expr(scope, index.value)?;
                    let ty = this.flag(index).wrap(ValueType::Bool);
                    Some(this.emit(Expr::Level { value, index, part }, ty, origin.clone()))
                };
                let is = half(self, LevelPart::Is)?;
                let val = half(self, LevelPart::Value)?;
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
        if self.declared_mutable(binding) {
            let span = self.design.decl_data(binding).span;
            self.unsupported(scope.file, span, "a `let mut` of an `Option`");
            return false;
        }
        let name = self.design.decl_data(binding).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(pair) = self.lower_option(scope, init) else { return false };
        let is = scope.locals.len() as u32;
        scope.locals.push(Local { name: format!("{name}.is"), ty: self.type_of(pair.is), mutable: false });
        scope.locals.push(Local { name: format!("{name}.val"), ty: self.type_of(pair.val), mutable: false });
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

}
