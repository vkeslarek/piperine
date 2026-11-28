//! An instantiation of a generic function made concrete (generics GN-01, GN-02, GN-07, part I
//! §3.8): the arguments the checker recorded for a call, folded under the substitution of the
//! instance the call sits in, the obligations the definition deferred discharged for them, and the
//! instance named in the turbofish spelling of D8 (`clamp::<u8>`). A false `where` is `E0127`, two
//! array lengths that differ are `E0180`, and a const expression that does not fold is `E0104`, each
//! naming the item and its arguments.

use std::collections::BTreeMap;

use piperine_ir::Constant;

use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::refusal::forms::StructureForm;
use crate::pom::{DeclId, DeclKind, ExprId, FileId, GenericParamData};
use crate::types::{ConstTerm, GenericSignature, GenericValue, Length, Obligation, Substitution, Ty};
use crate::msl::{Diagnostic, Span};

/// One argument of an instance, with nothing left to fold.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ConcreteValue {
    /// A type, with no parameter in it.
    Type(Ty),
    /// An integer const argument.
    Const(i128),
    /// A `bool` const argument.
    Bool(bool),
    /// A function named for an `Fn` bound.
    Function(DeclId),
    /// A closure written for an `Fn` bound.
    Closure(ExprId),
}

/// A generic function and the concrete argument of each of its parameters: what one IR function is
/// made for (§3.8.10).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Instance {
    /// The generic function.
    pub(super) item: DeclId,
    /// The arguments, in the order of its parameters.
    pub(super) arguments: Vec<ConcreteValue>,
}

impl Instance {
    /// The name of the instance in the turbofish spelling, `clamp::<u8>` (D8).
    pub(super) fn name(&self, elaboration: &Elaboration<'_, '_>) -> String {
        let item = elaboration.impl_qualified_name(self.item);
        format!("{item}::<{}>", self.argument_names(elaboration).join(", "))
    }

    /// The name of the generic item without its arguments, `clamp`.
    pub(super) fn item_name(&self, elaboration: &Elaboration<'_, '_>) -> String {
        elaboration.impl_qualified_name(self.item)
    }

    /// Each argument as the source spells it: a type as written, a number, a function's name, `closure$k`.
    pub(super) fn argument_names(&self, elaboration: &Elaboration<'_, '_>) -> Vec<String> {
        let design = elaboration.state.design;
        self.arguments
            .iter()
            .map(|argument| match argument {
                ConcreteValue::Type(ty) => ty.source_name(design),
                ConcreteValue::Const(value) => value.to_string(),
                ConcreteValue::Bool(value) => value.to_string(),
                ConcreteValue::Function(function) => design.decl_data(*function).name.as_ref().map_or_else(|| "fn".to_string(), |n| n.text.clone()),
                ConcreteValue::Closure(closure) => format!("closure${}${}", elaboration.closure_owner_name(*closure), elaboration.closure_ordinal(*closure)),
            })
            .collect()
    }
}

impl Elaboration<'_, '_> {
    /// What the generic parameters of `function` are replaced by in the body of the instance with `arguments`: a
    /// type for a type parameter and a value for a const one; a closure or a function has no type to give yet (T37).
    pub(super) fn instance_substitution(&self, function: DeclId, arguments: &[ConcreteValue]) -> Substitution {
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

    /// The function type a closure or a function name has: what the parameter bound by `Fn(A) -> R` is in the
    /// instance (D9: the instance takes the callable as a function value, called by `f(x)` like any local of
    /// function type). `None` for an argument that is no callable.
    fn callable_type(&self, value: &ConcreteValue) -> Option<Ty> {
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

    /// The instance the call `call` makes, from the instantiation the checker recorded folded under the
    /// substitution of the instance being lowered, with its obligations discharged. `None` after a diagnostic.
    pub(super) fn concrete_instance(&mut self, call: ExprId) -> Option<Instance> {
        let recorded = self.state.types.instantiation(call)?.clone();
        let (file, span) = (self.state.design.decl_data(self.state.design.expr_node(call).owner).file, self.state.design.expr_node(call).span);
        let mut arguments = Vec::new();
        for value in &recorded.arguments {
            arguments.push(self.fold_argument(value, file, span)?);
        }
        let instance = Instance { item: recorded.item, arguments };
        let bindings = self.instance_bindings(&instance);
        self.discharge(&instance, &bindings)?;
        Some(instance)
    }

    /// The argument `value` with the enclosing instance's substitution applied and every const folded.
    pub(super) fn concrete_argument(&mut self, value: &GenericValue, file: FileId, span: Span) -> Option<ConcreteValue> {
        self.fold_argument(value, file, span)
    }

    /// The argument `value`, folded under the substitution.
    fn fold_argument(&mut self, value: &GenericValue, file: FileId, span: Span) -> Option<ConcreteValue> {
        match value {
            GenericValue::Type(ty) => {
                let ty = self.state.consts.substitution().apply(ty);
                if ty.has_param() {
                    self.refuse(file, span, StructureForm::GenericArgumentThatIsStillParameter);
                    return None;
                }
                Some(ConcreteValue::Type(ty))
            }
            GenericValue::Const(term) => self.fold_term(term, file, span),
            GenericValue::Function(function) => Some(ConcreteValue::Function(*function)),
            GenericValue::Closure(closure) => Some(ConcreteValue::Closure(*closure)),
        }
    }

    /// A const argument as a value: a number, a `bool`, a const parameter of the enclosing instance or an
    /// expression over them.
    fn fold_term(&mut self, term: &ConstTerm, file: FileId, span: Span) -> Option<ConcreteValue> {
        match self.state.consts.substitution().apply_term(term) {
            ConstTerm::Value(value) => Some(ConcreteValue::Const(value)),
            ConstTerm::Bool(value) => Some(ConcreteValue::Bool(value)),
            // an expression over const parameters folds the way the checker folds a length: syntactically, over their values
            ConstTerm::Expr(expr) if let Some(value) = self.state.types.fold_with(expr, self.enclosing_values()).and_then(|value| i128::try_from(value).ok()) => Some(ConcreteValue::Const(value)),
            ConstTerm::Expr(expr) => {
                let bindings = self.enclosing_bindings();
                match self.state.consts.value(expr, &bindings) {
                    Ok(Constant::Int(value)) => Some(ConcreteValue::Const(value)),
                    Ok(Constant::Bool(value)) => Some(ConcreteValue::Bool(value)),
                    Ok(_) => {
                        self.refuse(file, span, StructureForm::ConstArgumentThatIsNotIntegerOr);
                        None
                    }
                    Err(fault) => {
                        self.const_fault(&fault, "a generic argument");
                        None
                    }
                }
            }
            ConstTerm::Param(_) | ConstTerm::Width(_) => {
                self.refuse(file, span, StructureForm::ConstArgumentThatIsStillParameter);
                None
            }
        }
    }

    /// The integer const arguments of the instance being lowered by parameter: what a syntactic fold of an expression reads.
    fn enclosing_values(&self) -> BTreeMap<DeclId, u128> {
        let generic = self.state.consts.substitution().entries().filter_map(|(parameter, value)| match value {
            GenericValue::Const(ConstTerm::Value(number)) => u128::try_from(*number).ok().map(|number| (parameter, number)),
            _ => None,
        });
        // the key params of the body being lowered: an argument `{ count }` reads the value the body was lowered for
        let keys = self.state.structure.body_keys.integers().into_iter().filter_map(|(param, number)| u128::try_from(number).ok().map(|number| (param, number)));
        generic.chain(keys).collect()
    }

    /// The const parameters of the instance being lowered bound to their values.
    fn enclosing_bindings(&self) -> Bindings {
        let mut bindings = Bindings::new();
        for (parameter, value) in self.state.consts.substitution().entries() {
            match value {
                GenericValue::Const(ConstTerm::Value(number)) => bindings.bind(parameter, Constant::Int(*number)),
                GenericValue::Const(ConstTerm::Bool(flag)) => bindings.bind(parameter, Constant::Bool(*flag)),
                _ => {}
            }
        }
        bindings
    }

    /// The integer const arguments of `instance` by parameter, and the level count of each type argument: what an array
    /// length folds over.
    fn instance_values(&self, instance: &Instance) -> BTreeMap<DeclId, u128> {
        let signature = GenericSignature::of(self.state.design, instance.item);
        signature
            .parameters()
            .iter()
            .zip(&instance.arguments)
            .filter_map(|(parameter, argument)| match argument {
                ConcreteValue::Const(value) => u128::try_from(*value).ok().map(|value| (parameter.decl, value)),
                // a type argument stands for its level count, which `T::WIDTH` in a length reads (§2)
                ConcreteValue::Type(ty) => self.level_count(ty).map(|levels| (parameter.decl, u128::from(levels))),
                _ => None,
            })
            .collect()
    }

    /// The const parameters of `instance`'s function bound to its const arguments.
    pub(super) fn instance_bindings(&self, instance: &Instance) -> Bindings {
        let mut bindings = Bindings::new();
        let signature = GenericSignature::of(self.state.design, instance.item);
        for (parameter, argument) in signature.parameters().iter().zip(&instance.arguments) {
            if !matches!(self.state.design.decl_data(parameter.decl).kind, DeclKind::GenericParam(GenericParamData::Const { .. })) {
                continue;
            }
            match argument {
                ConcreteValue::Const(value) => bindings.bind(parameter.decl, Constant::Int(*value)),
                ConcreteValue::Bool(flag) => bindings.bind(parameter.decl, Constant::Bool(*flag)),
                _ => {}
            }
        }
        bindings
    }

    /// Discharges what the definition of `instance`'s function deferred (Q1 (c)): the `where`, the pairs of
    /// array lengths and the const expressions, for these arguments. `None` after a diagnostic.
    pub(super) fn discharge(&mut self, instance: &Instance, bindings: &Bindings) -> Option<()> {
        // the `where` reads the instance's own type arguments (`A::WIDTH == B::WIDTH`)
        let outer = self.state.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
        let discharged = self.discharge_under_substitution(instance, bindings);
        self.state.consts.set_substitution(outer);
        discharged
    }

    /// [`Elaboration::discharge`] with the substitution of `instance` set.
    fn discharge_under_substitution(&mut self, instance: &Instance, bindings: &Bindings) -> Option<()> {
        let name = instance.name(self);
        let values = self.instance_values(instance);
        let mut holds = true;
        for obligation in self.state.types.obligations(instance.item).to_vec() {
            match obligation {
                Obligation::Where { item, condition } => match self.state.consts.value(condition, bindings) {
                    Ok(Constant::Bool(true)) => {}
                    Ok(Constant::Bool(false)) => {
                        let data = self.state.design.decl_data(item);
                        let span = self.state.design.expr_node(condition).span;
                        self.report(data.file, Diagnostic::new("E0127", format!("the `where` of `{name}` is false for these arguments (§3 rule 8)"), span, "a `where` that is false"));
                        holds = false;
                    }
                    Ok(_) => {}
                    Err(fault) => {
                        self.const_fault(&fault, &format!("the `where` of `{name}`"));
                        holds = false;
                    }
                },
                Obligation::SameLength { file, span, expected, found } => {
                    let (wanted, given) = (self.fold_length(instance, &expected, &values), self.fold_length(instance, &found, &values));
                    if let (Some(wanted), Some(given)) = (wanted, given)
                        && wanted != given
                    {
                        self.report(file, Diagnostic::new("E0180", format!("mismatched types in `{name}`: an array of {wanted} elements is expected, found one of {given}"), span, "array lengths that differ"));
                        holds = false;
                    }
                }
                Obligation::Constant { expr, file, span } => {
                    if self.state.types.fold_with(expr, values.clone()).is_none() {
                        self.report(file, Diagnostic::new("E0104", format!("a const expression of `{name}` does not fold: it underflows, overflows or divides by zero for these arguments"), span, "panics"));
                        holds = false;
                    }
                }
            }
        }
        holds.then_some(())
    }

    /// The number of elements `length` stands for when the const parameters have `values`, `None` when it does not fold.
    fn fold_length(&mut self, instance: &Instance, length: &Length, values: &BTreeMap<DeclId, u128>) -> Option<i128> {
        match length {
            Length::Fixed(count) => Some(i128::from(*count)),
            Length::Generic(ConstTerm::Value(count)) => Some(*count),
            Length::Generic(ConstTerm::Expr(expr)) => self.state.types.fold_with(*expr, values.clone()).and_then(|count| i128::try_from(count).ok()),
            Length::Generic(ConstTerm::Param(parameter)) => values.get(parameter).and_then(|count| i128::try_from(*count).ok()),
            // `T::WIDTH` of the argument the instance gives the parameter `T` (§2)
            Length::Generic(ConstTerm::Width(parameter)) => {
                let signature = GenericSignature::of(self.state.design, instance.item);
                let argument = signature.parameters().iter().zip(&instance.arguments).find_map(|(candidate, argument)| (candidate.decl == *parameter).then_some(argument))?;
                match argument {
                    ConcreteValue::Type(given) => self.level_count(given).map(i128::from),
                    _ => None,
                }
            }
            Length::Generic(_) => None,
        }
    }

    /// The name of the item a closure is written in: two items each have a `closure$0`, so the instance names the item too.
    pub(super) fn closure_owner_name(&self, closure: ExprId) -> String {
        let owner = self.state.design.expr_node(closure).owner;
        self.state.design.decl_data(owner).name.as_ref().map_or_else(String::new, |name| name.text.clone())
    }

    /// The position of the closure `closure` among the closures of the item it is written in, in source order.
    pub(super) fn closure_ordinal(&self, closure: ExprId) -> usize {
        let owner = self.state.design.expr_node(closure).owner;
        (0..closure.0).filter(|index| {
            let node = self.state.design.expr_node(ExprId(*index));
            node.owner == owner && matches!(node.kind, crate::pom::ExprKind::Closure { .. })
        }).count()
    }
}
