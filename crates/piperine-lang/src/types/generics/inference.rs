//! Inference of the generic arguments of a call (part I §3.8.5, §3.8.7, §1.4.4, GN-01): the
//! arguments written with `::<..>` first, then unification of each parameter's type with the type
//! of its argument and of the result with the type the call is expected to have, a const argument
//! from the length of an array argument, then the defaults. A parameter left open is `E0181`
//! naming it. The call is then typed by the signature with the arguments substituted.

use crate::msl::Span;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode, Resolution, TypeKind};

use super::GenericsSolver;
use super::generics::{Bound, GenericParameter, GenericSignature, ParameterKind};
use crate::types::{ConstTerm, GenericValue, Instantiation, Length, Substitution, Ty};

/// What a call of a generic function has learnt about its parameters so far.
pub(in crate::types) struct Inference<'s> {
    /// The parameters of the function, in order.
    signature: &'s GenericSignature,
    /// The arguments found, by parameter.
    found: Substitution,
}

impl<'s> Inference<'s> {
    /// An inference with nothing known.
    pub(in crate::types) fn new(signature: &'s GenericSignature) -> Self {
        Inference { signature, found: Substitution::new() }
    }

    /// Records the argument written for the parameter at `index`.
    pub(super) fn give(&mut self, index: usize, value: GenericValue) {
        if let Some(parameter) = self.signature.parameters().get(index) {
            self.found.bind(parameter.decl, value);
        }
    }

    /// The parameters nothing has given an argument yet.
    pub(super) fn open(&self) -> Vec<&'s GenericParameter> {
        self.signature.parameters().iter().filter(|parameter| self.found.get(parameter.decl).is_none()).collect()
    }

    /// What is known.
    pub(in crate::types) fn found(&self) -> &Substitution {
        &self.found
    }

    /// Learns from a parameter of type `declared` meeting a value of type `given`: an open type
    /// parameter takes `given`, and the two are taken apart together, an element with an element, a
    /// generic argument with a generic argument, an array length with an array length.
    pub(in crate::types) fn unify(&mut self, declared: &Ty, given: &Ty) {
        match (declared, given) {
            (Ty::Param(parameter), _) if self.owns(*parameter) => {
                if self.found.get(*parameter).is_none() {
                    self.found.bind(*parameter, GenericValue::Type(given.clone()));
                }
            }
            (Ty::Array(declared, declared_length), Ty::Array(given, given_length)) => {
                self.unify(declared, given);
                self.unify_length(declared_length, given_length);
            }
            (Ty::Option(declared), Ty::Option(given)) => self.unify(declared, given),
            // a carrier meeting a carrier: what each holds (`Bit<T>` meeting `Bit<u8>`)
            (Ty::Carrier { nature, value: declared }, Ty::Carrier { nature: given_nature, value: given }) if self.owns(*nature) => {
                // the carrier of a nature parameter: the parameter takes the nature given, and what it holds is unified too
                if self.found.get(*nature).is_none() {
                    self.found.bind(*nature, GenericValue::Type(Ty::Node(*given_nature)));
                }
                self.unify(declared, given);
            }
            (Ty::Carrier { value: declared, .. }, Ty::Carrier { value: given, .. }) => self.unify(declared, given),
            // `UInt<N>` meeting `u4` gives `N = 4`; meeting `UInt<M>` gives `N = M`
            (Ty::GenericInteger { signed, width: ConstTerm::Param(parameter) }, found) if self.owns(*parameter) && self.found.get(*parameter).is_none() => {
                let width = match found {
                    Ty::UInt(n) if !signed => Some(ConstTerm::Value(i128::from(*n))),
                    Ty::Int(n) if *signed => Some(ConstTerm::Value(i128::from(*n))),
                    Ty::GenericInteger { signed: other, width } if other == signed => Some(width.clone()),
                    _ => None,
                };
                if let Some(width) = width {
                    self.found.bind(*parameter, GenericValue::Const(width));
                }
            }
            (Ty::Tuple(declared), Ty::Tuple(given)) if declared.len() == given.len() => declared.iter().zip(given).for_each(|(declared, given)| self.unify(declared, given)),
            (Ty::Fn(declared, declared_result), Ty::Fn(given, given_result)) if declared.len() == given.len() => {
                declared.iter().zip(given).for_each(|(declared, given)| self.unify(declared, given));
                self.unify(declared_result, given_result);
            }
            (Ty::Range { element: declared, .. }, Ty::Range { element: given, .. }) | (Ty::Iterator { element: declared, .. }, Ty::Iterator { element: given, .. }) => self.unify(declared, given),
            (Ty::Struct(declared), Ty::Struct(given)) | (Ty::Enum(declared), Ty::Enum(given)) if declared.decl == given.decl && declared.arguments.len() == given.arguments.len() => {
                for (declared, given) in declared.arguments.iter().zip(&given.arguments) {
                    if let (GenericValue::Type(declared), GenericValue::Type(given)) = (declared, given) {
                        self.unify(declared, given);
                    } else if let (GenericValue::Const(ConstTerm::Param(parameter)), GenericValue::Const(term)) = (declared, given)
                        && self.owns(*parameter)
                        && self.found.get(*parameter).is_none()
                    {
                        self.found.bind(*parameter, GenericValue::Const(term.clone()));
                    }
                }
            }
            _ => {}
        }
    }

    /// A const parameter that is the whole length of an array takes the length of the array it meets.
    fn unify_length(&mut self, declared: &Length, given: &Length) {
        let Length::Generic(ConstTerm::Param(parameter)) = declared else { return };
        if !self.owns(*parameter) || self.found.get(*parameter).is_some() {
            return;
        }
        let term = match given {
            Length::Fixed(n) => ConstTerm::Value(i128::from(*n)),
            Length::Generic(term) => term.clone(),
        };
        self.found.bind(*parameter, GenericValue::Const(term));
    }

    /// Whether `parameter` is one of the function's own parameters.
    fn owns(&self, parameter: DeclId) -> bool {
        self.signature.parameters().iter().any(|own| own.decl == parameter)
    }
}

impl GenericsSolver<'_, '_> {
    /// A call of the generic function `function`: its generic arguments found, its signature
    /// substituted, its arguments bound and checked against the substituted parameters. `None` when
    /// an argument cannot be found (`E0181`) or one is refused.
    pub(in crate::types) fn generic_call(&mut self, expr: ExprId, callee: ExprId, function: DeclId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let signature = self.generic_signature(function);
        let (params, ret) = self.expressions().declared_signature(function)?;
        let mut inference = Inference::new(&signature);
        self.explicit_arguments(&mut inference, &signature, callee, function, file, span)?;
        self.argument_types(&mut inference, &params, args);
        self.callable_arguments(&mut inference, &params, args);
        if let Some(expected) = self.state.values.take_expected_call_type(expr) {
            inference.unify(&ret, &expected);
        }
        let substitution = self.complete_arguments(&mut inference, &signature, function, function, file, span)?;
        let arguments: Vec<GenericValue> = signature.parameters().iter().filter_map(|parameter| self.recorded_argument(parameter, &params, args, &substitution)).collect();
        self.arguments_satisfy_bounds(&signature, &arguments, function, file, span)?;
        self.state.result.instantiations.insert(expr, Instantiation { item: function, arguments });
        let params: Vec<Ty> = params.iter().map(|param| self.substitute(&substitution, param)).collect();
        let ret = self.substitute(&substitution, &ret);
        self.state.result.exprs[callee.index()] = Some(Ty::Fn(params.clone(), Box::new(ret.clone())));
        self.expressions().finish_call(expr, callee, function, params, ret, args, false)
    }

    /// The argument the instantiation records for `parameter`: the closure or the function a call
    /// passes for a parameter bound by `Fn(A) -> R` (D9, §3.8.6), else what the substitution gives.
    fn recorded_argument(&mut self, parameter: &GenericParameter, declared: &[Ty], args: &[Arg], substitution: &Substitution) -> Option<GenericValue> {
        let callable = parameter.bounds.iter().any(|bound| matches!(bound, Bound::Callable { .. }));
        let passed = declared.iter().position(|ty| *ty == Ty::Param(parameter.decl)).and_then(|index| args.get(index));
        if let (true, Some(arg)) = (callable, passed) {
            if let Some(closure) = self.expressions().closure_expr(arg.value) {
                return Some(GenericValue::Closure(closure));
            }
            if let Some(Resolution::Decl(decl)) = self.expressions().callee_resolution(arg.value) {
                if matches!(self.state.design.decl_data(decl).kind, DeclKind::Fn(_)) {
                    return Some(GenericValue::Function(decl));
                }
                if let Some(closure) = self.state.values.closure_of(decl) {
                    return Some(GenericValue::Closure(closure));
                }
            }
        }
        substitution.get(parameter.decl).cloned()
    }

    /// The arguments written with `::<..>` after the function's name, in the order of its
    /// parameters (§3.8.5); more than the list takes is `E0187`. `None` after a refusal.
    pub(super) fn explicit_arguments(&mut self, inference: &mut Inference<'_>, signature: &GenericSignature, callee: ExprId, function: DeclId, file: FileId, span: Span) -> Option<()> {
        // a template written in place (`G::<2> { k: 3.0 }(x)`, §8.4.3) names its arguments on the path of its head
        let (ExprKind::Path(path) | ExprKind::Struct { path, .. }) = &self.state.design.expr_node(callee).kind else { return Some(()) };
        let nodes = match self.state.design.path_node(*path).segments.last().and_then(|segment| segment.args.clone()) {
            Some(GenericArgsNode::Angle { args, .. }) => args,
            _ => return Some(()),
        };
        if nodes.len() > signature.parameters().len() {
            let name = self.state.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let message = format!("`{name}` takes at most {} generic arguments, found {} (§3.8.7)", signature.parameters().len(), nodes.len());
            self.coded(file, "E0187", span, message, "the wrong number of generic arguments");
            return None;
        }
        for (index, node) in nodes.iter().enumerate() {
            // `_` is as if left out (§1.2.12): the connections give the argument, else its default
            if matches!(node, GenericArgNode::Type(ty) if matches!(self.state.design.type_node(*ty).kind, TypeKind::Infer)) {
                continue;
            }
            let value = self.generic_argument(&signature.parameters()[index], node, file)?;
            inference.give(index, value);
        }
        Some(())
    }

    /// Learns from the arguments: each argument whose type does not depend on its context first, in
    /// order, then the integer literals with no suffix, which take the type already found (`larger(3u8,
    /// 200)`) or else their default (§1.4.2).
    pub(in crate::types) fn learn_from_arguments(&mut self, inference: &mut Inference<'_>, params: &[Ty], args: &[Arg]) {
        self.argument_types(inference, params, args);
    }

    pub(super) fn argument_types(&mut self, inference: &mut Inference<'_>, params: &[Ty], args: &[Arg]) {
        for open_literals in [false, true] {
            for (arg, declared) in args.iter().zip(params) {
                if self.expressions().integer_literal_text(arg.value).is_some() != open_literals || self.callable_bound_of(declared).is_some() {
                    continue;
                }
                if open_literals && !self.mentions_open_parameter(declared, inference) {
                    continue;
                }
                let given = if self.modules().is_nature_parameter(declared) { self.modules().infer_carrier(arg.value) } else { self.expressions().infer(arg.value) };
                if let Some(given) = given {
                    inference.unify(declared, &given);
                }
            }
        }
    }

    /// Types the arguments passed for parameters bounded by `Fn(A) -> R` last, against the function
    /// type the bound writes with what is known substituted (§3.8.6): a closure with parameters of
    /// no written type takes them from it, a `fn` name or a typed closure gives its own, and what
    /// it gives is learnt in turn (`F: Fn(T) -> U` finds `U` from the closure's result).
    fn callable_arguments(&mut self, inference: &mut Inference<'_>, params: &[Ty], args: &[Arg]) {
        for (arg, declared) in args.iter().zip(params) {
            let Some(wanted) = self.callable_bound_of(declared) else { continue };
            let expected = inference.found().apply(&wanted);
            let ty = if self.expressions().is_function_value(arg.value) && !self.mentions_open_parameter(&expected, inference) {
                self.expressions().check(arg.value, &expected);
                self.state.result.exprs[arg.value.index()].clone()
            } else {
                self.expressions().infer(arg.value)
            };
            if let Some(ty) = ty {
                inference.unify(declared, &ty);
                inference.unify(&wanted, &ty);
            }
        }
    }

    /// The function type `declared` is bound to when it is a type parameter bounded by `Fn(A) -> R`.
    fn callable_bound_of(&mut self, declared: &Ty) -> Option<Ty> {
        let Ty::Param(parameter) = declared else { return None };
        self.callable_of(*parameter)
    }

    /// Whether `declared` is, or holds, a parameter the inference has not found yet.
    fn mentions_open_parameter(&self, declared: &Ty, inference: &Inference<'_>) -> bool {
        match declared {
            Ty::Param(parameter) => inference.open().iter().any(|open| open.decl == *parameter),
            Ty::GenericInteger { width: ConstTerm::Param(parameter), .. } => inference.open().iter().any(|open| open.decl == *parameter),
            Ty::Struct(nominal) | Ty::Enum(nominal) => nominal.arguments.iter().any(|argument| match argument {
                GenericValue::Type(ty) => self.mentions_open_parameter(ty, inference),
                GenericValue::Const(ConstTerm::Param(parameter)) => inference.open().iter().any(|open| open.decl == *parameter),
                _ => false,
            }),
            Ty::Array(element, _) | Ty::Option(element) => self.mentions_open_parameter(element, inference),
            Ty::Tuple(items) => items.iter().any(|item| self.mentions_open_parameter(item, inference)),
            Ty::Fn(params, result) => params.iter().any(|param| self.mentions_open_parameter(param, inference)) || self.mentions_open_parameter(result, inference),
            _ => false,
        }
    }

    /// The substitution for every parameter: what was found, else the default, else `E0181` naming
    /// the first parameter left open, a template parameter (`Fn` bound) included.
    pub(in crate::types) fn complete_arguments(&mut self, inference: &mut Inference<'_>, signature: &GenericSignature, item: DeclId, function: DeclId, file: FileId, span: Span) -> Option<Substitution> {
        let mut substitution = Substitution::new();
        for parameter in signature.parameters() {
            let value = match inference.found().get(parameter.decl) {
                Some(value) => Some(value.clone()),
                None if parameter.default.is_some() => self.default_argument(parameter, &substitution, file),
                None => None,
            };
            let Some(value) = value else {
                let (name, turbofish) = self.callee_names(item, function);
                let (kind, how) = match parameter.kind {
                    ParameterKind::Const(_) => ("const parameter", "an array argument or"),
                    _ => ("type parameter", "an argument or"),
                };
                let message = format!("cannot infer the {kind} `{}` of `{name}`: no argument or expected type gives it; write it as `{turbofish}` (§3.8.7)", parameter.name);
                self.coded(file, "E0181", span, message, &format!("{how} turbofish would give `{}`", parameter.name));
                return None;
            };
            substitution.bind(parameter.decl, value);
        }
        Some(substitution)
    }

    /// What a diagnostic about the arguments of `item` calls the callee and how the turbofish is written: the item's own
    /// name, or for an `impl` (which has none) `Type::function` and `Type::<..>::function`, naming the function called.
    fn callee_names(&self, item: DeclId, function: DeclId) -> (String, String) {
        let own = self.state.design.decl_data(item).name.as_ref().map(|n| n.text.clone());
        if let Some(name) = own {
            return (name.clone(), format!("{name}::<..>"));
        }
        let function_name = self.state.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let DeclKind::Impl(data) = &self.state.design.decl_data(item).kind else { return (function_name.clone(), format!("{function_name}::<..>")) };
        let Resolution::Decl(owner) = self.state.design.type_resolution(data.self_ty) else { return (function_name.clone(), format!("{function_name}::<..>")) };
        let type_name = self.state.design.decl_data(owner).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        (format!("{type_name}::{function_name}"), format!("{type_name}::<..>::{function_name}"))
    }
}
