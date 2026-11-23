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
use crate::types::{Bound, ImplTable, Ty};

impl Elaboration<'_, '_> {
    /// The call `receiver.f(arguments)` of the method `function`: the receiver and the arguments lowered
    /// in order, a `Call` of the method's variant (for a function of a trait, the instance for the type
    /// of the receiver). `None` after a diagnostic.
    pub(super) fn lower_method_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        // a method of a generic `impl` is instantiated with the arguments its `impl` takes from the receiver's type
        let impl_arguments = if self.is_generic_impl_item(function) {
            let receiver_type = self.typed(receiver)?;
            let Some(values) = self.types.impl_arguments(function, &receiver_type) else {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "a method of a generic impl whose parameters the receiver leaves open");
                return None;
            };
            values.iter().map(|value| self.concrete_argument(value, scope.file, self.design.expr_node(expr).span)).collect::<Option<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let mut lowered = vec![self.lower_expr(scope, receiver)?];
        for argument in arguments {
            lowered.push(self.lower_expr(scope, *argument)?);
        }
        // a trait's own function runs for the type of its receiver, which is `Self` there
        let instance = if self.is_trait_function(function) { vec![ConcreteValue::Type(self.typed(receiver)?)] } else { impl_arguments };
        let (id, ret) = self.request_instance(function, instance, Half::Whole)?;
        Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
    }

    /// The call `receiver.f(arguments)` through a bound (`Callee::Bound { item }`, the trait's own function): the
    /// receiver's type is concrete in the instance being lowered, so the call is the `impl`'s item for that type, or
    /// the trait's default body with `Self` bound to it (static dispatch, GN-04). `None` after a diagnostic.
    pub(super) fn lower_bound_call(&mut self, scope: &mut Scope, expr: ExprId, item: DeclId, receiver: ExprId, arguments: &[ExprId], origin: piperine_ir::Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        let receiver_type = self.typed(receiver)?;
        let table = &self.impls;
        let Some(target) = table.implementation(item, &receiver_type, &mut |ty, bound| Self::bound_holds(table, ty, bound)) else {
            self.unsupported(scope.file, span, "a method call through a bound that no impl gives for this type");
            return None;
        };
        self.lower_method_call(scope, expr, target, receiver, arguments, origin)
    }

    /// Whether `ty` satisfies `bound` for choosing among the `impl`s of the design: a trait bound is met by an `impl`
    /// (blanket ones included); the checker has judged every other bound before the instance exists.
    fn bound_holds(table: &ImplTable<'_>, ty: &Ty, bound: &Bound) -> bool {
        match bound {
            Bound::Trait { decl, .. } => table.implements(ty, *decl, &mut |candidate, inner| Self::bound_holds(table, candidate, inner)),
            _ => true,
        }
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
