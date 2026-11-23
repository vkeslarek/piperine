//! Compound values lowered to nodes the IR already has (design "From a source form to IR nodes
//! the contract already has").
//!
//! - An enum is an IR enum of the type arena (`types.rs`): a unit variant is `VariantOf`, the
//!   constant evaluator names a unit variant by its position `k` in declaration order, and a
//!   `match` arm over it tests `IsVariant`.
//! - A `match` is an `if` chain over its scrutinee, read once: bound to a local unless it already
//!   is a local, a param or a literal. In code that has no local (a drive) the scrutinee is read
//!   again at each test: an expression of the IR has no effect, so the value is the same. Each
//!   arm's pattern is a comparison with the scrutinee, an or-pattern the disjunction of its
//!   alternatives (`||`); the last arm of the exhaustive `match` is the final `else`, and so is an
//!   arm that matches everything.
//! - An `Option<T>` is never an IR type: its value is a pair, a `bool` flag `is` and a payload
//!   `val` of `T` (`None` is `(false, zero)`, `Some(e)` is `(true, e)`). A `let` binds the pair to two locals, `o.is` and
//!   `o.val`; any other position writes the two expressions where they are read.
//!   `is_some`, `is_none`, `unwrap_or`, `if let` and `match` read the pair; `unwrap_or` of a
//!   literal `None` or `Some` is its value.
//! - `let (a, b) = (x, y);` is one local per component.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, LevelPart, Local, Method as IrMethod, Origin, Stmt, StmtId, UnaryOp, ValueType};

use super::elaborator::Elaboration;
use super::patterns::Subject;
use super::selectors::ArmCondition;
use super::lower::Scope;
use crate::pom::{Arm, DeclId, DeclKind, Design, ExprId, ExprKind, FileId, LitKind, PatId, PatKind, Resolution};
use crate::types::{Method, Ty};

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

    /// The position of `variant` among the enum's variants: the name the constant evaluator and the
    /// IR's `VariantOf` give it (its discriminant is `Types::enum_tag`).
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
        let subject = if matches!(self.typed(scrutinee), Some(Ty::Option(_))) { Subject::RereadPair(scrutinee) } else { Subject::Reread(scrutinee) };
        let mut tests = Vec::new();
        for before in earlier {
            if let Some(test) = self.pattern_test(scope, &subject, *before) {
                let ty = ValueType::Bool;
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

    /// `match scrutinee { arms }`, whose value has the base type `base`. `half` is `Some` for a
    /// `match` of `Option` values: the chain of that half of each arm's pair (`lower_option_match`).
    pub(super) fn lower_match(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[Arm], base: ValueType, half: Option<LevelPart>, origin: Origin) -> Option<LoweredExpr> {
        let (subject, binding) = self.subject_of(scope, scrutinee, false)?;
        let mut binding = binding;
        let chain = self.arms_chain(scope, scrutinee, &subject, arms, &base, half, &origin, &mut binding)?;
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

    /// The value of an arm body: the body itself, or, for a `match` of `Option` values, the
    /// `half` of its pair.
    pub(super) fn lower_arm_value(&mut self, scope: &mut Scope, body: ExprId, half: Option<LevelPart>) -> Option<LoweredExpr> {
        let Some(half) = half else { return self.lower_expr(scope, body) };
        let pair = self.lower_option(scope, body)?;
        Some(match half {
            LevelPart::Is => pair.is,
            LevelPart::Value => pair.val,
        })
    }

    /// The body of an arm under the `let`s of the names its pattern binds.
    fn arm_body(&mut self, scope: &mut Scope, names: &[(u32, LoweredExpr)], body: ExprId, half: Option<LevelPart>) -> Option<LoweredExpr> {
        let lets = self.binding_lets(names);
        let lowered = self.lower_arm_value(scope, body, half)?;
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
    fn arms_chain(&mut self, scope: &mut Scope, scrutinee: ExprId, subject: &Subject, arms: &[Arm], base: &ValueType, half: Option<LevelPart>, origin: &Origin, prelude: &mut Vec<StmtId>) -> Option<LoweredExpr> {
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
                self.lower_arm_value(scope, arm.body, half)
            } else {
                self.arm_body(scope, &tested.names, arm.body, half)
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
            let ty = base.clone();
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
        match self.typed(expr) {
            Some(Ty::Option(inner)) => self.value_type(&inner),
            // An argument read as `Some` of itself (§3.2.3) keeps its own type.
            Some(other) if self.types.implicit_some(expr) => self.value_type(&other),
            _ => None,
        }
    }

    /// The known zero of `ty`: the payload of `None`.
    pub(super) fn zero(&mut self, ty: &ValueType, origin: Origin) -> Option<LoweredExpr> {
        let kind = match ty {
            ValueType::F64 => Expr::Real(piperine_ir::Real::new(0.0)),
            ValueType::Bool => Expr::Bool(false),
            ValueType::UInt(_) | ValueType::Int(_) => Expr::Int(0),
            // the zero of an aggregate is the aggregate of zeros; that of an enum is its first variant
            ValueType::Tuple(parts) => Expr::Tuple(parts.iter().map(|part| self.zero(part, origin.clone())).collect::<Option<_>>()?),
            ValueType::Array(element, count) => Expr::ArrayRepeat { element: self.zero(element, origin.clone())?, count: *count },
            ValueType::Struct(id) => {
                let parts = self.type_arena.field_types(*id).to_vec();
                Expr::StructOf { ty: *id, fields: parts.iter().map(|part| self.zero(part, origin.clone())).collect::<Option<_>>()? }
            }
            ValueType::Enum(id) => {
                let parts = self.type_arena.variant_field_types(*id, 0).to_vec();
                Expr::VariantOf { ty: *id, variant: 0, fields: parts.iter().map(|part| self.zero(part, origin.clone())).collect::<Option<_>>()? }
            }
            ValueType::Option(inner) => {
                let is = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
                Expr::OptionPack { is, value: self.zero(inner, origin.clone())? }
            }
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
                    // A carrier holding an `Option`: the two halves of its value, each unpacked from
                    // the read where it stands.
                    (None, None) if let Some(carrier) = self.frame_carrier(scope.frame, decl) => {
                        let half = |this: &mut Self, part: LevelPart, ty: ValueType| {
                            let packed = this.carrier_read(carrier, origin.clone());
                            Some(this.emit(Expr::OptionPart { option: packed, part }, ty, origin.clone()))
                        };
                        let is = half(self, LevelPart::Is, ValueType::Bool)?;
                        let val = half(self, LevelPart::Value, payload.clone())?;
                        Some(Pair { is, val, literal: None })
                    }
                    (None, None) => match self.body_let_value(decl).or_else(|| self.const_value(decl)) {
                        Some(value) => self.lower_option(scope, value),
                        None => unsupported(self),
                    },
                },
                _ => unsupported(self),
            },
            ExprKind::Call { args, .. } if matches!(self.typed(expr), Some(Ty::Option(_))) && self.is_some_call(expr) => {
                let [arg] = args.as_slice() else { return unsupported(self) };
                let val = self.lower_expr(scope, arg.value)?;
                let is = self.emit(Expr::Bool(true), ValueType::Bool, origin);
                Some(Pair { is, val, literal: Some(true) })
            }
            // `Option::<T>::default()` is `None` (H.8 rule 4).
            ExprKind::Call { .. } if matches!(self.types.default_of(expr), Some(Ty::Option(_))) => {
                let is = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
                let Some(val) = self.zero(&payload, origin) else { return unsupported(self) };
                Some(Pair { is, val, literal: Some(false) })
            }
            // `cast::<A, B>(x)` (§22.1.3): an `Option<B>`, the two halves of one level cast.
            ExprKind::Call { args, .. } if matches!(self.types.callee(expr), Some(crate::types::Callee::Intrinsic(crate::types::Intrinsic::Cast))) => {
                let [arg] = args.as_slice() else { return unsupported(self) };
                self.lower_cast(scope, expr, arg.value)
            }
            // A call of a function that returns an `Option`: the pair of its two halves (`returns.rs`).
            ExprKind::Call { .. } if let Some(function) = self.calls_option_function(expr) => self.lower_option_call(scope, expr, function),
            // A field of `Option` type: the two halves of the packed value, each unpacked from the
            // field read where it stands.
            ExprKind::Field { expr: recv, name, .. } if matches!(self.typed(*recv), Some(Ty::Struct(_) | Ty::Tuple(_))) => {
                let index = self.field_index(&self.typed(*recv)?, &name.text)?;
                let mut half = |this: &mut Self, part: LevelPart, ty: ValueType| {
                    let base = this.lower_expr(scope, *recv)?;
                    let packed = this.emit(Expr::Field { expr: base, index }, ValueType::Option(Box::new(payload.clone())), origin.clone());
                    Some(this.emit(Expr::OptionPart { option: packed, part }, ty, origin.clone()))
                };
                let is = half(self, LevelPart::Is, ValueType::Bool)?;
                let val = half(self, LevelPart::Value, payload.clone())?;
                Some(Pair { is, val, literal: None })
            }
            // `a.first()`, `a.last()`, `a.get(i)` of an array (H.5).
            ExprKind::MethodCall { recv, args, .. } if matches!(self.types.method(expr), Some(Method::ArrayFirst | Method::ArrayLast | Method::ArrayGet)) => {
                let method = self.types.method(expr)?;
                let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                self.lower_array_option(scope, method, *recv, &args, payload, origin)
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
                let overflowed = self.emit(Expr::Method { method: overflows, receiver: a, args: vec![b] }, ValueType::Bool, origin.clone());
                let is = self.emit(Expr::Unary { op: UnaryOp::Not, expr: overflowed }, ValueType::Bool, origin.clone());
                let (a, b) = (self.lower_expr(scope, *recv)?, self.lower_expr(scope, arg.value)?);
                let val = self.emit(Expr::Binary { op, lhs: a, rhs: b }, payload, origin);
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
                let is_ty = ValueType::Bool;
                let val_ty = payload;
                let is = self.emit(Expr::If { cond: first, then: then.is, otherwise: otherwise.is }, is_ty, origin.clone());
                let val = self.emit(Expr::If { cond: second, then: then.val, otherwise: otherwise.val }, val_ty, origin);
                Some(Pair { is, val, literal: None })
            }
            // `match s { p => a, .. }` of `Option`s: the flag and the payload each a `match` chain
            // over `s`, an arm's body read as its pair.
            ExprKind::Match { scrutinee, arms } => {
                if let Some(selected) = self.folded_selection(scope, *scrutinee, arms) {
                    return self.lower_option(scope, arms[selected].body);
                }
                scope.decisions += 1;
                let is = self.lower_match(scope, *scrutinee, arms, ValueType::Bool, Some(LevelPart::Is), origin.clone());
                let val = self.lower_match(scope, *scrutinee, arms, payload, Some(LevelPart::Value), origin);
                scope.decisions -= 1;
                Some(Pair { is: is?, val: val?, literal: None })
            }
            // `if let p = s { a } else { b }` of `Option`s: the same, a `match` of two arms.
            ExprKind::If { cond, then, else_: Some(else_) } => {
                let (is, val) = (
                    self.lower_if_let(scope, *cond, then, Some(*else_), ValueType::Bool, Some(LevelPart::Is), origin.clone()),
                    self.lower_if_let(scope, *cond, then, Some(*else_), payload, Some(LevelPart::Value), origin),
                );
                Some(Pair { is: is?, val: val?, literal: None })
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
                let ty = ValueType::Bool;
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
                        let ty = base;
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
                        let ty = base;
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
                let is_ty = ValueType::Bool;
                let is = self.emit(Expr::If { cond: a.is, then: yes, otherwise: b.is }, is_ty, origin.clone());
                let val_ty = payload;
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
                let val_ty = payload;
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
                            let ty = payload;
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
                let is_ty = ValueType::Bool;
                let is = self.emit(Expr::If { cond: pair.is, then: is_inner, otherwise: no }, is_ty, origin.clone());
                let val_ty = payload;
                let val = self.emit(Expr::If { cond: pair.is, then: val_inner, otherwise: zero }, val_ty, origin);
                Some(Pair { is, val, literal: None })
            }
        }
    }

    /// `let o = e;` of type `Option<T>`: the flag to the local `o.is`, the payload to `o.val`.
    pub(super) fn lower_option_let(&mut self, scope: &mut Scope, out: &mut Vec<StmtId>, binding: DeclId, init: ExprId) -> bool {
        if self.types.is_assigned(binding) {
            let span = self.design.decl_data(binding).span;
            self.unsupported(scope.file, span, "an assignment to a local of type `Option`");
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

    /// Whether `let (p, q) = init;` binds a name to each component of a tuple expression.
    pub(super) fn is_component_let(&self, pats: &[PatId], init: ExprId) -> bool {
        matches!(&self.design.expr_node(self.peel_parens(init)).kind, ExprKind::Tuple(items) if items.len() == pats.len())
            && pats.iter().all(|p| matches!(self.design.pat_node(*p).kind, PatKind::Ident { sub: None, .. }))
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
