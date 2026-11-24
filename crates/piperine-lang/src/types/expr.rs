//! Value expressions: `check` against an expected type where the position gives one, `infer`
//! otherwise. A mismatch is one `E0180` at the expression and the expression then takes the
//! expected type, so one mistake is one diagnostic.

use crate::msl::lexer;
use crate::msl::{Diagnostic, Span};
use crate::pom::{BinOp, BlockNode, FileId, BuiltinKind, DeclKind, ExprId, ExprKind, GenericParamData, LitKind, PatKind, Resolution, StmtId, StmtKind, StructClass, UnOp};
use crate::stack::Stack;

use super::check::{Checker, Context};
use super::derives::DerivedTrait;
use super::flow::Assigned;
use super::operators::OperatorTrait;
use super::widths::FloatText;
use super::unsupported::Unsupported;
use super::{Nominal, Ty};

impl Checker<'_> {
    /// Check `expr` against `expected`. A mismatch is one diagnostic; the expression then takes
    /// the expected type.
    pub(super) fn check(&mut self, expr: ExprId, expected: &Ty) {
        self.check_as(expr, expected, "E0180");
    }

    /// `check` of an argument: an `Option<T>` parameter takes a `T` as `Some` of it (§3.2.3).
    pub(super) fn check_argument(&mut self, expr: ExprId, expected: &Ty) {
        if let Ty::Option(inner) = expected {
            self.option_argument(expr, expected, inner);
            return;
        }
        if matches!(expected, Ty::Carrier { .. }) {
            self.handle_argument(expr, expected);
            return;
        }
        self.check(expr, expected);
    }

    /// `check`, where a mismatch of `expr` itself carries `code` (a rule of the specification
    /// with a code of its own, `E0318` for a `where`) instead of the plain `E0180`.
    pub(super) fn check_as(&mut self, expr: ExprId, expected: &Ty, code: &'static str) {
        Stack::deep(|| {
            let node = self.design.expr_node(expr).clone();
            let file = self.file_of_expr(expr);
            if *expected == Ty::Event {
                self.event_slot = Some(expr);
            }
            // A local bound to a bare literal takes the integer type its first use requires (§1.4.4).
            if self.fix_open_literal(expr, expected) {
                self.exprs[expr.index()] = Some(expected.clone());
                return;
            }
            // Where the expected type flows into the parts of the expression: the value of a
            // block or of the arms of an `if` is where a mismatch is reported.
            let found = match &node.kind {
                ExprKind::Paren(inner) => {
                    self.check_as(*inner, expected, code);
                    Some(expected.clone())
                }
                ExprKind::Block(block) => self.statements_as(&block.stmts, Some(expected), code),
                ExprKind::Closure { .. } => self.closure(expr, expected),
                ExprKind::If { cond, then, else_: Some(else_) } => {
                    self.condition(*cond);
                    let before = self.flow.assigned.clone();
                    self.statements_as(&then.stmts, Some(expected), code);
                    let after_then = self.flow.end_branch(&before);
                    self.check_as(*else_, expected, code);
                    self.flow.assigned = Assigned::join([after_then, self.flow.assigned.clone()]);
                    Some(expected.clone())
                }
                // `100` where an `f64` is required: an integer literal never converts (§1 rule 7).
                ExprKind::Lit(lit) if matches!(&lit.kind, LitKind::Int(text) if !FloatText::is(text)) && expected.is_float() => {
                    self.mismatch_coded(file, node.span, code, &expected.name(self.design), "an integer literal");
                    Some(expected.clone())
                }
                ExprKind::Match { .. } => self.match_expr(expr, Some(expected)),
                // The expected aggregate type flows into the parts of a tuple and an array.
                ExprKind::Tuple(items) => self.tuple_expression(items, Some(expected)),
                ExprKind::Array(items) => self.array_expression(expr, items, Some(expected)),
                ExprKind::ArrayRepeat { elem, len } => self.array_repeat(expr, *elem, *len, Some(expected)),
                ExprKind::Loop { body } => self.loop_expr(body, Some(expected)),
                // `Name { params }` where a module type is expected: a template value (PI-04).
                ExprKind::Struct { class: StructClass::Template(module), fields, .. } if matches!(expected, Ty::Module(_)) => self.template_value(expr, *module, fields),
                // `None` and `Some(e)` where an `Option<T>` is expected.
                ExprKind::Path(_) if matches!(expected, Ty::Option(_)) && self.is_none_literal(expr) => Some(expected.clone()),
                ExprKind::Call { args, .. } if matches!(self.callee_resolution_of_call(expr), Some(Resolution::Builtin(id)) if id.info().name == "Some") => match expected {
                    Ty::Option(inner) => self.some(expr, args, Some(inner)),
                    _ => self.some(expr, args, None),
                },
                // An integer literal where an integer type is expected takes that type (§1 rule 7).
                ExprKind::Lit(lit) if expected.is_integer() && matches!(&lit.kind, LitKind::Int(text) if !FloatText::is(text)) => {
                    let LitKind::Int(text) = &lit.kind else { return };
                    self.integer_literal(file, node.span, text, Some(expected), false)
                }
                // `1.5` where a float type is expected takes that type (§1.4.2); one past the type's range is `E0909`.
                ExprKind::Lit(lit) if expected.is_float() && matches!(&lit.kind, LitKind::Float { raw, .. } if lexer::float_suffix(raw).is_empty()) => {
                    let LitKind::Float { raw, .. } = &lit.kind else { return };
                    self.check_float_literal(file, node.span, raw, expected);
                    Some(expected.clone())
                }
                // `-1.5` where a float type other than `f64` is expected: the minus takes the expected type.
                ExprKind::Unary { op: UnOp::Neg, expr: operand } if expected.is_other_float() && self.bare_float_literal(*operand) => {
                    self.check_as(*operand, expected, code);
                    Some(expected.clone())
                }
                // `-256` where an `i16` is expected: the minus takes the expected type, and a literal
                // operand may reach the magnitude of the type's minimum.
                ExprKind::Unary { op: UnOp::Neg, expr: operand } if expected.is_integer() => {
                    match self.integer_literal_text(*operand) {
                        Some(text) => {
                            let span = self.design.expr_node(*operand).span;
                            let found = self.integer_literal(file, span, &text, Some(expected), true);
                            if let Some(found) = &found
                                && found != expected
                            {
                                self.mismatch(file, span, expected, found);
                            }
                            self.exprs[operand.index()] = Some(expected.clone());
                        }
                        None => self.check(*operand, expected),
                    }
                    Some(expected.clone())
                }
                // An operator whose result has its operands' type passes the expected integer type
                // down to both, with the rule's code (`count <- count + 1.0` is `E0432`); a shift
                // passes it to its left operand only.
                ExprKind::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor, lhs, rhs } if expected.is_integer() || expected.is_other_float() => {
                    self.check_as(*lhs, expected, code);
                    self.check_as(*rhs, expected, code);
                    Some(expected.clone())
                }
                ExprKind::Binary { op: BinOp::Shl | BinOp::Shr, lhs, rhs } if expected.is_integer() => {
                    self.check_as(*lhs, expected, code);
                    self.shift_amount(*rhs);
                    Some(expected.clone())
                }
                ExprKind::Unary { op: UnOp::Not, expr: operand } if expected.is_integer() => {
                    self.check_as(*operand, expected, code);
                    Some(expected.clone())
                }
                // a call, a struct expression and a constructor name may take the arguments of a generic type from the type expected
                ExprKind::Call { .. } | ExprKind::Struct { .. } | ExprKind::Path(_) => {
                    self.call_expected = Some((expr, expected.clone()));
                    self.infer(expr)
                }
                _ => self.infer(expr),
            };
            if let Some(found) = found
                && !self.compatible(expr, &found, expected)
                && found != Ty::Never
            {
                if self.event_reads.contains(&expr) {
                    self.event_where_another_type_is_expected(expr, expected);
                } else {
                    let (expected_name, found_name) = (expected.name(self.design), found.name(self.design));
                    self.mismatch_coded(file, node.span, code, &expected_name, &found_name);
                }
            }
            self.exprs[expr.index()] = Some(expected.clone());
        })
    }

    /// The function a plain path names, if it names one.
    fn function_named(&self, expr: ExprId) -> Option<crate::pom::DeclId> {
        match self.callee_resolution(expr) {
            Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Fn(_)) => Some(decl),
            _ => None,
        }
    }

    /// A function's name used as a value: reached, typed from its signature.
    fn function_value(&mut self, expr: ExprId) -> Option<Ty> {
        let decl = self.function_named(expr)?;
        self.reach(decl);
        let (params, ret) = self.signature(decl)?;
        Some(Ty::Fn(params, Box::new(ret)))
    }

    /// The type of `expr` from the expression alone; `None` after a diagnostic.
    pub(super) fn infer(&mut self, expr: ExprId) -> Option<Ty> {
        Stack::deep(|| {
            let in_event_position = self.event_slot == Some(expr);
            let mut ty = self.infer_kind(expr);
            if ty == Some(Ty::Event) && !in_event_position {
                ty = Some(self.event_out_of_place(expr));
            }
            self.exprs[expr.index()] = ty.clone();
            ty
        })
    }

    fn infer_kind(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        match &node.kind {
            ExprKind::Lit(lit) => self.literal(&lit.kind, file, node.span),
            ExprKind::Path(_) => self.path(expr),
            ExprKind::Paren(inner) => {
                if self.event_slot == Some(expr) {
                    self.event_slot = Some(*inner);
                }
                self.infer(*inner)
            }
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => {
                if let Some(text) = self.integer_literal_text(*operand) {
                    let span = self.design.expr_node(*operand).span;
                    let ty = self.integer_literal(file, span, &text, None, true);
                    self.exprs[operand.index()] = ty.clone();
                    return ty;
                }
                match self.infer(*operand) {
                    Some(t) if t.is_integer() || t.is_other_float() => Some(t),
                    Some(Ty::F64) | None => Some(Ty::F64),
                    Some(other) => {
                        let span = self.design.expr_node(*operand).span;
                        self.mismatch(file, span, &Ty::F64, &other);
                        Some(Ty::F64)
                    }
                }
            }
            ExprKind::Unary { op: UnOp::Not, expr: operand } => {
                // An `Event` operand is judged here, as `E0440`, not as an `Event` out of place.
                self.event_slot = Some(*operand);
                match self.infer(*operand) {
                    Some(t) if t.is_integer() => Some(t),
                    Some(Ty::Bool) | None => Some(Ty::Bool),
                    Some(Ty::Event) => self.negated_event(expr, *operand),
                    // a struct or an enum takes `!` from the `impl` of `Not` (Appendix H.10)
                    Some(t) if Self::takes_operator_traits(&t) => self.operator_call(expr, OperatorTrait::Not, *operand, &t, None),
                    Some(other) => {
                        let span = self.design.expr_node(*operand).span;
                        self.mismatch(file, span, &Ty::Bool, &other);
                        Some(Ty::Bool)
                    }
                }
            }
            ExprKind::Assign { op, lhs, rhs } => self.assignment(expr, *op, *lhs, *rhs),
            ExprKind::While { cond, body } => self.while_loop(*cond, body),
            ExprKind::Loop { body } => self.loop_expr(body, None),
            ExprKind::For { pat, iter, body } => self.for_loop(*iter, *pat, body),
            ExprKind::Break(value) => self.break_expr(expr, *value),
            ExprKind::Continue => self.continue_expr(expr),
            ExprKind::Return(value) => self.return_expr(expr, *value),
            ExprKind::Try(operand) => self.try_expr(expr, *operand),
            ExprKind::Cast { expr: operand, ty } => self.cast(*operand, *ty),
            ExprKind::Binary { op, lhs, rhs } => self.binary(expr, *op, *lhs, *rhs),
            ExprKind::Block(block) => self.statements(&block.stmts, None),
            ExprKind::If { cond, then, else_ } => self.conditional(*cond, then, *else_),
            ExprKind::Call { .. } => self.call(expr),
            ExprKind::Match { .. } => self.match_expr(expr, None),
            ExprKind::MethodCall { .. } => self.method_call(expr),
            ExprKind::Field { .. } => self.field_read(expr),
            ExprKind::Tuple(items) => self.tuple_expression(items, None),
            ExprKind::Array(items) => self.array_expression(expr, items, None),
            ExprKind::ArrayRepeat { elem, len } => self.array_repeat(expr, *elem, *len, None),
            ExprKind::Index { expr: base, index } => self.index_expression(expr, *base, *index),
            ExprKind::Range { lo, hi, .. } => self.range_expr(file, node.span, *lo, *hi),
            ExprKind::Struct { path, fields, class: StructClass::StructLiteral } => self.struct_expression(expr, *path, fields),
            ExprKind::Closure { .. } => self.closure_value(expr),
            ExprKind::Emit { stream, fields } => self.emit(expr, stream, fields),
            ExprKind::IncludeStr(_) => self.include_str(expr),
            ExprKind::Struct { class: StructClass::Template(_), .. } => {
                self.unsupported(file, node.span, "module template `Name { .. }` outside a placement");
                None
            }
            other => {
                self.unsupported(file, node.span, Unsupported::expr(other));
                None
            }
        }
    }

    /// `include_str!("path")` is the text of a file, a `str`, and a const expression allowed in a
    /// `const` item only (§20.2 rule 1); a param default, a `where` or any other position is
    /// `E0610`.
    fn include_str(&mut self, expr: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let in_const_item = self.context == Context::Const && self.owner.is_some_and(|owner| matches!(self.design.decl_data(owner).kind, DeclKind::Const(_)));
        if !in_const_item {
            self.coded(file, "E0610", span, "`include_str!` outside a `const` item".to_string(), "`include_str!` is allowed in the initializer of a `const` item only");
            return None;
        }
        Some(Ty::Str)
    }

    pub(super) fn binary(&mut self, whole: ExprId, op: BinOp, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(lhs);
        let span = self.design.expr_node(whole).span;
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let comparison = matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
                let ty = match self.operands(lhs, rhs, comparison, |t| t.is_integer() || t.is_other_float()) {
                    Some(t) => t,
                    None => Ty::F64,
                };
                Some(if comparison { Ty::Bool } else { ty })
            }
            BinOp::And | BinOp::Or => self.logical(whole, op == BinOp::And, lhs, rhs),
            BinOp::Eq | BinOp::Ne => {
                let (first, second) = self.literal_last(lhs, rhs);
                let first_ty = self.infer(first);
                // a struct or an enum with an `impl` of `PartialEq` takes `==` from it, whatever its `Out` (Appendix H.10)
                if let (Some(ty), Some(operator), true) = (&first_ty, OperatorTrait::of_binary(op), first == lhs)
                    && Self::takes_operator_traits(ty)
                    && self.has_operator_impl(operator, ty)
                {
                    return self.operator_call(whole, operator, lhs, ty, Some(rhs));
                }
                match first_ty {
                    Some(t @ (Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::GenericInteger { .. } | Ty::Str)) => self.check(second, &t),
                    // §1.3.10, H.8 rule 7: an enum, a struct, a tuple, an array or an `Option` compares
                    // when it implements `PartialEq`.
                    Some(t) if t.is_composite() || matches!(t, Ty::Param(_)) => {
                        self.check(second, &t);
                        let span = self.design.expr_node(first).span;
                        self.require_trait(file, span, &t, DerivedTrait::PartialEq);
                    }
                    Some(_) => {
                        let span = self.design.expr_node(first).span;
                        self.unsupported(file, span, "comparison of values that are neither `f64`, `bool` nor an integer");
                    }
                    None => {
                        self.infer(second);
                    }
                }
                Some(Ty::Bool)
            }
            BinOp::Rem | BinOp::BitXor | BinOp::BitAnd | BinOp::BitOr => {
                let bitwise = op != BinOp::Rem;
                let (first, _) = self.literal_last(lhs, rhs);
                let first_ty = self.infer(first);
                // a struct or an enum takes the operator from the `impl` of its trait; a built-in type does when the
                // project writes an `impl` of the trait for it and the right operand is no bare literal (Appendix H.10)
                if let (Some(ty), Some(operator), true) = (&first_ty, OperatorTrait::of_binary(op), first == lhs)
                    && (Self::takes_operator_traits(ty) || (self.has_operator_impl(operator, ty) && self.integer_literal_text(rhs).is_none() && self.infer(rhs).is_some_and(|found| Self::takes_operator_traits(&found))))
                {
                    return self.operator_call(whole, operator, lhs, ty, Some(rhs));
                }
                match self.operands_of(first_ty, lhs, rhs, false, |t| t.is_integer() || (bitwise && *t == Ty::Bool)) {
                    Some(t) => Some(t),
                    None => {
                        self.unsupported(file, span, Unsupported::binary(op));
                        None
                    }
                }
            }
            BinOp::Shl | BinOp::Shr => match self.infer(lhs) {
                Some(t) if t.is_integer() => {
                    self.shift_amount(rhs);
                    Some(t)
                }
                Some(t) if Self::takes_operator_traits(&t) => OperatorTrait::of_binary(op).and_then(|operator| self.operator_call(whole, operator, lhs, &t, Some(rhs))),
                _ => {
                    self.unsupported(file, span, Unsupported::binary(op));
                    None
                }
            },
        }
    }

    /// The two operands of an operator that takes one type: the operand that is not a bare
    /// integer literal is typed first and the other is checked against it (local literal
    /// typing). `Some` of that type when `accepts` it; otherwise both operands are held to `f64`,
    /// as the M1 operators always were, and `None` comes back. An `ordering` operator (`<`, `<=`,
    /// `>`, `>=`) also takes an enum that derives `PartialOrd` (§2 rule 5), compared in
    /// declaration order; the result is then that enum.
    fn operands(&mut self, lhs: ExprId, rhs: ExprId, ordering: bool, accepts: impl Fn(&Ty) -> bool) -> Option<Ty> {
        let (first, _) = self.literal_last(lhs, rhs);
        let first_ty = self.infer(first);
        self.operands_of(first_ty, lhs, rhs, ordering, accepts)
    }

    /// [`Checker::operands`] with the type of the first operand already inferred.
    fn operands_of(&mut self, first_ty: Option<Ty>, lhs: ExprId, rhs: ExprId, ordering: bool, accepts: impl Fn(&Ty) -> bool) -> Option<Ty> {
        let (first, second) = self.literal_last(lhs, rhs);
        match first_ty {
            Some(t) if accepts(&t) => {
                self.check(second, &t);
                Some(t)
            }
            Some(t) if ordering && (t.is_composite() || matches!(t, Ty::Param(_))) => {
                self.check(second, &t);
                let file = self.file_of_expr(first);
                let span = self.design.expr_node(first).span;
                self.require_trait(file, span, &t, DerivedTrait::PartialOrd);
                Some(t)
            }
            found => {
                if let Some(found) = found
                    && found != Ty::F64
                {
                    let file = self.file_of_expr(first);
                    let span = self.design.expr_node(first).span;
                    self.mismatch(file, span, &Ty::F64, &found);
                }
                self.check(second, &Ty::F64);
                None
            }
        }
    }

    /// `(lhs, rhs)`, or `(rhs, lhs)` when only `lhs` is a bare integer literal: the order in which
    /// two operands of one type are typed.
    pub(super) fn literal_last(&self, lhs: ExprId, rhs: ExprId) -> (ExprId, ExprId) {
        let bare = |this: &Self, e: ExprId| this.integer_literal_text(e).is_some_and(|t| lexer::int_suffix_text(&t).is_empty()) || this.open_literal_local(e).is_some() || this.bare_float_literal(e);
        if bare(self, lhs) && !bare(self, rhs) { (rhs, lhs) } else { (lhs, rhs) }
    }

    /// The right operand of `<<` or `>>`: any integer type (DIG-01 AC 6); a bare literal there
    /// expects nothing and is `i32`.
    fn shift_amount(&mut self, rhs: ExprId) {
        match self.infer(rhs) {
            Some(t) if t.is_integer() => {}
            Some(other) => {
                let file = self.file_of_expr(rhs);
                let span = self.design.expr_node(rhs).span;
                self.mismatch_text(file, span, "an integer type", &other.name(self.design));
            }
            None => {}
        }
    }

    /// `expr as T`: between any two of the integer types and `f64`, and from `bool` to an integer
    /// type (§1 rule 1); any other pair is `E0182` naming both types (§1.4.5).
    fn cast(&mut self, operand: ExprId, ty: crate::pom::TypeId) -> Option<Ty> {
        let target = self.lower_type(ty);
        let source = self.infer(operand);
        let target = target?;
        let numeric = |t: &Ty| t.is_numeric();
        // §1.3.7: an enum without fields converts to an integer type; its value is the discriminant.
        if let Some(Ty::Enum(Nominal { decl, .. })) = &source
            && target.is_integer()
            && !self.has_payload(*decl)
        {
            return Some(target);
        }
        if let Some(source) = source
            && !(numeric(&target) && (numeric(&source) || (source == Ty::Bool && target.is_integer())))
        {
            let file = self.file_of_expr(operand);
            let span = self.design.expr_node(operand).span;
            let (found, wanted) = (source.name(self.design), target.name(self.design));
            self.coded(file, "E0182", span, format!("no `as` conversion from `{found}` to `{wanted}`"), &format!("`{found}` does not convert `as` `{wanted}`"));
        }
        Some(target)
    }

    /// `if cond { then } else { else_ }`: with an `else` the common type of the two blocks;
    /// without one, `()`.
    fn conditional(&mut self, cond: ExprId, then: &BlockNode, else_: Option<ExprId>) -> Option<Ty> {
        self.condition(cond);
        let before = self.flow.assigned.clone();
        let (found, after_then, after_else) = match else_ {
            // `if c { 0 } else { count + 1 }`: the first arm that is not a bare integer literal gives
            // the type the literal takes (local literal typing).
            Some(else_) if self.block_waits(then) => {
                let else_ty = self.infer(else_);
                let after_else = self.flow.end_branch(&before);
                self.statements(&then.stmts, else_ty.as_ref());
                (else_ty, self.flow.end_branch(&before), after_else)
            }
            _ => {
                let then_ty = self.statements(&then.stmts, None);
                let after_then = self.flow.end_branch(&before);
                let (found, after_else) = match (else_, then_ty) {
                    // a `then` that never completes takes the type of the `else`
                    (Some(else_), Some(Ty::Never)) => (self.infer(else_), self.flow.end_branch(&before)),
                    (Some(else_), Some(t)) => {
                        self.check(else_, &t);
                        (Some(t), self.flow.end_branch(&before))
                    }
                    (Some(else_), None) => {
                        self.infer(else_);
                        (None, self.flow.end_branch(&before))
                    }
                    (None, Some(Ty::Never)) => (Some(Ty::Unit), before.clone()),
                    (None, Some(t)) if t != Ty::Unit => {
                        let file = self.file_of_expr(cond);
                        self.mismatch(file, then.span, &Ty::Unit, &t);
                        (Some(Ty::Unit), before.clone())
                    }
                    (None, t) => (t, before.clone()),
                };
                (found, after_then, after_else)
            }
        };
        self.flow.assigned = Assigned::join([after_then, after_else]);
        found
    }

    fn literal(&mut self, kind: &LitKind, file: crate::pom::FileId, span: crate::msl::Span) -> Option<Ty> {
        match kind {
            // §1.4.2: a suffix gives the type; an SI-suffixed literal is always an `f64` (A.2 rule 1)
            LitKind::Float { raw, .. } if lexer::float_suffix(raw) == "f32" => {
                self.check_float_literal(file, span, raw, &Ty::F32);
                Some(Ty::F32)
            }
            LitKind::Float { .. } => Some(Ty::F64),
            LitKind::Bool(_) => Some(Ty::Bool),
            LitKind::Int(text) if FloatText::is(text) => {
                let ty = FloatText::suffix(text);
                if let Some(ty) = &ty {
                    self.check_float_literal(file, span, text, ty);
                }
                ty
            }
            LitKind::Int(text) => self.integer_literal(file, span, text, None, false),
            LitKind::Str { .. } => Some(Ty::Str),
        }
    }

    /// The type of the integer literal `text`: its suffix, else `expected`, else `i32` (§1 rule
    /// 7). A suffix of another type than `expected` is a mismatch; a value outside the type's
    /// range is `E0909`; a suffix outside the subset (`u128`, `f32`) is `P0001` naming it. With
    /// `negated` the literal is the operand of a unary `-`.
    pub(super) fn integer_literal(&mut self, file: FileId, span: Span, text: &str, expected: Option<&Ty>, negated: bool) -> Option<Ty> {
        let suffix = lexer::int_suffix_text(text);
        let ty = match suffix {
            "" => expected.cloned().unwrap_or(Ty::Int(32)),
            _ => {
                let width = suffix[1..].parse::<u8>().unwrap_or(u8::MAX);
                match (&suffix[..1], width) {
                    ("u", 1..=128) => Ty::UInt(u32::from(width)),
                    ("i", 1..=128) => Ty::Int(u32::from(width)),
                    ("u" | "i", _) => {
                        self.unsupported(file, span, &format!("integer type `{suffix}`"));
                        return None;
                    }
                    _ => {
                        self.unsupported(file, span, &format!("`{suffix}` literal"));
                        return None;
                    }
                }
            }
        };
        if let Some(expected) = expected
            && *expected != ty
        {
            return Some(ty);
        }
        let value = lexer::int_value(text);
        if let (Some(value), Some(max)) = (value, ty.literal_max(negated))
            && value > max
        {
            let name = ty.name(self.design);
            let message = format!("`{text}` is out of range: an integer literal beyond the range of `{name}`");
            self.report(file, Diagnostic::new("E0909", message, span, "literal out of range").with_help("write a value the type holds, or a wider type"));
        }
        Some(ty)
    }

    /// The text of `expr` when it is an integer literal without an `f64` suffix, through parens.
    pub(super) fn integer_literal_text(&self, expr: ExprId) -> Option<String> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.integer_literal_text(*inner),
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Int(text) if !FloatText::is(text) => Some(text.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether the value of `block` takes its type from another arm: an integer literal with no
    /// suffix, or `None`.
    fn block_waits(&self, block: &BlockNode) -> bool {
        let Some(last) = block.stmts.last() else { return false };
        match &self.design.stmt_node(*last).kind {
            StmtKind::Expr { expr, semi: false } => self.integer_literal_waits(*expr) || self.is_none_literal(*expr),
            _ => false,
        }
    }

    /// Whether `expr` is an integer literal with no suffix: its type comes from its position.
    pub(super) fn integer_literal_waits(&self, expr: ExprId) -> bool {
        self.integer_literal_text(expr).is_some_and(|t| lexer::int_suffix_text(&t).is_empty())
    }

    /// The resolution of the callee of the call `expr`.
    fn callee_resolution_of_call(&self, expr: ExprId) -> Option<Resolution> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Call { callee, .. } => self.callee_resolution(*callee),
            _ => None,
        }
    }

    /// A name: a param, a constant, a parameter or local binding, or a function as a value.
    fn path(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let file = self.file_of_expr(expr);
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        match res {
            Resolution::Decl(decl) => {
                let kind = self.design.decl_data(decl).kind.clone();
                match kind {
                    DeclKind::Const(c) => {
                        self.reach(decl);
                        self.lower_type(c.ty)
                    }
                    DeclKind::Param(p) => self.lower_type(p.ty),
                    // §3.8.3: a const parameter is a const expression of its declared type wherever it is in scope
                    DeclKind::GenericParam(GenericParamData::Const { ty, .. }) => self.lower_type(ty),
                    DeclKind::Port(p) => self.read_type(decl, p.ty),
                    DeclKind::Signal(s) => self.read_type(decl, s.ty),
                    DeclKind::FnParam(_) | DeclKind::Binding(_) => {
                        // a handle read: the value of a discrete carrier, the node itself (§3.1.5, §3.1.6)
                        if let Some(read) = self.handle_read(decl) {
                            return Some(read);
                        }
                        let local = self.locals.get(&decl).cloned();
                        // A read that requires no type gives the literal its default `i32` (§1.4.2).
                        self.open_literals.remove(&decl);
                        if self.read_before_assignment(file, node.span, decl) {
                            return local;
                        }
                        if self.pending_closures.contains_key(&decl) {
                            let message = "a closure whose parameters have no written type is typed by its first call: call it before using it as a value (§1.4.4)".to_string();
                            self.coded(file, "E0181", node.span, message, "parameter type unknown");
                            return None;
                        }
                        if local.is_none() && self.lets_open.contains_key(&decl) {
                            return self.self_read(file, node.span, decl);
                        }
                        if local.is_none() && self.continuous_result_in_const(file, node.span, decl) {
                            return None;
                        }
                        if let Some(Ty::Module(module)) = &local {
                            return self.instance_as_value(file, node.span, decl, *module);
                        }
                        if local.is_none() && !self.poisoned.contains(&decl) {
                            let what = if self.fn_where { "a value parameter in the `where` of a `fn`" } else { "a name whose type is unknown" };
                            self.unsupported(file, node.span, what);
                        }
                        local
                    }
                    DeclKind::Struct(_) | DeclKind::Variant(_) => self.constructor_value(expr, decl, file, node.span),
                    // §1.2.10: a function's name is a value of type `fn(T..) -> U`
                    DeclKind::Fn(_) => self.function_value(expr),
                    other => {
                        self.unsupported(file, node.span, &format!("{} used as a value", Unsupported::item(&other)));
                        None
                    }
                }
            }
            Resolution::Builtin(id) if id.info().kind == BuiltinKind::Node => Some(Ty::AnyNode),
            Resolution::Builtin(id) if id.info().kind == BuiltinKind::Event => Some(self.settle_event()),
            Resolution::Builtin(id) if id.info().name == "None" => {
                let message = "`None` has no type here: write it where an `Option<T>` is expected";
                self.report(file, Diagnostic::new("E0180", format!("mismatched types: {message}"), node.span, "`Option` of what?"));
                None
            }
            Resolution::Builtin(id) => match self.width_read(expr) {
                Some(width) => Some(width),
                None => {
                    self.unsupported(file, node.span, &format!("built-in `{}`", id.name()));
                    None
                }
            },
            Resolution::Deferred(_) => match self.width_read(expr) {
                Some(width) => Some(width),
                None => {
                    self.unsupported(file, node.span, "an unresolved name");
                    None
                }
            },
            Resolution::Unresolved => {
                self.unsupported(file, node.span, "an unresolved name");
                None
            }
        }
    }

    /// The statements of a block, a function body or an analog block; the type of the block is
    /// that of its last expression without a `;`, else `()`. With an `expected` type the last
    /// expression is checked against it.
    pub(super) fn statements(&mut self, stmts: &[StmtId], expected: Option<&Ty>) -> Option<Ty> {
        self.statements_as(stmts, expected, "E0180")
    }

    /// `statements`, where a mismatch of the block's value carries `code` (see `check_as`).
    fn statements_as(&mut self, stmts: &[StmtId], expected: Option<&Ty>, code: &'static str) -> Option<Ty> {
        let mut value = Some(Ty::Unit);
        // Whether a statement never completes: a block with no final expression is then `!`.
        let mut diverged = false;
        for (i, stmt) in stmts.iter().enumerate() {
            let last = i + 1 == stmts.len();
            value = Some(Ty::Unit);
            let node = self.design.stmt_node(*stmt).clone();
            let file = self.design.decl_data(node.owner).file;
            match &node.kind {
                StmtKind::Let { pat, ty, init, else_ } => self.let_statement(file, *pat, *ty, *init, else_.as_ref()),
                StmtKind::Expr { expr, semi } => {
                    if last && !semi {
                        match expected {
                            Some(expected) => {
                                self.check_as(*expr, expected, code);
                                value = Some(expected.clone());
                            }
                            None => value = self.infer(*expr),
                        }
                    } else {
                        diverged |= self.infer(*expr) == Some(Ty::Never);
                    }
                }
                StmtKind::Empty => {}
                StmtKind::Contribution { lhs, rhs } => self.contribution(file, node.span, *lhs, *rhs),
                StmtKind::Commit { place, value, after } => self.commit(file, node.span, *place, *value, *after),
                StmtKind::Item(_) | StmtKind::Signal(_) | StmtKind::Reaction(_) => {
                    self.unsupported(file, node.span, Unsupported::stmt(&node.kind));
                }
            }
        }
        if diverged && value == Some(Ty::Unit) { Some(Ty::Never) } else { value }
    }

    /// `let x = init;`, `let x: T = init;`, and `let x;` or `let x: T;` (§1.7.5):
    /// the binding takes the annotation, else the type of the initializer, else the type of its
    /// first assignment.
    fn let_statement(&mut self, file: crate::pom::FileId, pat: crate::pom::PatId, ty: Option<crate::pom::TypeId>, init: Option<ExprId>, else_: Option<&BlockNode>) {
        let pat_node = self.design.pat_node(pat).clone();
        let PatKind::Ident { sub: None, binding, res: Resolution::Unresolved, .. } = &pat_node.kind else {
            self.let_pattern(file, pat, ty, init, else_);
            return;
        };
        if else_.is_some() {
            self.let_pattern(file, pat, ty, init, else_);
            return;
        }
        let annotated = match ty {
            Some(t) => match self.lower_type(t).filter(|ty| {
                let carriers = ty.is_carrier_element() && matches!(ty, Ty::Array(..));
                if carriers {
                    self.unsupported(file, self.design.type_node(t).span, "an array of carriers as a `let` type");
                }
                !carriers
            }) {
                Some(t) => Some(t),
                None => {
                    if let Some(init) = init {
                        self.infer(init);
                    }
                    self.poisoned.insert(*binding);
                    return;
                }
            },
            None => None,
        };
        if annotated.is_none()
            && let Some(init) = init
            && let Some(closure) = self.closure_expr(init)
            && self.has_open_parameter(closure)
            && closure == init
        {
            self.defer_closure(*binding, closure);
            return;
        }
        let bound = match (annotated, init) {
            (Some(t), Some(init)) => {
                self.check(init, &t);
                if matches!(t, Ty::Fn(..)) {
                    self.refuse_capturing_pointer(init);
                }
                Some(t)
            }
            (None, Some(init)) => self.infer(init),
            (annotated, None) => {
                self.deferred.insert(*binding);
                if let Some(t) = annotated {
                    self.locals.insert(*binding, t);
                }
                return;
            }
        };
        if let Some(closure) = init.and_then(|i| self.closure_expr(i)) {
            self.closure_locals.insert(*binding, closure);
        }
        match bound {
            Some(t) => {
                if let (None, Some(init)) = (ty, init) {
                    self.wait_for_use(*binding, init);
                }
                self.locals.insert(*binding, t);
            }
            None => {
                self.poisoned.insert(*binding);
            }
        }
    }
}
