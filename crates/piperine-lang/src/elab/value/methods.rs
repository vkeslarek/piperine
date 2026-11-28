//! Methods and associated items in the elaborator (part I §3.1, §3.7, GN-03): a method call is a
//! call of the method's IR function with the receiver first, as `self` is the first parameter; a
//! function a trait's default body gives is instantiated for the type of the receiver, which stands
//! for `Self`. `Type::f(..)` is an ordinary call and `Type::NAME` a folded constant, which the call
//! and the constant folder already lower.

use super::ValueLowering;
use piperine_ir::{Expr, ExprId as LoweredExpr};

use crate::elab::functions::instances::ConcreteValue;
use crate::elab::elaborator::Scope;
use crate::elab::functions::variants::Half;
use crate::refusal::forms::GenericsForm;
use crate::pom::{DeclId, ExprId};
use crate::types::Ty;

impl ValueLowering<'_, '_, '_> {
    /// The call `receiver.f(arguments)` of the method `function`: the receiver and the arguments lowered
    /// in order, a `Call` of the method's variant (for a function of a trait, the instance for the type
    /// of the receiver). `None` after a diagnostic.
    pub(super) fn lower_method_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        let impl_arguments = if self.is_generic_impl_item(function) { self.method_impl_arguments(scope, expr, function, receiver)? } else { Vec::new() };
        // a method called in a value that is differentiated is expanded where it stands (`expansion.rs`)
        if scope.analog.differentiated {
            let instance = if self.is_trait_function(function) { vec![ConcreteValue::Type(self.typed(receiver)?)] } else { impl_arguments };
            return self.analog().expand_method_call(scope, expr, function, receiver, arguments, instance);
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
        let (id, ret) = self.functions().request_instance(function, instance, Half::Whole)?;
        Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
    }

    /// The arguments the generic `impl` of the method `function` takes from the type of `receiver`, concrete. `None`
    /// after a diagnostic.
    pub(in crate::elab) fn method_impl_arguments(&mut self, scope: &Scope, expr: ExprId, function: DeclId, receiver: ExprId) -> Option<Vec<ConcreteValue>> {
        let receiver_type = self.typed(receiver)?;
        let span = self.state.design.expr_node(expr).span;
        let Some(values) = self.state.types.impl_arguments(function, &receiver_type) else {
            self.refuse(scope.values.file, span, GenericsForm::MethodOfGenericImplWhoseParametersReceiver);
            return None;
        };
        values.iter().map(|value| self.functions().concrete_argument(value, scope.values.file, span)).collect()
    }

    /// The call `receiver.f(arguments)` through a bound (`Callee::Bound { item }`, the trait's own function): the
    /// receiver's type is concrete in the instance being lowered, so the call is the `impl`'s item for that type, or
    /// the trait's default body with `Self` bound to it (static dispatch, GN-04). `None` after a diagnostic.
    pub(super) fn lower_bound_call(&mut self, scope: &mut Scope, expr: ExprId, item: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        let receiver_type = self.typed(receiver)?;
        let Some(target) = self.bound_implementation(expr, item, &receiver_type) else {
            self.refuse(scope.values.file, span, GenericsForm::MethodCallThroughBoundThatNoImpl);
            return None;
        };
        self.lower_method_call(scope, expr, target, receiver, arguments, origin)
    }

    /// The item that runs for the call `expr` of the trait function `item` through a bound on a receiver of type `receiver`
    /// (`consts/dispatch.rs`).
    pub(in crate::elab) fn bound_implementation(&mut self, expr: ExprId, item: DeclId, receiver: &Ty) -> Option<DeclId> {
        self.state.consts.bound_implementation(expr, item, receiver)
    }

}
