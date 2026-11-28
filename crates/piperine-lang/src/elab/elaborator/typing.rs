//! What the type table says of an expression, a binding or a written type, and of the instance being lowered: the one read of it (`ConstEval::typed`) and the IR types and widths that follow.

use std::collections::BTreeMap;
use piperine_ir::{ValueType};
use crate::elab::functions::instances::ConcreteValue;
use crate::elab::value::expressions::ValueTypes;
use crate::pom::{DeclId, DeclKind, ExprId};
use crate::types::{ConstTerm, GenericSignature, GenericValue, Length, Substitution, Ty};
use super::Elaboration;

impl<'d, 't> Elaboration<'d, 't> {
    /// The type the checker gave `expr`, with the generic parameters of the instance being lowered
    /// replaced by their arguments: the one read of the type table of an expression (`ConstEval::typed`).
    pub(in crate::elab) fn typed(&self, expr: ExprId) -> Option<Ty> {
        self.state.consts.typed(expr).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The type of the binding `binding` under the substitution.
    pub(in crate::elab) fn typed_local(&self, binding: DeclId) -> Option<Ty> {
        self.state.consts.typed_local(binding).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The type the type node `ty` writes, under the substitution.
    pub(in crate::elab) fn typed_written(&self, ty: crate::pom::TypeId) -> Option<Ty> {
        self.state.consts.typed_written(ty).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The IR type of a type of the subset; `None` when no value has it in the IR.
    pub(in crate::elab) fn value_type(&self, ty: &Ty) -> Option<ValueType> {
        ValueTypes { arena: &self.state.values.type_arena }.of(ty)
    }

    /// `ty` with each `[_; T::WIDTH]` that is still generic (the consts evaluator folds the built-in layouts) folded
    /// to the `WIDTH` of the hand-written `impl Repr` of the type `T` is given.
    pub(in crate::elab) fn with_hand_written_widths(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Array(element, Length::Generic(ConstTerm::Width(parameter))) => {
                let element = Box::new(self.with_hand_written_widths(element));
                let width = match self.state.consts.substitution().get(*parameter) {
                    Some(GenericValue::Type(given)) => self.level_count(given),
                    _ => None,
                };
                match width {
                    Some(levels) => Ty::Array(element, Length::Fixed(u64::from(levels))),
                    None => Ty::Array(element, Length::Generic(ConstTerm::Width(*parameter))),
                }
            }
            Ty::Array(element, length) => Ty::Array(Box::new(self.with_hand_written_widths(element)), length.clone()),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|item| self.with_hand_written_widths(item)).collect()),
            Ty::Option(inner) => Ty::Option(Box::new(self.with_hand_written_widths(inner))),
            other => other.clone(),
        }
    }

    /// The number of levels of the `Repr` of `ty` (§2): the width of its hand-written `impl`, else the count its layout
    /// gives (rules 2 to 6). `None` for a type with no laid-out value type.
    pub(in crate::elab) fn level_count(&self, ty: &Ty) -> Option<u32> {
        let hand_written = self.state.impls.hand_written_repr(ty).map(|entry| entry.decl).and_then(|owner| self.hand_written_width(owner, &self.hand_written_arguments(owner, ty)?));
        hand_written.or_else(|| self.state.types.repr_levels(ty))
    }

    /// The arguments the `impl Repr` `owner` takes for the type `ty` it is written for: what its parameters stand for
    /// (`impl<T: Repr> Repr for Pair<T>` meeting `Pair<u8>` gives `u8`), in the order of its parameters; empty for an
    /// `impl` with none. `None` when a parameter stays open.
    pub(in crate::elab) fn hand_written_arguments(&self, owner: DeclId, ty: &Ty) -> Option<Vec<ConcreteValue>> {
        let encode = self.state.impls.item(owner, "encode")?;
        if !self.is_generic_impl_item(encode) {
            return Some(Vec::new());
        }
        let found = self.state.types.impl_arguments(encode, ty)?;
        found
            .into_iter()
            .map(|value| match value {
                GenericValue::Type(argument) => Some(ConcreteValue::Type(argument)),
                GenericValue::Const(ConstTerm::Value(count)) => Some(ConcreteValue::Const(count)),
                _ => None,
            })
            .collect()
    }

    /// The folded `WIDTH` of the `impl Repr` `owner` for `arguments`: the level count of each type argument and the value
    /// of each const argument stand where the `impl`'s parameters are named (`2 * T::WIDTH`).
    pub(in crate::elab) fn hand_written_width(&self, owner: DeclId, arguments: &[ConcreteValue]) -> Option<u32> {
        let width = self.state.impls.item(owner, "WIDTH")?;
        let DeclKind::Const(data) = &self.state.design.decl_data(width).kind else { return None };
        let signature = GenericSignature::of(self.state.design, owner);
        let values: BTreeMap<DeclId, u128> = signature
            .parameters()
            .iter()
            .zip(arguments)
            .filter_map(|(parameter, argument)| match argument {
                ConcreteValue::Type(ty) => self.level_count(ty).map(|levels| (parameter.decl, u128::from(levels))),
                ConcreteValue::Const(count) => u128::try_from(*count).ok().map(|count| (parameter.decl, count)),
                _ => None,
            })
            .collect();
        u32::try_from(self.state.types.fold_with(data.value?, values)?).ok()
    }

    /// The function type a closure or a function name has: what the parameter bound by `Fn(A) -> R` is in the
    /// instance (D9: the instance takes the callable as a function value, called by `f(x)` like any local of
    /// function type). `None` for an argument that is no callable.
    pub(in crate::elab) fn callable_type(&self, value: &ConcreteValue) -> Option<Ty> {
        match value {
            ConcreteValue::Closure(closure) => self.typed(*closure),
            ConcreteValue::Function(function) => {
                let data = self.state.design.decl_data(*function);
                let DeclKind::Fn(signature) = &data.kind else { return None };
                let inputs: Option<Vec<Ty>> = data.children.iter().copied().filter(|child| matches!(self.state.design.decl_data(*child).kind, DeclKind::FnParam(_))).map(|param| self.typed_local(param)).collect();
                let output = signature.ret.and_then(|ret| self.typed_written(ret)).unwrap_or(Ty::Unit);
                Some(Ty::Fn(inputs?, Box::new(output)))
            }
            _ => None,
        }
    }

    /// What the generic parameters of `function` are replaced by in the body of the instance with `arguments`: a
    /// type for a type parameter and a value for a const one; a closure or a function has no type to give yet (T37).
    pub(in crate::elab) fn instance_substitution(&self, function: DeclId, arguments: &[ConcreteValue]) -> Substitution {
        let mut substitution = Substitution::new();
        // a method of a generic `impl` takes the parameters of the `impl`
        let owner = self.state.design.decl_data(function).parent.filter(|owner| matches!(self.state.design.decl_data(*owner).kind, DeclKind::Impl(_)) && self.state.design.decl_data(*owner).generics_span.is_some()).unwrap_or(function);
        let signature = GenericSignature::of(self.state.design, owner);
        // a function a trait declares runs for one type, which is `Self`: its one argument
        if let (true, Some(ConcreteValue::Type(ty))) = (self.is_trait_function(function), arguments.first())
            && let Some(owner) = self.state.design.decl_data(function).parent
        {
            substitution.bind(owner, GenericValue::Type(ty.clone()));
        }
        for (parameter, argument) in signature.parameters().iter().zip(arguments) {
            match argument {
                ConcreteValue::Type(ty) => substitution.bind(parameter.decl, GenericValue::Type(ty.clone())),
                ConcreteValue::Const(value) => substitution.bind(parameter.decl, GenericValue::Const(ConstTerm::Value(*value))),
                ConcreteValue::Bool(flag) => substitution.bind(parameter.decl, GenericValue::Const(ConstTerm::Bool(*flag))),
                // a closure or a function stands for its function type: the parameter that takes it is a function value
                ConcreteValue::Function(_) | ConcreteValue::Closure(_) => {
                    if let Some(ty) = self.callable_type(argument) {
                        substitution.bind(parameter.decl, GenericValue::Type(ty));
                    }
                }
            }
        }
        substitution
    }
}
