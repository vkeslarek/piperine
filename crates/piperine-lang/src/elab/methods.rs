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

impl Elaboration<'_, '_> {
    /// The call `receiver.f(arguments)` of the method `function`: the receiver and the arguments lowered
    /// in order, a `Call` of the method's variant (for a function of a trait, the instance for the type
    /// of the receiver). `None` after a diagnostic.
    pub(super) fn lower_method_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        if self.is_generic_impl_item(function) {
            // the layout of a generic type has no arena instance until T38
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a method of a generic impl");
            return None;
        }
        let mut lowered = vec![self.lower_expr(scope, receiver)?];
        for argument in arguments {
            lowered.push(self.lower_expr(scope, *argument)?);
        }
        // a trait's own function runs for the type of its receiver, which is `Self` there
        let instance = if self.is_trait_function(function) { vec![ConcreteValue::Type(self.typed(receiver)?)] } else { Vec::new() };
        let (id, ret) = self.request_instance(function, instance, Half::Whole)?;
        Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
    }

    /// Whether `function` is declared in a trait.
    pub(super) fn is_trait_function(&self, function: DeclId) -> bool {
        self.design.decl_data(function).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Trait(_)))
    }

    /// Whether `function` is written in an `impl` that has generic parameters.
    fn is_generic_impl_item(&self, function: DeclId) -> bool {
        self.design.decl_data(function).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Impl(_)) && self.design.decl_data(parent).generics_span.is_some())
    }
}
