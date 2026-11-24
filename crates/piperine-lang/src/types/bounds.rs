//! What a type parameter's bounds give (part I §3 rule 9, §3.8.9, GN-07): a generic body is
//! checked once at its definition with each parameter opaque, so a value of type `T` offers exactly
//! what the bounds of `T` offer. `T: PartialEq` gives `==`, `T: PartialOrd` gives `<`, `T: Repr`
//! gives `T::WIDTH`, `x.encode()` and `T::decode(levels)`, `T: Default` gives `T::default()`, and
//! `F: Fn(A) -> R` gives a call. Anything else is refused at the definition naming the bound that
//! would give it; the errors that depend on the arguments (a `where`, a length) wait for an
//! instantiation.

use crate::msl::Span;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericParamData, Resolution, TypeId};

use super::check::Checker;
use super::derives::DerivedTrait;
use super::generics::{Bound, GenericSignature};
use super::{ConstTerm, GenericValue, Length, Substitution, Ty};

/// Whether a type satisfies a bound (§3.8.7).
pub(super) struct BoundCheck<'c, 'd> {
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> BoundCheck<'c, 'd> {
    /// A check of bounds with the checker's tables.
    pub(super) fn new(checker: &'c mut Checker<'d>) -> Self {
        BoundCheck { checker }
    }

    /// Whether `ty` satisfies `bound`: an H.8 trait through the derive table, `Repr` through the
    /// type's layout, `Nature` for a nature, a project trait where the type names it as a bound, and
    /// `Fn(A) -> R` for a function type of that signature. A type parameter satisfies what its own
    /// bounds give.
    pub(super) fn satisfies(&mut self, ty: &Ty, bound: &Bound) -> bool {
        match bound {
            Bound::Library(name) => DerivedTrait::ALL.into_iter().find(|candidate| candidate.name() == *name).is_some_and(|which| self.checker.implements(ty, which)),
            Bound::Repr => match ty {
                Ty::Param(param) => self.checker.bounds_of(*param).contains(&Bound::Repr),
                other => self.checker.repr_levels(other).is_some(),
            },
            Bound::Nature => match ty {
                Ty::Param(param) => self.checker.bounds_of(*param).contains(&Bound::Nature),
                other => matches!(other, Ty::Node(_) | Ty::Carrier { .. }),
            },
            Bound::Resolve { arguments } => {
                let nature = match ty {
                    Ty::Param(nature) | Ty::Node(nature) | Ty::Carrier { nature, .. } => *nature,
                    _ => return false,
                };
                let wanted: Option<Vec<Ty>> = arguments.iter().map(|argument| self.checker.lower_type(*argument)).collect();
                let Some(wanted) = wanted else { return false };
                let Some(driven) = wanted.first() else { return false };
                super::nature::ResolveImpls::new(self.checker).of(nature).iter().any(|entry| entry.driven == *driven && wanted.get(1).is_none_or(|value| entry.value == *value))
            }
            Bound::Trait { decl, arguments } => {
                if matches!(ty, Ty::Param(param) if self.checker.bounds_of(*param).contains(bound)) {
                    return true;
                }
                let wanted: Option<Vec<Ty>> = arguments.iter().map(|argument| self.checker.lower_type(*argument)).collect();
                let Some(wanted) = wanted else { return false };
                let table = std::rc::Rc::clone(&self.checker.impls);
                let checker = &mut *self.checker;
                let entries = table.implementing(ty, *decl, &mut |candidate, bound| BoundCheck::new(&mut *checker).satisfies(candidate, bound));
                // a bound with no arguments asks for the trait whatever it is written with
                entries.iter().any(|entry| {
                    wanted.is_empty() || {
                        let given: Option<Vec<Ty>> = entry.trait_arguments.iter().map(|argument| checker.lower_type(*argument)).collect();
                        given.is_some_and(|given| given == wanted)
                    }
                })
            }
            Bound::Callable { inputs, output } => {
                let wanted = self.checker.callable_type(inputs, *output);
                match (ty, wanted) {
                    (Ty::Param(param), Some(wanted)) => self.checker.bounds_of(*param).iter().any(|own| matches!(own, Bound::Callable { inputs, output } if self.checker.callable_type(inputs, *output).as_ref() == Some(&wanted))),
                    (Ty::Module(module), Some(_)) => self.checker.first_differing_port(module, inputs).is_none(),
                    (found, Some(wanted)) => *found == wanted,
                    (_, None) => false,
                }
            }
        }
    }
}

impl Checker<'_> {
    /// The bounds written on the type parameter `param`, in order.
    pub(super) fn bounds_of(&self, param: DeclId) -> Vec<Bound> {
        let mut found: Vec<Bound> = match &self.design.decl_data(param).kind {
            DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) => bounds.iter().filter_map(|path| GenericSignature::bound(self.design, *path)).collect(),
            _ => Vec::new(),
        };
        // §3.1.2: the parameters of an `impl` take their bounds from the declaration of the type it is written for
        found.extend(self.inherited_bounds(param));
        found
    }

    /// The bounds the declaration gives the parameter `param` of an `impl` (`impl<T> Window<T>` with
    /// `struct Window<T: PartialOrd>`): those of the declaration's parameter written at the same place.
    fn inherited_bounds(&self, param: DeclId) -> Vec<Bound> {
        let Some(owner) = self.design.decl_data(param).parent else { return Vec::new() };
        let DeclKind::Impl(data) = &self.design.decl_data(owner).kind else { return Vec::new() };
        let crate::pom::TypeKind::Path(path) = &self.design.type_node(data.self_ty).kind else { return Vec::new() };
        let Some(segment) = self.design.path_node(*path).segments.last() else { return Vec::new() };
        let (Resolution::Decl(target), Some(crate::pom::GenericArgsNode::Angle { args, .. })) = (segment.res, &segment.args) else { return Vec::new() };
        let written = args.iter().position(|arg| matches!(arg, crate::pom::GenericArgNode::Type(ty) if matches!(self.design.type_resolution(*ty), Resolution::Decl(decl) if decl == param)));
        let declared = GenericSignature::of(self.design, target);
        written.and_then(|at| declared.parameters().get(at)).map_or_else(Vec::new, |parameter| parameter.bounds.clone())
    }

    /// Whether the bounds of `param` give the H.8 trait `which`, a supertrait of a bound included
    /// (`Ord` gives `Eq`, `PartialOrd` and `PartialEq`).
    pub(super) fn param_implements(&self, param: DeclId, which: DerivedTrait) -> bool {
        self.bounds_of(param).iter().any(|bound| match bound {
            Bound::Library(name) => DerivedTrait::ALL.into_iter().find(|candidate| candidate.name() == *name).is_some_and(|given| given.gives(which)),
            _ => false,
        })
    }

    /// The function type `Fn(inputs) -> output` a callable bound writes, `None` when a type in it
    /// is refused (the refusal is reported where the bound is read).
    pub(super) fn callable_type(&mut self, inputs: &[TypeId], output: Option<TypeId>) -> Option<Ty> {
        let inputs: Option<Vec<Ty>> = inputs.iter().map(|input| self.lower_type(*input)).collect();
        let output = match output {
            Some(output) => self.lower_type(output)?,
            None => Ty::Unit,
        };
        Some(Ty::Fn(inputs?, Box::new(output)))
    }

    /// The function parameter or binding `callee` names when its type is a type parameter bound by `Fn`.
    pub(super) fn callable_parameter(&mut self, callee: ExprId) -> Option<DeclId> {
        let Some(Resolution::Decl(decl)) = self.callee_resolution(callee) else { return None };
        if matches!(self.design.decl_data(decl).kind, DeclKind::Param(_)) {
            return self.module_param_callable(decl).map(|_| decl);
        }
        let Some(Ty::Param(param)) = self.locals.get(&decl).cloned() else { return None };
        self.callable_of(param).map(|_| decl)
    }

    /// Whether `callee` is a param whose type is a type parameter with an `Fn(..)` bound, though the bound did not type (an input
    /// that failed to lower): the failure is already reported.
    pub(super) fn fn_bound_failed(&mut self, callee: ExprId) -> bool {
        let Some(Resolution::Decl(decl)) = self.callee_resolution(callee) else { return false };
        let DeclKind::Param(data) = &self.design.decl_data(decl).kind else { return false };
        matches!(self.lower_type(data.ty), Some(Ty::Param(parameter)) if super::GenericSignature::is_callable(self.design, parameter))
    }

    /// The function type the `Fn(P)` bound of a param's type gives it: a param `cell: C` of a generic module whose `C` is bound by
    /// `Fn(P)` holds a template of a module, called with the connections of `P` (§8.4.1, §8.4.4).
    pub(super) fn module_param_callable(&mut self, param: DeclId) -> Option<Ty> {
        let DeclKind::Param(data) = &self.design.decl_data(param).kind else { return None };
        match self.lower_type(data.ty)? {
            Ty::Param(parameter) => self.callable_of(parameter),
            _ => None,
        }
    }

    /// A call of a value of a type parameter bounded by `Fn(A) -> R`: its arguments are positional
    /// only (§3.2.11, `E0140` for a named one) and checked against the bound's inputs.
    pub(super) fn call_through_bound(&mut self, expr: ExprId, parameter: DeclId, args: &[Arg]) -> Option<Ty> {
        if let Some(named) = args.iter().find(|arg| arg.name.is_some()) {
            let file = self.file_of_expr(expr);
            self.coded(file, "E0140", named.span, "a call through an `Fn` bound takes its arguments by position (§3.2.11)".to_string(), "positional only");
            for arg in args {
                self.infer(arg.value);
            }
            return None;
        }
        if matches!(self.design.decl_data(parameter).kind, DeclKind::Param(_)) {
            let Some(Ty::Fn(params, result)) = self.module_param_callable(parameter) else { return None };
            self.connect_through_bound(expr, &params, args)?;
            return Some(*result);
        }
        self.local_call(expr, parameter, args)
    }

    /// The connections of a placement through a template param bounded by `Fn(P)` (§8.4.5): a node input takes what a port of
    /// that nature takes at a placement, a node of a compatible nature or `gnd` (§8.1.10, `connect`); any other input is an
    /// argument of its type. `None` after an arity mismatch.
    fn connect_through_bound(&mut self, expr: ExprId, params: &[Ty], args: &[Arg]) -> Option<()> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        if args.len() != params.len() {
            self.arity_mismatch(file, span, params.len(), args);
            return None;
        }
        for (position, (arg, ty)) in args.iter().zip(params).enumerate() {
            match ty {
                Ty::Node(_) => {
                    let Some(found) = self.infer(arg.value) else { continue };
                    self.connect(arg.value, &format!("the node input {} of the `Fn` bound", position + 1), ty, &found);
                }
                _ => self.check_argument(arg.value, ty),
            }
        }
        Some(())
    }

    /// The function type a type parameter's `Fn(A) -> R` bound gives its values, `None` without one.
    pub(super) fn callable_of(&mut self, param: DeclId) -> Option<Ty> {
        let (inputs, output) = self.bounds_of(param).into_iter().find_map(|bound| match bound {
            Bound::Callable { inputs, output } => Some((inputs, output)),
            _ => None,
        })?;
        self.callable_type(&inputs, output)
    }

    /// The type parameter a path owner names (`T` of `T::WIDTH`), when it is one.
    pub(super) fn parameter_owner(&self, owner: Resolution) -> Option<DeclId> {
        match owner {
            Resolution::Decl(decl) if matches!(self.design.decl_data(decl).kind, DeclKind::GenericParam(GenericParamData::Type { .. })) => Some(decl),
            _ => None,
        }
    }

    /// `T::WIDTH` of a type parameter bound by `Repr`: a `u32`, known when `T` is (§2); without the
    /// bound it is `E0161` naming it.
    pub(super) fn parameter_width(&mut self, param: DeclId, file: FileId, span: Span) -> Option<Ty> {
        self.require_bound(param, &Bound::Repr, "WIDTH", file, span)?;
        Some(Ty::UInt(32))
    }

    /// `x.encode()` on a value of a type parameter bound by `Repr`: the `[bool; T::WIDTH]` of its
    /// levels (§2); `E0161` without the bound.
    pub(super) fn parameter_encode(&mut self, param: DeclId, args: &[Arg], file: FileId, span: Span) -> Option<Ty> {
        self.require_bound(param, &Bound::Repr, "encode", file, span)?;
        if !args.is_empty() {
            self.arity_mismatch(file, span, 0, args);
            return None;
        }
        Some(Ty::Array(Box::new(Ty::Bool), Length::Generic(ConstTerm::Width(param))))
    }

    /// `T::decode(levels)` on a type parameter bound by `Repr`: an `Option<T>` from `[bool; T::WIDTH]`
    /// (§2); `E0161` without the bound. `None` when `callee` is not such a path.
    pub(super) fn parameter_decode(&mut self, expr: ExprId, callee: ExprId, args: &[Arg]) -> Option<Option<Ty>> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        let segments = self.design.path_node(*path).segments.clone();
        let [owner, member] = segments.as_slice() else { return None };
        let param = self.parameter_owner(owner.res).filter(|_| member.name.text == "decode" && matches!(member.res, Resolution::Deferred(_) | Resolution::Builtin(_)))?;
        let (file, span) = (self.file_of_expr(expr), self.design.expr_node(expr).span);
        if self.require_bound(param, &Bound::Repr, "decode", file, span).is_none() {
            return Some(None);
        }
        let levels = Ty::Array(Box::new(Ty::Bool), Length::Generic(ConstTerm::Width(param)));
        if args.len() != 1 {
            self.arity_mismatch(file, span, 1, args);
            return Some(None);
        }
        self.check_argument(args[0].value, &levels);
        Some(Some(Ty::Option(Box::new(Ty::Param(param)))))
    }

    /// A method call on a value of the type parameter `param`: what its bounds give, else `E0161`
    /// naming the method, the parameter and the bound that would give it. The arguments are still
    /// typed, so the one mistake is one diagnostic.
    pub(super) fn parameter_method(&mut self, param: DeclId, name: &str, method_span: Span, args: &[Arg], file: FileId, call: (ExprId, ExprId)) -> Option<Ty> {
        // a method a project trait gives: through a bound of the parameter, or the trait itself for its `Self`
        if let Some((function, trait_arguments)) = self.trait_method_of(param, name) {
            // the arguments the bound writes for the trait's own parameters (`C: Convert<f64>`) replace them in the
            // signature, and the elaborator picks the `impl` of that argument list (GN-04)
            let wanted: Option<Vec<Ty>> = trait_arguments.iter().map(|argument| self.lower_type(*argument)).collect();
            if let (Some(wanted), Some(trait_decl)) = (wanted.filter(|wanted| !wanted.is_empty()), self.design.decl_data(function).parent) {
                let signature = self.generic_signature(trait_decl);
                let mut bound = Substitution::new();
                for (parameter, argument) in signature.parameters().iter().zip(&wanted) {
                    bound.bind(parameter.decl, GenericValue::Type(argument.clone()));
                }
                self.bound_substitution = Some(bound);
                self.bound_arguments.insert(call.0, wanted);
            }
            return self.method_call_of(call.0, call.1, &Ty::Param(param), function, args);
        }
        if name == "encode" && self.bounds_of(param).contains(&Bound::Repr) {
            return self.parameter_encode(param, args, file, method_span);
        }
        let given: Vec<String> = self.bounds_of(param).iter().map(|bound| self.bound_name(bound)).collect();
        let parameter = Ty::Param(param).name(self.design);
        let hint = if name == "clone" {
            "every value is copied; delete the name".to_string()
        } else if let Some(bound) = Self::bound_giving(name) {
            format!("`{name}` comes from `{bound}`: add `{parameter}: {bound}`")
        } else {
            "no bound of the parameter gives it".to_string()
        };
        let bounds = if given.is_empty() { "no bound".to_string() } else { format!("the bounds {}", given.join(" + ")) };
        let message = format!("no method `{name}` on `{parameter}`: {bounds} of `{parameter}` give none called `{name}` (§3 rule 9); {hint}");
        self.coded(file, "E0161", method_span, message, "a method no bound gives");
        for arg in args {
            self.infer(arg.value);
        }
        None
    }

    /// The function `name` a trait gives a value of the type parameter `param`: an item of a trait that
    /// bounds it (supertraits included), or, when `param` is a trait's own `Self`, of that trait.
    fn trait_method_of(&self, param: DeclId, name: &str) -> Option<(DeclId, Vec<TypeId>)> {
        let own = matches!(self.design.decl_data(param).kind, DeclKind::Trait(_)).then_some((param, Vec::new()));
        let bounded = self.bounds_of(param).into_iter().filter_map(|bound| if let Bound::Trait { decl, arguments } = bound { Some((decl, arguments)) } else { None });
        let mut pending: Vec<(DeclId, Vec<TypeId>)> = own.into_iter().chain(bounded).collect();
        while let Some((tr, arguments)) = pending.pop() {
            if let Some(item) = self.impls.item(tr, name).filter(|item| matches!(self.design.decl_data(*item).kind, DeclKind::Fn(_))) {
                return Some((item, arguments));
            }
            // a supertrait's arguments are its own, not the bound's
            pending.extend(self.impls.supertraits(tr).into_iter().map(|supertrait| (supertrait, Vec::new())));
        }
        None
    }

    /// `E0161` unless the bounds of `param` include `bound`; `Some(())` when they do.
    fn require_bound(&mut self, param: DeclId, bound: &Bound, member: &str, file: FileId, span: Span) -> Option<()> {
        if self.bounds_of(param).contains(bound) {
            return Some(());
        }
        let parameter = Ty::Param(param).name(self.design);
        let needed = self.bound_name(bound);
        let message = format!("`{parameter}::{member}` needs the bound `{needed}`, which the bounds of `{parameter}` do not give (§3 rule 9): add `{parameter}: {needed}`");
        self.coded(file, "E0161", span, message, "a member no bound gives");
        None
    }

    /// The bound as it is written, for messages.
    pub(super) fn bound_name(&self, bound: &Bound) -> String {
        match bound {
            Bound::Trait { decl, .. } => self.design.decl_data(*decl).name.as_ref().map_or_else(|| "a trait".to_string(), |n| n.text.clone()),
            Bound::Library(name) => (*name).to_string(),
            Bound::Repr => "Repr".to_string(),
            Bound::Nature => "Nature".to_string(),
            Bound::Resolve { arguments } => {
                let written: Vec<String> = arguments.iter().map(|argument| self.written_type_name(*argument)).collect();
                if written.is_empty() { "Resolve".to_string() } else { format!("Resolve<{}>", written.join(", ")) }
            }
            Bound::Callable { .. } => "Fn".to_string(),
        }
    }

    /// The name a type is written with: the last segment of its path, `_` for any other form.
    fn written_type_name(&self, ty: TypeId) -> String {
        match &self.design.type_node(ty).kind {
            crate::pom::TypeKind::Path(path) => self.design.path_node(*path).segments.last().map_or_else(|| "_".to_string(), |segment| segment.name.text.clone()),
            _ => "_".to_string(),
        }
    }

    /// The built-in bound that gives the method `name`, when one does.
    fn bound_giving(name: &str) -> Option<&'static str> {
        match name {
            "encode" | "decode" => Some("Repr"),
            "default" => Some("Default"),
            _ => None,
        }
    }
}
