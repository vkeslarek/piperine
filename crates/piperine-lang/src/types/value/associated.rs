//! Calls of an associated function written in an `impl` (`Logic::from(b)`, `Pair::new(a, b)`): which function a name
//! stands for when several `impl`s of `From` write it (the one whose source type the argument has, Appendix H.8
//! rule 8), and the arguments of a generic `impl` read from the call (`impl<const N: usize> From<UInt<N>> for
//! Logic<N>` meeting a `u4` gives `N = 4`), recorded as the instantiation of the `impl` the elaborator runs.

use crate::pom::{Arg, DeclId, DeclKind, ExprId, TypeId};

use super::ExpressionTyping;
use crate::types::generics::inference::Inference;
use crate::types::{Instantiation, Ty};

impl ExpressionTyping<'_, '_> {
    /// The generic `impl` (one with parameters of its own) `function` is written in, when `function` has none of its own.
    pub(super) fn generic_impl_of(&self, function: DeclId) -> Option<DeclId> {
        let owner = self.state.design.decl_data(function).parent?;
        let own_generics = self.state.design.decl_data(function).generics_span.is_some();
        (matches!(self.state.design.decl_data(owner).kind, DeclKind::Impl(_)) && self.state.design.decl_data(owner).generics_span.is_some() && !own_generics).then_some(owner)
    }

    /// The function a call of `function` means: `Logic::from` names the `from` of the first `impl From<_> for Logic`,
    /// and the call is of the one whose source type the argument has (H.8 rule 8). Any other function is itself.
    pub(super) fn overload_of(&mut self, function: DeclId, args: &[Arg]) -> DeclId {
        let Some(owner) = self.state.design.decl_data(function).parent else { return function };
        let table = std::rc::Rc::clone(&self.state.traits);
        let Some(entry) = table.impls().iter().find(|entry| entry.decl == owner && entry.builtin_trait == Some("From")) else { return function };
        let DeclKind::Impl(data) = &self.state.design.decl_data(owner).kind else { return function };
        let Some(self_ty) = self.lower_type_quietly(data.self_ty) else { return function };
        let candidates = table.builtin_functions("From", "from", &self_ty);
        if candidates.len() < 2 || entry.trait_arguments.is_empty() {
            return function;
        }
        let [only] = args else { return function };
        let Some(given) = self.infer(only.value) else { return function };
        candidates.into_iter().find(|candidate| self.accepts_first_parameter(*candidate, &given)).unwrap_or(function)
    }

    /// Whether the first parameter of the `impl` function `function` takes a value of type `given` once the `impl`'s
    /// parameters are read from it.
    fn accepts_first_parameter(&mut self, function: DeclId, given: &Ty) -> bool {
        let Some((params, _)) = self.declared_signature(function) else { return false };
        let Some(wanted) = params.first() else { return false };
        let Some(owner) = self.generic_impl_of(function) else { return given.fits(wanted) };
        let signature = self.generics().generic_signature(owner);
        let mut inference = Inference::new(&signature);
        inference.unify(wanted, given);
        given.fits(&inference.found().apply(wanted))
    }

    /// The type a written type is, without a diagnostic of its own: `None` when it has none.
    pub(in crate::types) fn lower_type_quietly(&mut self, ty: TypeId) -> Option<Ty> {
        self.state.result.written.get(&ty).cloned().or_else(|| self.lower_type(ty))
    }

    /// A call of `function`, written in the generic `impl` `owner`: the arguments of the `impl` are read from the types
    /// of the call's arguments and from the type the call is expected to have, an `impl` parameter with a default
    /// takes it, and the call is recorded as an instantiation of the `impl`.
    pub(super) fn associated_call(&mut self, expr: ExprId, callee: ExprId, function: DeclId, owner: DeclId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let (params, ret) = self.declared_signature(function)?;
        let signature = self.generics().generic_signature(owner);
        let mut inference = Inference::new(&signature);
        self.generics().learn_from_arguments(&mut inference, &params, args);
        if let Some(expected) = self.state.values.call_expected.take().filter(|(at, _)| *at == expr).map(|(_, ty)| ty) {
            inference.unify(&ret, &expected);
        }
        let substitution = self.generics().complete_arguments(&mut inference, &signature, owner, function, file, span)?;
        let arguments = signature.parameters().iter().filter_map(|parameter| substitution.get(parameter.decl).cloned()).collect();
        self.state.result.instantiations.insert(expr, Instantiation { item: owner, arguments });
        let params: Vec<Ty> = params.iter().map(|param| substitution.apply(param)).collect();
        let ret = substitution.apply(&ret);
        self.finish_call(expr, callee, function, params, ret, args, false)
    }
}
