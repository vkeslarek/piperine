//! Methods and associated items in the elaborator (part I §3.1, §3.7, GN-03): a method call is a
//! call of the method's IR function with the receiver first, as `self` is the first parameter; a
//! function a trait's default body gives is instantiated for the type of the receiver, which stands
//! for `Self`. `Type::f(..)` is an ordinary call and `Type::NAME` a folded constant, which the call
//! and the constant folder already lower.

use piperine_ir::{Expr, ExprId as LoweredExpr};

use super::elaborator::Elaboration;
use super::instances::ConcreteValue;
use super::lower::Scope;
use super::variants::Half;
use crate::pom::{DeclId, DeclKind, ExprId};
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// The call `receiver.f(arguments)` of the method `function`: the receiver and the arguments lowered
    /// in order, a `Call` of the method's variant (for a function of a trait, the instance for the type
    /// of the receiver). `None` after a diagnostic.
    pub(super) fn lower_method_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        let impl_arguments = if self.is_generic_impl_item(function) { self.method_impl_arguments(scope, expr, function, receiver)? } else { Vec::new() };
        // a method called in a value that is differentiated is expanded where it stands (`expansion.rs`)
        if scope.differentiated {
            let instance = if self.is_trait_function(function) { vec![ConcreteValue::Type(self.typed(receiver)?)] } else { impl_arguments };
            return self.expand_method_call(scope, expr, function, receiver, arguments, instance);
        }
        // a method with a handle for `self` (an `impl` on a nature) is inlined at the call (`handles.rs`)
        if self.is_inlined(function) {
            let mut all = vec![receiver];
            all.extend_from_slice(arguments);
            return self.inline_emitting_value(scope, function, &all, origin);
        }
        let mut lowered = vec![self.lower_expr(scope, receiver)?];
        for argument in arguments {
            lowered.push(self.lower_expr(scope, *argument)?);
        }
        // a trait's own function runs for the type of its receiver, which is `Self` there
        let instance = if self.is_trait_function(function) { vec![ConcreteValue::Type(self.typed(receiver)?)] } else { impl_arguments };
        let (id, ret) = self.request_instance(function, instance, Half::Whole)?;
        Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
    }

    /// The arguments the generic `impl` of the method `function` takes from the type of `receiver`, concrete. `None`
    /// after a diagnostic.
    pub(super) fn method_impl_arguments(&mut self, scope: &Scope, expr: ExprId, function: DeclId, receiver: ExprId) -> Option<Vec<ConcreteValue>> {
        let receiver_type = self.typed(receiver)?;
        let span = self.design.expr_node(expr).span;
        let Some(values) = self.types.impl_arguments(function, &receiver_type) else {
            self.unsupported(scope.file, span, "a method of a generic impl whose parameters the receiver leaves open");
            return None;
        };
        values.iter().map(|value| self.concrete_argument(value, scope.file, span)).collect()
    }

    /// The call `receiver.f(arguments)` through a bound (`Callee::Bound { item }`, the trait's own function): the
    /// receiver's type is concrete in the instance being lowered, so the call is the `impl`'s item for that type, or
    /// the trait's default body with `Self` bound to it (static dispatch, GN-04). `None` after a diagnostic.
    pub(super) fn lower_bound_call(&mut self, scope: &mut Scope, expr: ExprId, item: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        let receiver_type = self.typed(receiver)?;
        let Some(target) = self.bound_implementation(expr, item, &receiver_type) else {
            self.unsupported(scope.file, span, "a method call through a bound that no impl gives for this type");
            return None;
        };
        self.lower_method_call(scope, expr, target, receiver, arguments, origin)
    }

    /// The item that runs for the call `expr` of the trait function `item` through a bound on a receiver of type `receiver`
    /// (`consts/dispatch.rs`).
    pub(super) fn bound_implementation(&mut self, expr: ExprId, item: DeclId, receiver: &Ty) -> Option<DeclId> {
        self.consts.bound_implementation(expr, item, receiver)
    }

    /// Whether `function` is declared in a trait.
    pub(super) fn is_trait_function(&self, function: DeclId) -> bool {
        self.design.decl_data(function).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Trait(_)))
    }

    /// Whether `function` is written in an `impl` that has generic parameters.
    pub(super) fn is_generic_impl_item(&self, function: DeclId) -> bool {
        self.design.decl_data(function).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Impl(_)) && self.design.decl_data(parent).generics_span.is_some())
    }
}
