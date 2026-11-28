//! Value code: POM expressions lowered to the IR node of the same form.
//!
//! One `Scope` per owner (a module body, later a function) names the params, nodes and locals
//! an expression may read. A `const` read becomes its folded value; a param read stays a
//! `Param` read, never a number (a host `set` must reach the solver without re-elaboration).
//! An integer expression that reads only literals and constants is folded to one literal, by
//! the rules the compiled code follows (`ConstEval`).

use crate::elab::analog::AnalogLowering;
use super::ValueLowering;
use piperine_ir::{BinaryOp, Constant, Expr, ExprId as LoweredExpr, Local, MathFn, Method as IrMethod, Origin, Stmt, StmtId as LoweredStmt, UnaryOp, ValueType};

use crate::elab::digital::elements::ElementPart;
use super::compound::EnumLayout;
use crate::elab::functions::variants::Half;
use crate::elab::consts::Bindings;
use crate::elab::elaborator::{Scope};
use crate::elab::analog::selectors::ArmCondition;
use crate::refusal::forms::ValueForm;
use crate::refusal::RefusedForm;
use crate::msl::Span;
use crate::pom::{BinOp, CallClass, DeclId, DeclKind, ExprId, ExprKind, FileId, LitKind, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::{Callee, Method, Ty};

/// The IR type of a type of the subset.
pub(in crate::elab) struct ValueTypes<'a> {
    pub(in crate::elab) arena: &'a super::types::TypeArena,
}

impl ValueTypes<'_> {
    /// The IR type of an `Option` held as one value (`ValueType::Option`) where it is an element of an array or an item of a
    /// tuple, as it is a field of a struct; `None` for any other type. An `Option` of value code is the pair of its halves.
    fn packed_option(&self, ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::Option(inner) => self.of(inner).or_else(|| self.packed_option(inner)).map(|payload| ValueType::Option(Box::new(payload))),
            _ => None,
        }
    }

    /// `None` for a type no value has in the IR: a node, a module, an `Option` (a pair of values,
    /// msl-digital T27).
    pub(in crate::elab) fn of(&self, ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::F64 => Some(ValueType::F64),
            Ty::F32 => Some(ValueType::F32),
            Ty::F16 => Some(ValueType::F16),
            Ty::F128 => Some(ValueType::F128),
            Ty::Bool => Some(ValueType::Bool),
            Ty::Unit => Some(ValueType::Unit),
            Ty::UInt(n) => Some(ValueType::UInt(*n)),
            Ty::Int(n) => Some(ValueType::Int(*n)),
            Ty::Fn(params, ret) => {
                let params: Option<Vec<ValueType>> = params.iter().map(|p| self.of(p)).collect();
                Some(ValueType::Fn(params?, Box::new(self.of(ret)?)))
            }
            // A struct, an enum, a tuple and an array are the arena's entries and their composition.
            Ty::Struct(_) | Ty::Enum(_) | Ty::Tuple(_) | Ty::Array(..) => self.arena.of(ty, |part| self.of(part).or_else(|| self.packed_option(part))),
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::Option(_) | Ty::Carrier { .. } | Ty::Event | Ty::Str | Ty::Never | Ty::Range { .. } | Ty::Iterator { .. } => None,
            // the elaborator reads types through the substitution (T32): a parameter never reaches the IR
            Ty::Param(_) | Ty::GenericInteger { .. } => None,
        }
    }
}

impl ValueLowering<'_, '_, '_> {
    /// The IR method a numeric row of Appendix H lowers to; `None` for the rows that lower
    /// elsewhere (`exp`, `ln`, `sqrt`, `abs` of `f64` are `Math`; `checked_*` and the `Option`
    /// and `bool` rows are pairs).
    pub(super) fn ir_method(method: Method) -> Option<IrMethod> {
        Some(match method {
            Method::Min => IrMethod::Min,
            Method::Max => IrMethod::Max,
            Method::Clamp => IrMethod::Clamp,
            Method::Floor => IrMethod::Floor,
            Method::Sin => IrMethod::Sin,
            Method::Cos => IrMethod::Cos,
            Method::Tan => IrMethod::Tan,
            Method::Asin => IrMethod::Asin,
            Method::Acos => IrMethod::Acos,
            Method::Atan => IrMethod::Atan,
            Method::Atan2 => IrMethod::Atan2,
            Method::Sinh => IrMethod::Sinh,
            Method::Cosh => IrMethod::Cosh,
            Method::Tanh => IrMethod::Tanh,
            Method::Powf => IrMethod::Powf,
            Method::Powi => IrMethod::Powi,
            Method::Log10 => IrMethod::Log10,
            Method::Log2 => IrMethod::Log2,
            Method::Ceil => IrMethod::Ceil,
            Method::Round => IrMethod::Round,
            Method::Trunc => IrMethod::Trunc,
            Method::Fract => IrMethod::Fract,
            Method::Signum => IrMethod::Signum,
            Method::RemEuclid => IrMethod::RemEuclid,
            Method::Hypot => IrMethod::Hypot,
            Method::IsNan => IrMethod::IsNan,
            Method::IsFinite => IrMethod::IsFinite,
            Method::IsInfinite => IrMethod::IsInfinite,
            Method::IntAbs => IrMethod::IntAbs,
            Method::Pow => IrMethod::Pow,
            Method::SaturatingAdd => IrMethod::SaturatingAdd,
            Method::SaturatingSub => IrMethod::SaturatingSub,
            Method::SaturatingMul => IrMethod::SaturatingMul,
            _ => return None,
        })
    }

    /// `expr` lowered, or `None` after the diagnostics of what could not be. An expression that fails while nothing
    /// has been reported is refused by name here, so no position (a statement, a definition, an event expression, a
    /// value input) can drop it silently.
    pub(in crate::elab) fn lower_expr(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let lowered = self.lower_expr_unchecked(scope, expr);
        if lowered.is_none() && self.state.diagnostics.is_empty() {
            self.elaboration.refuse_silent_failure(scope.values.file, self.state.design.expr_node(expr).span, ValueForm::ThisExpression);
        }
        lowered
    }

    fn lower_expr_unchecked(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.state.design.decl_data(node.owner).file;
        let origin = self.expr_origin(expr);
        // `!` is no value: a diverging expression is `()` until its position gives it a type (`exits.rs`).
        let ty = match self.typed(expr) {
            Some(Ty::Never) => Some(ValueType::Unit),
            other => other.and_then(|t| self.value_type(&t)),
        };
        let Some(ty) = ty else {
            self.refuse(file, node.span, ValueForm::ExpressionTypeCheckerGaveNoValueType);
            return None;
        };
        // An event read as a `bool` is `fired(k)` (§13 rules 2 and 9, `event_reads.rs`).
        if self.state.types.event_read(expr) {
            return self.digital().lower_event_read(scope, expr);
        }
        if let Some(folded) = self.lower_folded_expression(scope, expr, &ty, &origin) {
            return folded;
        }
        match &node.kind {
            ExprKind::Lit(_) => self.lower_literal(expr, ty, origin),
            ExprKind::Paren(inner) => self.lower_expr(scope, *inner),
            ExprKind::Path(_) => self.lower_path(scope, expr, ty, origin),
            ExprKind::Field { expr: recv, name, .. } => self.lower_field_expression(scope, expr, *recv, &name.text, ty, origin),
            ExprKind::Struct { path, fields, .. } => self.lower_struct_expr(scope, expr, *path, fields, ty, origin),
            ExprKind::Tuple(items) => self.lower_tuple(scope, items, ty, origin),
            ExprKind::Array(items) => self.lower_array(scope, items, ty, origin),
            ExprKind::ArrayRepeat { elem, len } => self.lower_array_repeat(scope, *elem, *len, origin),
            ExprKind::Index { expr: base, index } => self.lower_index(scope, expr, *base, *index, origin),
            // an operator the checker resolved to the `impl` function of an operator trait is a call of it (Appendix H.10)
            ExprKind::Unary { expr: operand, .. } if let Some(Callee::Method(function)) = self.state.types.callee(expr) => self.lower_method_call(scope, expr, function, *operand, &[], origin),
            ExprKind::Binary { lhs, rhs, .. } if let Some(Callee::Method(function)) = self.state.types.callee(expr) => self.lower_method_call(scope, expr, function, *lhs, &[*rhs], origin),
            ExprKind::Unary { op, expr: operand } => self.lower_unary(scope, *op, *operand, ty, origin),
            ExprKind::Binary { op: op @ (BinOp::Eq | BinOp::Ne), lhs, rhs } if matches!(self.typed(*lhs), Some(Ty::Str)) => self.digital().lower_text_comparison(scope, *op, *lhs, *rhs, origin),
            ExprKind::Binary { op: op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge), lhs, rhs } if matches!(self.typed(*lhs), Some(Ty::Option(_))) => self.lower_option_comparison(scope, *op, *lhs, *rhs, origin),
            ExprKind::Binary { op, lhs, rhs } => self.lower_binary(scope, *op, *lhs, *rhs, ty, origin),
            ExprKind::If { cond, then, else_ } if matches!(self.state.design.expr_node(*cond).kind, ExprKind::Let { .. }) => self.lower_if_let(scope, *cond, then, *else_, ty, None, origin),
            // A condition that reads only key params and constants is folded: only the arm it
            // selects is lowered (`structural.rs`, §11 rule 3).
            ExprKind::If { cond, then, else_ } if let Some(holds) = self.analog().folded_decision(scope, *cond) => self.lower_folded_if(scope, expr, holds, &then.stmts, *else_, origin),
            ExprKind::If { cond, then, else_ } => self.lower_if(scope, expr, *cond, &then.stmts, *else_, ty, origin),
            ExprKind::Block(block) => self.lower_block_expression(scope, file, &block.stmts, node.span),
            ExprKind::Assign { .. } => self.lower_assignment_expr(scope, expr, origin),
            ExprKind::While { cond, body } => self.lower_while(scope, *cond, body, origin),
            ExprKind::Loop { body } => self.lower_loop(scope, body, ty, matches!(self.typed(expr), Some(Ty::Never)), origin),
            ExprKind::For { pat, iter, body } => self.lower_for(scope, *pat, *iter, body, origin),
            ExprKind::Break(value) => self.lower_break(scope, *value, node.span, origin),
            ExprKind::Continue => Some(self.lower_continue(origin)),
            ExprKind::Return(value) => self.lower_return(scope, *value, node.span, origin),
            ExprKind::Try(inner) => self.lower_try(scope, *inner, node.span, origin),
            ExprKind::Call { args, class, .. } => self.lower_call(scope, expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), *class, ty, origin),
            ExprKind::MethodCall { recv, args, method: method_name, .. } => self.lower_method_expression(scope, expr, *recv, args, &method_name.text, ty, origin),
            ExprKind::Cast { expr: operand, .. } => self.lower_cast(scope, *operand, ty, origin),
            // A scrutinee over the key params folds: only the arm it selects is lowered (`structural.rs`).
            ExprKind::Match { scrutinee, arms } if let Some(selected) = self.analog().folded_selection(scope, *scrutinee, arms) => self.lower_expr(scope, arms[selected].body),
            ExprKind::Match { scrutinee, arms } => self.lower_decided_match(scope, *scrutinee, arms, ty, origin),
            ExprKind::Closure { params, body, .. } => self.lower_closure(scope, expr, params, *body, origin),
            other => {
                self.refuse(file, node.span, RefusedForm::expression(other));
                None
            }
        }
    }

    /// A constant integer expression, or a method of a module type run on the template its param holds (§3.1.4, §8.4.5),
    /// folded at elaboration: the fold's result (`None` after a diagnostic), or `None` when `expr` is neither.
    fn lower_folded_expression(&mut self, scope: &mut Scope, expr: ExprId, ty: &ValueType, origin: &Origin) -> Option<Option<LoweredExpr>> {
        if ty.is_integer() && self.constant_form(expr) {
            return Some(match self.state.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(self.constant_expr(value, ty.clone(), origin.clone())),
                Err(fault) => {
                    self.const_fault(&fault, "a constant expression");
                    None
                }
            });
        }
        // a method of a module type (`fet.gain()`, `N: Mosfet`) runs at elaboration on the template the param holds (§3.1.4, §8.4.5)
        if self.is_template_method(expr) {
            let bindings = self.frame_bindings(scope.values.frame).overlaid(&scope.values.unrolled);
            return Some(match self.state.consts.value(expr, &bindings) {
                Ok(value) => Some(self.constant_expr(value, ty.clone(), origin.clone())),
                Err(fault) => {
                    self.const_fault(&fault, "a method of a template");
                    None
                }
            });
        }
        None
    }

    /// A unary `-` or `!`.
    fn lower_unary(&mut self, scope: &mut Scope, op: UnOp, operand: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let op = match op {
            UnOp::Neg => UnaryOp::Neg,
            UnOp::Not => UnaryOp::Not,
        };
        let operand = self.lower_expr(scope, operand)?;
        Some(self.emit(Expr::Unary { op, expr: operand }, ty, origin))
    }

    /// A binary operator of two values; the right operand of `&&` and `||` is conditional exactly like an `if` arm.
    fn lower_binary(&mut self, scope: &mut Scope, op: BinOp, lhs: ExprId, rhs: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        // The right operand of `&&`/`||` runs only when the left does not decide: it is
        // conditional exactly like an `if` arm.
        let short_circuit = matches!(op, BinOp::And | BinOp::Or);
        let left = lhs;
        let lhs = self.lower_expr(scope, lhs);
        // `a && b` runs `b` when `a` holds, `a || b` when it does not.
        if short_circuit {
            scope.analog.guards.push(ArmCondition::If { condition: left, holds: matches!(op, BinOp::And) });
        }
        let rhs = self.lower_expr(scope, rhs);
        if short_circuit {
            scope.analog.guards.pop();
        }
        let (lhs, rhs) = (lhs?, rhs?);
        let op = Self::binary_op(op);
        Some(self.emit(Expr::Binary { op, lhs, rhs }, ty, origin))
    }

    /// An `if` whose condition reads only key params and constants: only the arm it selects is lowered (`structural.rs`,
    /// §11 rule 3).
    fn lower_folded_if(&mut self, scope: &mut Scope, expr: ExprId, holds: bool, then: &[StmtId], else_: Option<ExprId>, origin: Origin) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(expr);
        let (file, span) = (self.state.design.decl_data(node.owner).file, node.span);
        if holds {
            return self.lower_block(scope, file, then, span);
        }
        match else_ {
            Some(else_) => self.lower_expr(scope, else_),
            None => Some(self.emit(Expr::Unit, ValueType::Unit, origin)),
        }
    }

    /// An `if` with both arms lowered, each under its own condition.
    fn lower_if(&mut self, scope: &mut Scope, expr: ExprId, cond: ExprId, then: &[StmtId], else_: Option<ExprId>, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(expr);
        let (file, span) = (self.state.design.decl_data(node.owner).file, node.span);
        let source = cond;
        let cond = self.lower_expr(scope, cond);
        scope.values.decisions += 1;
        let arm = AnalogLowering::enter_arm(scope, cond, true);
        scope.analog.guards.push(ArmCondition::If { condition: source, holds: true });
        let then = self.lower_block(scope, file, then, span);
        scope.analog.guards.pop();
        AnalogLowering::leave_arm(scope, arm);
        let arm = AnalogLowering::enter_arm(scope, cond, false);
        scope.analog.guards.push(ArmCondition::If { condition: source, holds: false });
        let otherwise = match else_ {
            Some(else_) => self.lower_expr(scope, else_),
            None => Some(self.emit(Expr::Unit, ValueType::Unit, origin.clone())),
        };
        scope.analog.guards.pop();
        AnalogLowering::leave_arm(scope, arm);
        scope.values.decisions -= 1;
        let (cond, then, otherwise) = (cond?, then?, otherwise?);
        let (then, otherwise) = (self.fit_to(then, &ty), self.fit_to(otherwise, &ty));
        let whole = self.emit(Expr::If { cond, then, otherwise }, ty, origin);
        self.note_branch(whole, cond, then, otherwise);
        Some(whole)
    }

    /// A method call the checker typed by a row of Appendix H that lowers to a math function or an IR method; the rows that
    /// are `Option`s lower on their pair.
    fn lower_numeric_method(&mut self, scope: &mut Scope, expr: ExprId, recv: ExprId, args: &[crate::pom::Arg], name: &str, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(expr);
        let (file, span) = (self.state.design.decl_data(node.owner).file, node.span);
        let Some(method) = self.state.types.method(expr) else {
            // `x.encode()` of a `T: Repr` parameter has no method row: its levels are written out per instance
            if args.is_empty() && let Some(levels) = self.lower_parameter_encode(scope, expr, recv, name, origin.clone()) {
                return levels;
            }
            self.refuse(file, span, ValueForm::MethodOutsideCatalogue);
            return None;
        };
        let f = match method {
            Method::Exp => MathFn::Exp,
            Method::Ln => MathFn::Ln,
            Method::Sqrt => MathFn::Sqrt,
            Method::Abs => MathFn::Abs,
            row if Self::ir_method(row).is_some() => {
                let method = Self::ir_method(row)?;
                let receiver = self.lower_expr(scope, recv);
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.lower_expr(scope, arg.value));
                }
                let (receiver, args) = (receiver?, lowered.into_iter().collect::<Option<Vec<_>>>()?);
                return Some(self.emit(Expr::Method { method, receiver, args }, ty, origin));
            }
            // Lowered as a flag and a payload from msl-digital T27 on.
            Method::IsSome | Method::IsNone | Method::UnwrapOr | Method::Unwrap | Method::Expect | Method::UnwrapOrElse => {
                let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                return self.lower_option_method(scope, method, recv, &args, ty, origin);
            }
            // The rest are `Option`s (`checked_*`, `map`, `and_then`, `or`, `then`, `then_some`): they lower
            // on their pair, never as one value.
            _ => {
                self.refuse_with(file, span, ValueForm::RowReadAsValueRatherThanAs, [format!("{:?}", method)]);
                return None;
            }
        };
        let recv = self.lower_expr(scope, recv)?;
        Some(self.emit(Expr::Math { f, args: vec![recv] }, ty, origin))
    }

    /// A field read: of a struct or tuple value, of one element of an array of bundles, or of a node of a bundle.
    fn lower_field_expression(&mut self, scope: &mut Scope, expr: ExprId, recv: ExprId, name: &str, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        if matches!(self.typed(recv), Some(Ty::Struct(_) | Ty::Tuple(_))) && self.structure().bundle_leaf(scope, expr).is_none() {
            return self.lower_field_value(scope, recv, name, ty, origin);
        }
        // a field of an element of an array of bundles (`ps[k].b`) is one element of its leaf (struct of arrays, `elements.rs`)
        if self.structure().bundle_leaf(scope, expr).is_some_and(|place| !place.elements.is_empty()) {
            return self.structure().lower_bundle_element(scope, expr, origin);
        }
        self.structure().lower_field(scope, expr, ty, origin)
    }

    /// A method call, by what the checker resolved it to: a method of the project, a method through a bound, a row of the
    /// sequence methods, a read of a built-in, or a row of Appendix H.
    fn lower_method_expression(&mut self, scope: &mut Scope, expr: ExprId, recv: ExprId, args: &[crate::pom::Arg], name: &str, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        if let Some(Callee::Method(function)) = self.state.types.callee(expr) {
            let arguments: Vec<ExprId> = args.iter().map(|a| a.value).collect();
            return self.lower_method_call(scope, expr, function, recv, &arguments, origin);
        }
        if let Some(Callee::Bound { item }) = self.state.types.callee(expr) {
            let arguments: Vec<ExprId> = args.iter().map(|a| a.value).collect();
            return self.lower_bound_call(scope, expr, item, recv, &arguments, origin);
        }
        if self.state.types.method(expr).is_some_and(Method::is_sequence_row) {
            let method = self.state.types.method(expr)?;
            let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
            return self.lower_sequence_method(scope, expr, method, recv, &args, ty, origin);
        }
        if self.state.types.builtin(expr).is_some() {
            let builtin = self.state.types.builtin(expr)?;
            return self.digital().lower_builtin_read(scope, expr, builtin, recv, ty, origin);
        }
        self.lower_numeric_method(scope, expr, recv, args, name, ty, origin)
    }

    /// A literal, folded to its constant.
    fn lower_literal(&mut self, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        match self.state.consts.value(expr, &Bindings::new()) {
            Ok(value) => Some(self.constant_expr(value, ty, origin)),
            Err(fault) => {
                self.const_fault(&fault, "a literal");
                None
            }
        }
    }

    /// A block as an expression, one level deeper in the analog block it sits in.
    fn lower_block_expression(&mut self, scope: &mut Scope, file: FileId, stmts: &[StmtId], span: Span) -> Option<LoweredExpr> {
        scope.analog.block_depth += 1;
        let lowered = self.lower_block(scope, file, stmts, span);
        scope.analog.block_depth -= 1;
        lowered
    }

    /// An `as` cast of `operand` to the type of the expression.
    fn lower_cast(&mut self, scope: &mut Scope, operand: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let operand = self.lower_expr(scope, operand)?;
        Some(self.emit(Expr::Cast { expr: operand, to: ty.clone() }, ty, origin))
    }

    /// A `match` that is a decision of the code it sits in.
    fn lower_decided_match(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[crate::pom::Arm], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        scope.values.decisions += 1;
        let lowered = self.lower_match(scope, scrutinee, arms, ty, None, origin);
        scope.values.decisions -= 1;
        lowered
    }

    /// A read of a param of the module: its index in the body, the constant its placement bound, or the value it was given
    /// read in place.
    fn lower_param_path(&mut self, scope: &mut Scope, decl: DeclId, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        match (scope.values.params.get(&decl), self.state.structure.frames.get(scope.values.frame).and_then(|f| f.params.get(&decl)).copied()) {
            (Some(index), _) if scope.values.frame == 0 => Some(self.emit(Expr::Param(*index), ty, origin)),
            // A param of a hoisted module is the constant its placement bound (`hoist.rs`).
            (_, Some(value)) => Some(self.constant_expr(value, ty, origin)),
            // a param of a reach frame whose given value does not fold is read in place (`reach/placement_params.rs`)
            _ if scope.values.frame != 0 && self.state.structure.frames.get(scope.values.frame).is_some_and(|frame| frame.deferred_params.contains_key(&decl)) => self.structure().lower_deferred_param(scope, decl),
            _ => {
                self.refuse(scope.values.file, span, ValueForm::ParamOfAnotherModule);
                None
            }
        }
    }

    /// A read of a port or a signal: an element of the carrier a generation made, the carrier itself, the expression a
    /// hoisted module's caller gave it, or the empty array.
    fn lower_carrier_path(&mut self, scope: &mut Scope, decl: DeclId, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        // a carrier declared inside generation, read inside its loops, is the iteration's element (§8.2.23)
        if matches!(self.state.design.decl_data(decl).kind, DeclKind::Signal(_)) && !self.digital().generated_carrier_levels(scope, expr).is_empty() {
            return match self.digital().element_lookup(scope, expr) {
                crate::elab::digital::elements::Lookup::Found(element) => Some(self.digital().carrier_element_read(scope, element, origin)),
                _ => None,
            };
        }
        match (self.structure().frame_carrier(scope.values.frame, decl), self.structure().input_of(scope, decl)) {
            (Some(index), _) if scope.analog.in_analog_block && self.state.digital.structure.carriers[index as usize].count.is_some() => Some(self.digital().elements_of_array(scope, index, ElementPart::Value, origin)),
            (Some(index), _) => Some(self.digital().carrier_read(index, origin)),
            // A value input of a hoisted module: the caller's expression, in the caller's frame.
            (None, Some(Some(given))) => self.structure().in_caller(scope, |this, scope| this.values().lower_expr(scope, given)),
            (None, Some(None)) => {
                self.refuse(scope.values.file, span, ValueForm::ValueInputLeftOutReadAsValue);
                None
            }
            // an array of carriers of length 0 read whole is the empty array (§1.2.3)
            (None, None) if self.state.structure.frames.get(scope.values.frame).is_some_and(|f| f.empty_arrays.contains(&decl)) => Some(self.emit(Expr::Array(Vec::new()), ty, origin)),
            (None, None) => {
                self.refuse(scope.values.file, span, ValueForm::NodeOrPortReadAsValue);
                None
            }
        }
    }

    /// A read of a function parameter or a binding: the constant of the unrolled iteration or call, the carrier a handle was
    /// bound to, a local, a `let` of the body written in generation or not, or a block-local `let` read as its source.
    fn lower_binding_path(&mut self, scope: &mut Scope, decl: DeclId, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        // The loop variable of an unrolled analog loop and the parameter of an expanded recursion
        // are the constant of this iteration or call (`unroll.rs`).
        if let Some(value) = scope.analog.values.get(&decl).copied() {
            return Some(value);
        }
        // a handle read: the carrier the caller passed (`handles.rs`)
        if matches!(self.state.design.decl_data(decl).kind, DeclKind::FnParam(_)) && let Some(bound) = scope.digital.handles.get(&decl).copied() {
            return self.lower_expr(scope, bound);
        }
        if let Some(value) = scope.values.unrolled.get(decl) {
            return Some(self.constant_expr(value, ty, origin));
        }
        match scope.values.local_index.get(&decl) {
            Some(index) => {
                let ty = scope.values.locals[*index as usize].ty.clone();
                Some(self.emit(Expr::Local(*index), ty, origin))
            }
            // a `let` written in generation: the iteration's value inside its loops, the array of them outside (§8.2.23)
            None if self.body_let_value(decl).is_some() && self.state.structure.generated.of(self.state.design.decl_data(decl).parent?).is_some() => self.structure().lower_generated_let(scope, expr, decl, ty, origin),
            // A module-body `let`: inlined at each read (Appendix C.1.1, `hoist.rs`).
            None if self.body_let_value(decl).is_some() => {
                let value = self.body_let_value(decl)?;
                self.structure().lower_body_let(scope, value)
            }
            // A block-local `let` in a standalone scope is its source expression (`derivatives.rs`).
            None if scope.digital.no_locals && scope.values.let_sources.contains_key(&decl) => {
                let source = scope.values.let_sources[&decl];
                self.lower_expr(scope, source)
            }
            None => {
                self.refuse(scope.values.file, span, ValueForm::NameWithNoValueHere);
                None
            }
        }
    }

    /// A call: of a function of the reachable set (one `Call` node, the function lowered once
    /// however many bodies call it), or of a core primitive or a probe (see `analog.rs`).
    fn lower_call(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], class: CallClass, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        // a `fn` called in a value that is differentiated is expanded where it stands (`expansion.rs`)
        if scope.analog.differentiated && class != CallClass::Probe && let Some(expanded) = self.analog().expand_function_call(scope, expr) {
            return expanded;
        }
        // a call of a generic function is typed (generics T20 to T23) and instantiated by T33 and T34: until
        // then the elaborator names it and stops
        if let Some(Callee::Function(function)) = self.state.types.callee(expr)
            && self.state.design.decl_data(function).generics_span.is_some()
        {
            // a generic `extern` function types like any generic function but has no body to instantiate: its call is
            // refused by name until the feature that runs it (`params-and-io`: `parse_table`, `parse_hex`, `table_model`)
            if matches!(&self.state.design.decl_data(function).kind, DeclKind::Fn(f) if f.abi.is_some()) {
                let name = self.state.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                self.refuse_with(scope.values.file, span, ValueForm::CallOfGenericExternFunction, [name.to_string()]);
                return None;
            }
            // the instance is folded and checked (`instances.rs`) and lowered once for its arguments (`variants.rs`)
            let instance = self.functions().concrete_instance(expr)?;
            // a function generic over a nature takes handles: inlined for the nature it is instantiated at
            if self.digital().is_inlined(instance.item) {
                let outer = self.elaboration.state.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
                let inlined = self.digital().inline_emitting_value(scope, instance.item, args, origin);
                self.state.consts.set_substitution(outer);
                return inlined;
            }
            let lowered = self.lower_arguments(scope, expr, instance.item)?;
            let (id, ret) = self.functions().request_instance(instance.item, instance.arguments, Half::Whole)?;
            return Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin));
        }
        // an associated function of a generic `impl` runs for the arguments the checker read from the call (`Logic::from(b)`)
        if let Some(Callee::Function(function)) = self.state.types.callee(expr)
            && self.is_generic_impl_item(function)
            && let Some(recorded) = self.state.types.instantiation(expr).cloned()
        {
            let values = recorded.arguments.iter().map(|value| self.functions().concrete_argument(value, scope.values.file, span)).collect::<Option<Vec<_>>>()?;
            let lowered = self.lower_arguments(scope, expr, function)?;
            let (id, ret) = self.functions().request_instance(function, values, Half::Whole)?;
            return Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin));
        }
        match (class, self.state.types.callee(expr)) {
            (CallClass::Probe, _) => self.analog().lower_probe(scope, expr, ty, origin),
            // A function that emits is no IR function: its call is inlined where it stands (§19 rule 10).
            (_, Some(Callee::Function(function))) if self.digital().is_inlined(function) => self.digital().inline_emitting_value(scope, function, args, origin),
            // A recursion reached from an `analog` block is expanded where it is called (§3 rule 12, `unroll.rs`).
            (_, Some(Callee::Function(function))) if scope.analog.in_analog_block && self.analog().reaches_recursion(function) => self.analog().lower_expanded_call(scope, expr, function, origin),
            (_, Some(Callee::Function(function))) => {
                let lowered = self.lower_arguments(scope, expr, function)?;
                let (id, ret) = self.functions().request(function, Half::Whole)?;
                Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
            }
            (_, Some(Callee::Core(id))) => self.analog().lower_core(scope, expr, id, args, ty, origin),
            // A module with a result is hoisted: its result is read where the placement stands.
            (_, Some(Callee::Module(_)) | None) if self.structure().result_callee(expr).is_some() => self.structure().lower_result(scope, expr),
            (_, Some(Callee::Module(_))) => {
                self.refuse(scope.values.file, span, ValueForm::PlacementUsedAsValue);
                None
            }
            // A method of the project is typed (generics T25) and lowered by its own task (T35).
            // `Tr::f(x, ..)` by path: the first argument is the receiver
            (_, Some(Callee::Method(function))) if let Some((receiver, rest)) = args.split_first() => self.lower_method_call(scope, expr, function, *receiver, rest, origin),
            (_, Some(Callee::Bound { item })) if let Some((receiver, rest)) = args.split_first() => self.lower_bound_call(scope, expr, item, *receiver, rest, origin),
            (_, Some(Callee::Method(_) | Callee::Bound { .. })) => {
                self.refuse(scope.values.file, span, ValueForm::LoweredMethodCall);
                None
            }
            // Typed by `types::intrinsic`, lowered by its §22.1 contract (`intrinsics.rs`).
            (_, Some(Callee::Intrinsic(intrinsic))) => self.lower_intrinsic(scope, expr, intrinsic, args, origin),
            (_, None) if self.state.types.default_of(expr).is_some() => Some(self.emit(Expr::Default(ty.clone()), ty, origin)),
            (_, None) if let ExprKind::Call { callee, .. } = &self.state.design.expr_node(expr).kind && let Some(owner) = self.constructed_decl(*callee) => self.lower_constructor_call(scope, args, owner, ty, origin),
            (_, None) if let Some(decl) = self.called_local(scope, expr) => self.lower_value_call(scope, expr, decl, args, origin),
            // a callee that is an expression whose value is a function (`(|y: f64| y * 2.0)(x)`, `doubler()(x)`, `(twice)(x)`)
            (_, None) if let ExprKind::Call { callee, .. } = &self.state.design.expr_node(expr).kind && self.is_function_valued(*callee) => {
                let callee = *callee;
                self.lower_expression_call(scope, callee, args, origin)
            }
            (_, None) => {
                self.refuse(scope.values.file, span, ValueForm::CallTypeCheckerDidNotResolve);
                None
            }
        }
    }

    /// The arguments of the call `expr` of `function`, one IR argument per parameter in parameter
    /// order, from the checker's binding of the call (§3.2): an `Option<T>` parameter takes two,
    /// its flag and its payload, a parameter left out `(false, zero)`. `None` after a diagnostic.
    pub(in crate::elab) fn lower_arguments(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Vec<LoweredExpr>> {
        let Some(binding) = self.state.types.binding(expr).map(<[_]>::to_vec) else {
            self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, ValueForm::CallWhoseArgumentsTypeCheckerDidNot);
            return None;
        };
        let params: Vec<DeclId> = self.state.design.decl_data(function).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut lowered = Vec::new();
        let mut ok = true;
        for (param, slot) in params.iter().zip(binding) {
            match self.typed_local(*param) {
                Some(Ty::Option(payload)) => match self.lower_option_slot(scope, slot, &payload, expr) {
                    Some(pair) => lowered.extend(pair),
                    None => ok = false,
                },
                _ => match slot.and_then(|arg| self.lower_expr(scope, arg)) {
                    Some(arg) => lowered.push(arg),
                    None => ok = false,
                },
            }
        }
        ok.then_some(lowered)
    }

    /// Allocate the local a `Binding` declaration names: its name and its type, in declaration
    /// order, assignable exactly when the body assigns it (§1.5.12). `None` after a diagnostic
    /// when the checker typed it with a non-value type.
    pub(in crate::elab) fn bind_local(&mut self, scope: &mut Scope, binding: DeclId) -> Option<u32> {
        let assignable = self.state.types.is_assigned(binding);
        self.bind_local_as(scope, binding, assignable)
    }

    /// `bind_local` with the assignable flag given: a loop's own variable is bound with `false`
    /// and copied (`assignable_copy`), so the loop's counter is never written.
    pub(super) fn bind_local_as(&mut self, scope: &mut Scope, binding: DeclId, assignable: bool) -> Option<u32> {
        let data = self.state.design.decl_data(binding);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (file, span) = (data.file, data.span);
        let Some(ty) = self.typed_local(binding).and_then(|t| self.value_type(&t)) else {
            self.refuse(file, span, ValueForm::NameWhoseTypeIsNotValueType);
            return None;
        };
        let index = scope.values.locals.len() as u32;
        scope.values.locals.push(Local { name, ty, mutable: assignable });
        scope.values.local_index.insert(binding, index);
        Some(index)
    }

    /// The statements of a block as one expression: a block with statements is a `Block`, one
    /// that is only its value is that value (`{ x }` is `x`).
    pub(super) fn lower_block(&mut self, scope: &mut Scope, file: FileId, stmts: &[StmtId], span: Span) -> Option<LoweredExpr> {
        let origin = self.state.locator.origin(file, span);
        let (stmts, value) = self.lower_statements(scope, stmts)?;
        match (stmts.is_empty(), value) {
            (true, Some(value)) => Some(value),
            (_, value) => {
                let ty = value.map_or(ValueType::Unit, |v| self.type_of(v));
                let block = self.emit(Expr::Block { stmts: stmts.clone(), value }, ty, origin);
                self.note_block(block, &stmts, value);
                Some(block)
            }
        }
    }

    /// The statements of a block in order and its value (the last expression without `;`).
    /// `None` after the diagnostics of every statement that could not be lowered.
    pub(in crate::elab) fn lower_statements(&mut self, scope: &mut Scope, stmts: &[StmtId]) -> Option<(Vec<LoweredStmt>, Option<LoweredExpr>)> {
        let mut out = Vec::new();
        let mut value = None;
        let mut ok = true;
        let outer = std::mem::take(&mut scope.values.pending);
        for (i, stmt) in stmts.iter().enumerate() {
            let start = out.len();
            let node = self.state.design.stmt_node(*stmt).clone();
            let file = self.state.design.decl_data(node.owner).file;
            let carried = std::mem::replace(&mut ok, true);
            match &node.kind {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => ok &= self.lower_let(scope, &mut out, *pat, *init, file, node.span),
                // `let x;`: a local first assigned later (§1.7.5, `control.rs`).
                StmtKind::Let { pat, init: Some(init), else_: Some(otherwise), .. } => ok &= self.lower_let_else(scope, &mut out, *pat, *init, otherwise, file),
                StmtKind::Let { pat, init: None, else_: None, .. } => ok &= self.lower_deferred_let(scope, &mut out, *pat, file, node.span),
                // `place = value;` and `place op= value;` (§1.5.12, `control.rs`).
                StmtKind::Expr { expr, .. } if matches!(self.state.design.expr_node(*expr).kind, ExprKind::Assign { .. }) => ok &= self.lower_assignment(scope, &mut out, *expr),
                // A loop of an `analog` block that contributes or places is its unrolling (`unroll.rs`).
                StmtKind::Expr { expr, .. } if let Some(lowered) = self.analog().lower_analog_loop(scope, &mut out, *expr) => ok &= lowered,
                // `emit!` and a call of a function that emits are statements (`records.rs`).
                StmtKind::Expr { expr, .. } if let Some(lowered) = self.digital().lower_record_statement(scope, &mut out, *expr) => ok &= lowered,
                // `bound_step(dt);` is a statement of the analog block (`analog.rs`).
                StmtKind::Expr { expr, .. } if let Some(lowered) = self.analog().lower_bound_step(scope, &mut out, *expr) => ok &= lowered,
                StmtKind::Expr { expr, semi } => match self.lower_expr(scope, *expr) {
                    Some(lowered) if i + 1 == stmts.len() && !semi => value = Some(lowered),
                    Some(lowered) => out.push(self.stmt(Stmt::Expr(lowered))),
                    None => ok = false,
                },
                StmtKind::Empty => {}
                StmtKind::Contribution { lhs, rhs } => ok &= self.analog().lower_contribution(scope, &mut out, *lhs, *rhs, node.span),
                StmtKind::Commit { place, value, after } => ok &= self.digital().lower_commit(scope, &mut out, *place, *value, *after),
                _ => {
                    self.refuse(file, node.span, ValueForm::Statement);
                    ok = false;
                }
            }
            // a statement that failed to lower while nothing was reported is refused by name: nothing vanishes silently (a failure that rests on an earlier diagnostic, such as an unreadable file, keeps that one)
            if !ok && self.state.diagnostics.is_empty() {
                self.refuse_silent_failure(file, node.span, ValueForm::Statement);
            }
            ok &= carried;
            // a statement that is the value of its block leaves the flag to the code that takes the value
            if value.is_none() && scope.values.inline_exits.last_mut().is_some_and(|frame| std::mem::take(&mut frame.loop_flagged)) {
                let origin = self.state.locator.origin(file, node.span);
                out.push(self.leave_if_inlined_return_ran(scope, origin));
            }
            let declared = std::mem::take(&mut scope.values.pending);
            out.splice(start..start, declared);
        }
        scope.values.pending = outer;
        ok.then_some((out, value))
    }

    /// `let pat = init;`: a name, an `Option` (a flag local and a payload local) or a tuple
    /// (one local per component). `false` after a diagnostic.
    fn lower_let(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, pat: crate::pom::PatId, init: ExprId, file: FileId, span: Span) -> bool {
        match self.state.design.pat_node(pat).kind.clone() {
            PatKind::Ident { binding, sub: None, .. } if matches!(self.typed_local(binding), Some(Ty::Str)) => self.digital().lower_text_let(scope, binding, init),
            PatKind::Ident { binding, sub: None, .. } if matches!(self.typed_local(binding), Some(Ty::Option(_))) => self.lower_option_let(scope, out, binding, init),
            // An iterator or a range holds no value: the name stands for the chain it was bound to (`iterators.rs`).
            PatKind::Ident { binding, sub: None, .. } if self.is_sequence_binding(binding) => {
                scope.values.let_sources.insert(binding, init);
                true
            }
            // A `let` bound to a node holds no value: the name stands for the node, as a handle does (`let y = x; v(y)`, §6 rule 4)
            PatKind::Ident { binding, sub: None, .. } if matches!(self.typed_local(binding), Some(Ty::Node(_) | Ty::AnyNode)) => {
                scope.digital.handles.insert(binding, init);
                true
            }
            PatKind::Ident { binding, sub: None, .. } => {
                let local = self.bind_local(scope, binding);
                scope.values.let_sources.insert(binding, init);
                let name = self.state.design.decl_data(binding).name.as_ref().map(|n| n.text.clone());
                scope.analog.limit_name = name.map(|n| (self.peel_parens(init), n));
                let init = self.lower_expr(scope, init);
                scope.analog.limit_name = None;
                match (local, init) {
                    (Some(local), Some(init)) => {
                        // A local is what its initializer is.
                        let mutable = scope.values.locals[local as usize].mutable;
                        let ty = self.type_of(init);
                        scope.values.locals[local as usize].ty = ty;
                        if !mutable {
                            scope.values.local_inits.insert(local, init);
                        }
                        out.push(self.stmt(Stmt::Let { local, init }));
                        true
                    }
                    _ => false,
                }
            }
            // `let (a, b) = (x, y);` is one local per component; any other value is bound once and
            // taken apart by its pattern (`aggregate.rs`).
            PatKind::Tuple(pats) if self.is_component_let(&pats, init) => self.lower_tuple_let(scope, out, &pats, init, file, span),
            PatKind::Tuple(_) | PatKind::TupleStruct { .. } | PatKind::Struct { .. } | PatKind::Array(_) => self.lower_destructuring_let(scope, out, pat, init),
            _ => {
                self.refuse(file, span, ValueForm::LetPatternThatIsNotName);
                false
            }
        }
    }

    pub(in crate::elab) fn binary_op(op: BinOp) -> BinaryOp {
        match op {
            BinOp::Add => BinaryOp::Add,
            BinOp::Sub => BinaryOp::Sub,
            BinOp::Mul => BinaryOp::Mul,
            BinOp::Div => BinaryOp::Div,
            BinOp::Lt => BinaryOp::Lt,
            BinOp::Le => BinaryOp::Le,
            BinOp::Gt => BinaryOp::Gt,
            BinOp::Ge => BinaryOp::Ge,
            BinOp::Eq => BinaryOp::Eq,
            BinOp::Ne => BinaryOp::Ne,
            BinOp::And => BinaryOp::And,
            BinOp::Or => BinaryOp::Or,
            BinOp::Rem => BinaryOp::Rem,
            BinOp::BitXor => BinaryOp::BitXor,
            BinOp::BitAnd => BinaryOp::BitAnd,
            BinOp::BitOr => BinaryOp::BitOr,
            BinOp::Shl => BinaryOp::Shl,
            BinOp::Shr => BinaryOp::Shr,
        }
    }

    /// Whether `expr` calls a method on a value of a module type: a template, which exists at elaboration only.
    fn is_template_method(&self, expr: ExprId) -> bool {
        matches!(&self.state.design.expr_node(expr).kind, ExprKind::MethodCall { recv, .. } if matches!(self.typed(*recv), Some(Ty::Module(_))))
    }

    /// Whether `expr` reads only literals and constants through operators, casts and the
    /// catalogue's methods: an expression `ConstEval` folds (DIG-08 AC 2). A call, a block or an
    /// `if` is kept as written.
    fn constant_form(&self, expr: ExprId) -> bool {
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => matches!(lit.kind, LitKind::Int(_) | LitKind::Float { .. } | LitKind::Bool(_)),
            ExprKind::Paren(inner) => self.constant_form(*inner),
            ExprKind::Path(path) => self.state.types.width(expr).is_some() || matches!(self.state.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if matches!(self.state.design.decl_data(d).kind, DeclKind::Const(_) | DeclKind::Variant(_))),
            ExprKind::Unary { op: UnOp::Neg | UnOp::Not, expr: operand } => self.constant_form(*operand),
            ExprKind::Binary { lhs, rhs, .. } => self.constant_form(*lhs) && self.constant_form(*rhs),
            ExprKind::Cast { expr: operand, .. } => self.constant_form(*operand),
            ExprKind::MethodCall { recv, args, .. } => self.state.types.method(expr).is_some() && self.constant_form(*recv) && args.iter().all(|a| self.constant_form(a.value)),
            _ => false,
        }
    }

    /// A folded value as the IR expression of its type; an integer takes `ty`, its expression's
    /// value type.
    pub(in crate::elab) fn constant_expr(&mut self, value: Constant, ty: ValueType, origin: Origin) -> LoweredExpr {
        match value {
            Constant::Real(r) => self.emit(Expr::Real(r), ValueType::F64, origin),
            Constant::Bool(b) => self.emit(Expr::Bool(b), ValueType::Bool, origin),
            // A unit variant is folded to its position among the enum's variants.
            Constant::Int(v) if let ValueType::Enum(id) = ty => self.emit(Expr::VariantOf { ty: id, variant: v as u32, fields: Vec::new() }, ty, origin),
            Constant::Int(v) => self.emit(Expr::Int(v), ty, origin),
            Constant::FloatBits(bits) => self.emit(Expr::FloatBits(bits), ty, origin),
            Constant::Wide(id) => self.emit(Expr::Wide(id), ty, origin),
            // A composite constant of the IR's arena (a param of a hoisted placement) is written out as the expression that builds it
            // (`consts/imported.rs`), an `Option` as its packed pair.
            Constant::Aggregate(id) => {
                let tree = self.imported_tree(value);
                match self.tree_expr(&tree, &ty, &origin) {
                    Some(built) => built,
                    // the checker's types exclude a composite with no form at its type: reaching here is the elaborator's bug, said
                    // as one, and the run gives no `Ir` (the stand-in below never reaches a backend)
                    None => {
                        self.internal_error(&format!("the composite constant agg{} has no constant form at its type {ty:?}", id.0));
                        self.emit(Expr::Int(0), ty, origin)
                    }
                }
            }
        }
    }

    /// A name: a `const` (folded), a param of the body (a read), a local.
    fn lower_path(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let res = self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        if let Some(levels) = self.state.consts.parameter_levels(expr) {
            return Some(self.constant_expr(Constant::Int(i128::from(levels)), ty, origin));
        }
        let Resolution::Decl(decl) = res else {
            self.refuse(scope.values.file, node.span, ValueForm::BuiltInNameUsedAsValue);
            return None;
        };
        match &self.state.design.decl_data(decl).kind {
            // A composite `const` is written out where it is read (`consts/imported.rs`).
            DeclKind::Const(_) if self.state.consts.is_composite_constant(decl) => self.lower_composite_constant(scope, decl, expr, ty, origin),
            DeclKind::Const(_) => match self.state.consts.constant(decl) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const`");
                    None
                }
            },
            // A unit variant is a value of its enum (§1.3.5).
            DeclKind::Variant(_) => match (EnumLayout::variant_tag(self.state.design, decl), &ty) {
                (Some(position), ValueType::Enum(id)) => Some(self.emit(Expr::VariantOf { ty: *id, variant: position as u32, fields: Vec::new() }, ty, origin)),
                _ => {
                    self.refuse(scope.values.file, node.span, ValueForm::VariantOutsideItsEnum);
                    None
                }
            },
            DeclKind::Param(_) => self.lower_param_path(scope, decl, expr, ty, origin),
            DeclKind::Port(_) | DeclKind::Signal(_) => self.lower_carrier_path(scope, decl, expr, ty, origin),
            DeclKind::FnParam(_) | DeclKind::Binding(_) => self.lower_binding_path(scope, decl, expr, ty, origin),
            // a const parameter read in the body of an instance is the constant of its argument (T33, GN-02)
            DeclKind::GenericParam(_) => match self.state.consts.substitution().get(decl) {
                Some(crate::types::GenericValue::Const(crate::types::ConstTerm::Value(value))) => Some(self.constant_expr(piperine_ir::Constant::Int(*value), ty, origin)),
                Some(crate::types::GenericValue::Const(crate::types::ConstTerm::Bool(flag))) => Some(self.constant_expr(piperine_ir::Constant::Bool(*flag), ty, origin)),
                _ => {
                    self.refuse(scope.values.file, node.span, ValueForm::GenericParameterReadAsValue);
                    None
                }
            },
            DeclKind::Fn(_) => self.lower_function_value(scope, decl, ty, node.span, origin),
            DeclKind::Struct(_) => self.lower_unit_struct(ty, origin),
            _ => {
                self.refuse(scope.values.file, node.span, ValueForm::NameAsValue);
                None
            }
        }
    }
}
