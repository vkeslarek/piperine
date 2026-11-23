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
use crate::pom::{DeclId, DeclKind, ExprId, FileId, GenericParamData};
use crate::types::{ConstTerm, GenericSignature, GenericValue, Length, Obligation, Ty};
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
        let design = elaboration.design;
        let item = design.decl_data(self.item).name.as_ref().map_or("fn", |n| n.text.as_str());
        let arguments: Vec<String> = self
            .arguments
            .iter()
            .map(|argument| match argument {
                ConcreteValue::Type(ty) => ty.source_name(design),
                ConcreteValue::Const(value) => value.to_string(),
                ConcreteValue::Bool(value) => value.to_string(),
                ConcreteValue::Function(function) => design.decl_data(*function).name.as_ref().map_or_else(|| "fn".to_string(), |n| n.text.clone()),
                ConcreteValue::Closure(closure) => format!("closure${}", elaboration.closure_ordinal(*closure)),
            })
            .collect();
        format!("{item}::<{}>", arguments.join(", "))
    }
}

impl Elaboration<'_, '_> {
    /// The instance the call `call` makes, from the instantiation the checker recorded folded under the
    /// substitution of the instance being lowered, with its obligations discharged. `None` after a diagnostic.
    pub(super) fn concrete_instance(&mut self, call: ExprId) -> Option<Instance> {
        let recorded = self.types.instantiation(call)?.clone();
        let (file, span) = (self.design.decl_data(self.design.expr_node(call).owner).file, self.design.expr_node(call).span);
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
    fn fold_argument(&mut self, value: &GenericValue, file: FileId, span: Span) -> Option<ConcreteValue> {
        match value {
            GenericValue::Type(ty) => {
                let ty = self.consts.substitution().apply(ty);
                if ty.has_param() {
                    self.unsupported(file, span, "a generic argument that is still a parameter");
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
        match self.consts.substitution().apply_term(term) {
            ConstTerm::Value(value) => Some(ConcreteValue::Const(value)),
            ConstTerm::Bool(value) => Some(ConcreteValue::Bool(value)),
            ConstTerm::Expr(expr) => {
                let bindings = self.enclosing_bindings();
                match self.consts.value(expr, &bindings) {
                    Ok(Constant::Int(value)) => Some(ConcreteValue::Const(value)),
                    Ok(Constant::Bool(value)) => Some(ConcreteValue::Bool(value)),
                    Ok(_) => {
                        self.unsupported(file, span, "a const argument that is not an integer or a `bool`");
                        None
                    }
                    Err(fault) => {
                        self.const_fault(&fault, "a generic argument");
                        None
                    }
                }
            }
            ConstTerm::Param(_) | ConstTerm::Width(_) => {
                self.unsupported(file, span, "a const argument that is still a parameter");
                None
            }
        }
    }

    /// The const parameters of the instance being lowered bound to their values.
    fn enclosing_bindings(&self) -> Bindings {
        let mut bindings = Bindings::new();
        for (parameter, value) in self.consts.substitution().entries() {
            match value {
                GenericValue::Const(ConstTerm::Value(number)) => bindings.bind(parameter, Constant::Int(*number)),
                GenericValue::Const(ConstTerm::Bool(flag)) => bindings.bind(parameter, Constant::Bool(*flag)),
                _ => {}
            }
        }
        bindings
    }

    /// The integer const arguments of `instance` by parameter: what an array length folds over.
    fn instance_values(&self, instance: &Instance) -> BTreeMap<DeclId, u128> {
        let signature = GenericSignature::of(self.design, instance.item);
        signature
            .parameters()
            .iter()
            .zip(&instance.arguments)
            .filter_map(|(parameter, argument)| match argument {
                ConcreteValue::Const(value) => u128::try_from(*value).ok().map(|value| (parameter.decl, value)),
                _ => None,
            })
            .collect()
    }

    /// The const parameters of `instance`'s function bound to its const arguments.
    fn instance_bindings(&self, instance: &Instance) -> Bindings {
        let mut bindings = Bindings::new();
        let signature = GenericSignature::of(self.design, instance.item);
        for (parameter, argument) in signature.parameters().iter().zip(&instance.arguments) {
            if !matches!(self.design.decl_data(parameter.decl).kind, DeclKind::GenericParam(GenericParamData::Const { .. })) {
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
    fn discharge(&mut self, instance: &Instance, bindings: &Bindings) -> Option<()> {
        let name = instance.name(self);
        let values = self.instance_values(instance);
        let mut holds = true;
        for obligation in self.types.obligations(instance.item).to_vec() {
            match obligation {
                Obligation::Where { item, condition } => match self.consts.value(condition, bindings) {
                    Ok(Constant::Bool(true)) => {}
                    Ok(Constant::Bool(false)) => {
                        let data = self.design.decl_data(item);
                        let span = self.design.expr_node(condition).span;
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
                    let (wanted, given) = (self.fold_length(&expected, &values), self.fold_length(&found, &values));
                    if let (Some(wanted), Some(given)) = (wanted, given)
                        && wanted != given
                    {
                        self.report(file, Diagnostic::new("E0180", format!("mismatched types in `{name}`: an array of {wanted} elements is expected, found one of {given}"), span, "array lengths that differ"));
                        holds = false;
                    }
                }
                Obligation::Constant { expr, file, span } => {
                    if self.types.fold_with(expr, values.clone()).is_none() {
                        self.report(file, Diagnostic::new("E0104", format!("a const expression of `{name}` does not fold: it underflows, overflows or divides by zero for these arguments"), span, "panics"));
                        holds = false;
                    }
                }
            }
        }
        holds.then_some(())
    }

    /// The number of elements `length` stands for when the const parameters have `values`, `None` when it does not fold.
    fn fold_length(&mut self, length: &Length, values: &BTreeMap<DeclId, u128>) -> Option<i128> {
        match length {
            Length::Fixed(count) => Some(i128::from(*count)),
            Length::Generic(ConstTerm::Value(count)) => Some(*count),
            Length::Generic(ConstTerm::Expr(expr)) => self.types.fold_with(*expr, values.clone()).and_then(|count| i128::try_from(count).ok()),
            Length::Generic(ConstTerm::Param(parameter)) => values.get(parameter).and_then(|count| i128::try_from(*count).ok()),
            Length::Generic(_) => None,
        }
    }

    /// The position of the closure `closure` among the closures of the item it is written in, in source order.
    pub(super) fn closure_ordinal(&self, closure: ExprId) -> usize {
        let owner = self.design.expr_node(closure).owner;
        (0..closure.0).filter(|index| {
            let node = self.design.expr_node(ExprId(*index));
            node.owner == owner && matches!(node.kind, crate::pom::ExprKind::Closure { .. })
        }).count()
    }
}
