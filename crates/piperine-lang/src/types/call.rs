//! Calls of functions and closures. A call is typed from the declared signature of its callee
//! after each argument is checked against its parameter; a closure takes its parameter types
//! from the function type of the parameter it is passed to.

use crate::refusal::RefusedForm;
use crate::refusal::forms::ValueForm;
use crate::msl::Span;
use crate::pom::{Arg, CallClass, DeclId, DeclKind, DeferReason, ExprId, ExprKind, FileId, LitKind, PatKind, Resolution};

use super::catalogue::{Catalogue, Lookup};
use super::check::{Checker, Context};
use super::flow::ExitFrame;
use super::{Builtin, Callee, Method, Ty};

/// The carrier methods of B.1: not rows of Appendix H, so a miss on them is not `E0161`.
const CARRIER_METHODS: &[&str] = &["changed", "prev", "rise", "fall", "others", "inflow"];

/// The methods the explicit model of unknowns took out (§3.3): a call is `E0161` with the help that names the three forms.
const REMOVED_FLAG_METHODS: &[&str] = &["known", "known_or", "is_known", "high", "level", "merge"];

impl Checker<'_> {
    /// What a callee path names, if the callee is a plain path.
    pub(super) fn callee_resolution(&self, callee: ExprId) -> Option<Resolution> {
        match &self.design.expr_node(callee).kind {
            ExprKind::Path(path) => self.design.path_node(*path).segments.last().map(|s| s.res),
            _ => None,
        }
    }

    /// The call `expr`: a call of a function, or of a name whose type is a function type.
    pub(super) fn call(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Call { callee, args, params, class } = &node.kind else { return None };
        if params.is_some() && !matches!(class, CallClass::Instance { module: Some(_), .. }) {
            self.refuse(file, node.span, ValueForm::ParamBlockOnCall);
            return None;
        }
        if matches!(self.callee_resolution(*callee), Some(Resolution::Builtin(id)) if id.info().name == "Some") {
            return self.some(expr, args, None);
        }
        if let Some(found) = self.default_call(expr, *callee, args.len()) {
            return found;
        }
        if let Some(found) = self.parameter_decode(expr, *callee, args) {
            return found;
        }
        match class {
            CallClass::Function => {}
            CallClass::Instance { module: Some(module), .. } => return self.placement(expr, *module),
            CallClass::Instance { module: None, .. } if matches!(self.callee_resolution(*callee), Some(Resolution::Builtin(_))) => {}
            // §3.8.6: a value of a type parameter bound by `Fn(A) -> R` is called as a function; the POM
            // reads the call as a placement because a module parameter has the same bound
            CallClass::Instance { module: None, .. } if let Some(parameter) = self.callable_parameter(*callee) => return self.call_through_bound(expr, parameter, args),
            // the `Fn` bound of the param names a type that failed (its refusal is reported where the type is written): the call has
            // nothing left to say
            CallClass::Instance { module: None, .. } if self.fn_bound_failed(*callee) => {
                for arg in args {
                    self.infer(arg.value);
                }
                return None;
            }
            // any other head is a value called by its type: a function, else `E0180` (§1.5)
            CallClass::Instance { module: None, .. } => return self.value_call(expr, *callee, args),
            CallClass::Probe => return self.probe(expr, false),
            CallClass::Unclassified => {
                self.refuse(file, node.span, ValueForm::CallThatIsNotFunctionCall);
                return None;
            }
        }
        match self.callee_resolution(*callee) {
            Some(Resolution::Builtin(id)) => self.core_call(expr, id, args),
            Some(Resolution::Decl(decl)) => match &self.design.decl_data(decl).kind {
                DeclKind::Fn(_) => self.function_call(expr, *callee, decl, args),
                // A local that is no function and hides the function of its name is `E0180` (§3.5.4).
                DeclKind::FnParam(_) | DeclKind::Binding(_) if self.hidden_function(decl).is_some() && matches!(self.infer(*callee), Some(ty) if !matches!(ty, Ty::Fn(..))) => self.hidden_function_call(expr, *callee, decl, args),
                DeclKind::FnParam(_) | DeclKind::Binding(_) => self.local_call(expr, decl, args),
                DeclKind::Struct(_) | DeclKind::Variant(_) => self.constructor_call(expr, decl, args),
                DeclKind::Param(_) | DeclKind::Const(_) | DeclKind::Let(_) if self.hidden_function(decl).is_some() => self.hidden_function_call(expr, *callee, decl, args),
                _ => {
                    self.refuse(file, node.span, ValueForm::CallOfThisDeclaration);
                    None
                }
            },
            // the callee is an expression, not a name (Part III `callee ::= path | call | '(' expr ')'`)
            _ => self.value_call(expr, *callee, args),
        }
    }

    /// A call whose callee is an expression that names nothing (`(f)(x)` aside, which is a name: `make()(x)`, `(|y: f64| y)(x)`,
    /// `(if c { f } else { g })(x)`): its value is called by the type it has (§1.5). A function type takes the arguments; any other
    /// type is `E0180`. A callee that did not type has its own diagnostic.
    fn value_call(&mut self, expr: ExprId, callee: ExprId, args: &[Arg]) -> Option<Ty> {
        match self.infer(callee) {
            Some(Ty::Fn(params, result)) => {
                self.apply(expr, &params, args)?;
                Some(*result)
            }
            found => {
                if let Some(found) = found {
                    let (file, span) = (self.file_of_expr(expr), self.design.expr_node(callee).span);
                    let message = format!("the callee's type `{}` is no function and no template (§1.5)", found.name(self.design));
                    self.coded(file, "E0180", span, message, "a call of a value that is not a function");
                }
                for arg in args {
                    self.infer(arg.value);
                }
                None
            }
        }
    }

    /// A call of a function of the reachable set: arguments against the declared parameters,
    /// the declared result as the type.
    fn function_call(&mut self, expr: ExprId, callee: ExprId, function: DeclId, args: &[Arg]) -> Option<Ty> {
        let function = self.overload_of(function, args);
        if let Some(intrinsic) = self.intrinsic_of(function) {
            return self.intrinsic_call(expr, intrinsic, args);
        }
        self.reach(function);
        self.refuse_outside_class(expr, function);
        self.emitting_call(expr, function);
        self.check_call_class(expr, function);
        if self.is_trait_function(function) {
            return self.trait_path_call(expr, function, args);
        }
        if self.design.decl_data(function).generics_span.is_some() {
            return self.generic_call(expr, callee, function, args);
        }
        if let Some(owner) = self.generic_impl_of(function) {
            return self.associated_call(expr, callee, function, owner, args);
        }
        let (params, ret) = self.signature(function)?;
        self.finish_call(expr, callee, function, params, ret, args, true)
    }

    /// The typing of a call of `function` once its parameter types and result are known (a generic
    /// function's are the substituted ones): the callee recorded, the arguments bound to the
    /// parameters, the result the call's type. `pointers` holds a closure that captures out of a
    /// parameter of function type, which is a function pointer; a parameter bounded by `Fn` is not one.
    pub(super) fn finish_call(&mut self, expr: ExprId, callee: ExprId, function: DeclId, params: Vec<Ty>, ret: Ty, args: &[Arg], pointers: bool) -> Option<Ty> {
        self.callees.insert(expr, Callee::Function(function));
        self.exprs[callee.index()] = Some(Ty::Fn(params.clone(), Box::new(ret.clone())));
        let name = self.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let required = params.len() - params.iter().rev().take_while(|ty| matches!(ty, Ty::Option(_))).count();
        let named: Vec<Option<usize>> = args
            .iter()
            .map(|arg| match (&arg.name, arg.res) {
                (Some(_), Resolution::Decl(param)) => self.design.decl_data(param).parent.and_then(|owner| {
                    self.design.decl_data(owner).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::FnParam(_))).position(|c| *c == param)
                }),
                _ => None,
            })
            .collect();
        self.bind_arguments(expr, &name, &params, required, args, &named)?;
        // a parameter of function type is a function pointer: a closure that captures is refused
        let slots = self.bindings.get(&expr).cloned().unwrap_or_default();
        for (slot, param) in slots.iter().zip(&params) {
            if let (true, Some(arg), Ty::Fn(..)) = (pointers, slot, param) {
                self.refuse_capturing_pointer(*arg);
            }
        }
        Some(ret)
    }

    /// §3.2: binds the arguments of a call of `name` to its parameters. The first `required`
    /// parameters are required, the rest optional (a function's maximal trailing run of
    /// `Option<T>` ones, §3.2.7); positional arguments bind in order, a named one binds the
    /// parameter `named[i]` it names (`None`: it names no parameter, `E0140`; for a function the
    /// POM has reported that already), a named one for a required parameter is `E0140`
    /// (§3.2.8), and a required parameter left unbound or an argument past the last parameter
    /// is `E0141`. An argument of type `T` for `Option<T>` is `Some` of it, one of type
    /// `Option<T>` is as it is (§3.2.3, §3.2.10). The binding, one slot per parameter, is kept
    /// per call.
    pub(super) fn bind_arguments(&mut self, expr: ExprId, name: &str, params: &[Ty], required: usize, args: &[Arg], named: &[Option<usize>]) -> Option<()> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let mut slots: Vec<Option<ExprId>> = vec![None; params.len()];
        let mut bound: Vec<(ExprId, usize)> = Vec::new();
        let mut ok = true;
        for (i, arg) in args.iter().enumerate() {
            let slot = if arg.name.is_none() { Some(i) } else { named.get(i).copied().flatten() };
            match slot {
                Some(slot) if slot < params.len() && slots[slot].is_some() => {
                    let message = format!("the parameter {} of `{name}` is already bound", slot + 1);
                    self.coded(file, "E0140", arg.span, message, "already bound");
                    ok = false;
                    self.infer(arg.value);
                }
                Some(slot) if slot < params.len() => {
                    if arg.name.is_some() && slot < required {
                        let message = format!("`{name}`: a required parameter, or an `Option` one followed by a required one, is positional only, not bound by name (§3.2.8)");
                        self.coded(file, "E0140", arg.span, message, "positional only");
                        ok = false;
                    }
                    slots[slot] = Some(arg.value);
                    bound.push((arg.value, slot));
                }
                _ => {
                    // Past the last parameter, or a name that is no parameter.
                    if let Some(written) = &arg.name {
                        let message = format!("no optional parameter `{}` in `{name}`", written.text);
                        self.coded(file, "E0140", arg.span, message, "not an optional parameter");
                    }
                    ok = false;
                    self.infer(arg.value);
                }
            }
        }
        let unbound = slots[..required].iter().any(Option::is_none);
        if unbound || args.len() > params.len() {
            let found = args.len();
            let message = if required == params.len() { format!("`{name}` takes {required} argument(s), found {found}") } else { format!("`{name}` takes {required} to {} arguments, found {found}", params.len()) };
            self.coded(file, "E0141", span, message, "wrong number of arguments");
            ok = false;
        }
        for (value, slot) in bound {
            self.check_argument(value, &params[slot]);
        }
        if !ok {
            return None;
        }
        self.bindings.insert(expr, slots);
        Some(())
    }

    /// The function of the name of `hider` (a param, a `let` or a `const` that is not a function)
    /// that `hider` hides: a `fn` of that name among the children of an enclosing namespace
    /// (Part I §3.5.4).
    fn hidden_function(&self, hider: DeclId) -> Option<DeclId> {
        let name = &self.design.decl_data(hider).name.as_ref()?.text;
        let mut scope = self.design.decl_data(hider).parent;
        while let Some(namespace) = scope {
            let data = self.design.decl_data(namespace);
            let hidden = data.children.iter().copied().find(|child| {
                let child_data = self.design.decl_data(*child);
                matches!(child_data.kind, DeclKind::Fn(_)) && child_data.name.as_ref().is_some_and(|candidate| candidate.text == *name)
            });
            if hidden.is_some() {
                return hidden;
            }
            scope = data.parent;
        }
        None
    }

    /// `E0180` "expected a function, found `<type>`" for a call whose callee is a value that
    /// hides the function of its name, with a note at that function's declaration (Part I
    /// §3.5.4, L20). The arguments are still inferred, so the one mistake is one diagnostic.
    fn hidden_function_call(&mut self, expr: ExprId, callee: ExprId, hider: DeclId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(callee).span;
        let found = self.infer(callee).map_or_else(|| "a value".to_string(), |ty| ty.name(self.design));
        let name = self.design.decl_data(hider).name.as_ref().map_or_else(String::new, |name| name.text.clone());
        let mut diagnostic = crate::msl::Diagnostic::new("E0180", format!("expected a function, found `{found}`"), span, format!("`{name}` is a `{found}` here, not the function"));
        if let Some(function) = self.hidden_function(hider) {
            let declaration = self.design.decl_data(function);
            if declaration.file == file {
                diagnostic = diagnostic.with_label(declaration.name.as_ref().map_or(declaration.span, |name| name.span), format!("the function `{name}` is hidden by the {} of the same name", RefusedForm::declaration_name(&self.design.decl_data(hider).kind)));
            }
        }
        self.report(file, diagnostic);
        for arg in args {
            self.infer(arg.value);
        }
        None
    }

    /// `E0141` (Part I §3.2.6): a call that takes `expected` arguments (a count, or a range
    /// spelled `1 to 2`) and has the arguments of `args`. Every argument is still inferred, so
    /// one wrong count is one diagnostic.
    pub(super) fn arity_mismatch(&mut self, file: FileId, span: Span, expected: impl std::fmt::Display, args: &[Arg]) {
        let found = args.len();
        self.coded(file, "E0141", span, format!("expected {expected} argument(s), found {found}"), "wrong number of arguments");
        for arg in args {
            self.infer(arg.value);
        }
    }

    /// Check each argument against its parameter, in order. Only a function call binds by name
    /// (`bind_arguments`); the callers of this one take positional arguments.
    pub(super) fn apply(&mut self, expr: ExprId, params: &[Ty], args: &[Arg]) -> Option<()> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        if args.len() != params.len() {
            self.arity_mismatch(file, span, params.len(), args);
            return None;
        }
        for (arg, ty) in args.iter().zip(params) {
            self.check_argument(arg.value, ty);
        }
        Some(())
    }

    /// A method call: a built-in the stdlib's own item stands for (`prev`, `cast`, ..), or
    /// a `Deferred` or `Unresolved` one resolved against the Appendix H catalogue on the type of
    /// its receiver, the row recorded (DIG-02). Every other method is `P0001` naming it.
    pub(super) fn method_call(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::MethodCall { recv, method, generics, args, res } = &node.kind else { return None };
        let name = method.text.as_str();
        if generics.is_some() {
            self.refuse_with(file, node.span, RefusedForm::method(), [name.to_string()]);
            return None;
        }
        if name == "others" && self.others_on_one_driver(expr, *recv, args) {
            return None;
        }
        // `c0.cs.abs()`: the receiver's type is the concrete item's, which nothing here gives (§8.4.8)
        if self.untyped_template_receiver(*recv, args) {
            return None;
        }
        // The built-in a stdlib item resolved to, or the built-in row of the language.
        let builtin = match res {
            Resolution::Decl(f) => match self.stdlib_builtin(*f) {
                Some(builtin) => Some(builtin),
                // a method the project writes in an `impl` or a trait is typed from its signature
                None if self.is_project_method(*f) => None,
                None => {
                    let replaced = matches!(name, "rise" | "fall");
                    if replaced {
                        self.refuse_with(file, node.span, ValueForm::MethodDefinedByProjectCheckerKnowsOnly, [name.to_string(), name.to_string()]);
                    } else {
                        self.refuse_with(file, node.span, RefusedForm::method(), [name.to_string()]);
                    }
                    return None;
                }
            },
            Resolution::Builtin(id) => match id.info().name {
                "prev" => Some(Builtin::Prev),
                "changed" => Some(Builtin::Changed),
                "inflow" => return self.inflow_read(expr, *recv, args),
                "others" => return self.others_read(expr, *recv, args),
                _ => {
                    self.refuse_with(file, node.span, RefusedForm::method(), [name.to_string()]);
                    return None;
                }
            },
            Resolution::Deferred(DeferReason::ReceiverTypeUnknown) | Resolution::Unresolved => None,
            Resolution::Deferred(_) => {
                self.refuse_with(file, node.span, RefusedForm::method(), [name.to_string()]);
                return None;
            }
        };
        // `r.changed()` and `r.prev()` on a param (§5.3.1): the param's history, not a carrier's.
        if matches!(name, "changed" | "prev") && self.param_receiver(*recv).is_some() {
            return self.param_history(expr, name == "changed", *recv, args);
        }
        if builtin == Some(Builtin::Prev) {
            return self.prev_read(expr, *recv, args);
        }
        if let Some(edge @ (Builtin::Changed | Builtin::Rise | Builtin::Fall)) = builtin {
            return self.edge_read(expr, edge, *recv, args);
        }
        // §7 rule 13: a function of a bundle, called on a bundle carrier or on a bundle handle (`bundle_handles.rs`)
        if let Some(bundle) = self.bundle_named_type(*recv) {
            self.exprs[recv.index()] = Some(bundle.clone());
            let Some(function) = self.method_of(&bundle, name) else {
                let message = format!("no method `{name}` on `{}`", bundle.name(self.design));
                self.coded(file, "E0161", method.span, message, "a bundle has the functions of its `impl` and no others");
                return None;
            };
            return self.method_call_of(expr, *recv, &bundle, function, args);
        }
        // §3.1.7: a method of a carrier is the carrier's own (above), then the `impl`s of its nature, then the value's
        if let Some(carrier @ Ty::Carrier { .. }) = self.carrier_named(*recv)
            && let Some(function) = self.method_of(&carrier, name)
        {
            self.infer(*recv)?;
            return self.method_call_of(expr, *recv, &carrier, function, args);
        }
        let recv_ty = self.infer(*recv)?;
        // §3 rule 9: a value of a type parameter has the methods its bounds give
        if let Ty::Param(param) = recv_ty {
            return self.parameter_method(param, name, method.span, args, file, (expr, *recv));
        }
        // §3.1.7: the methods of a struct or an enum are the project's
        if matches!(recv_ty, Ty::Struct(_) | Ty::Enum(_))
            && let Some(function) = self.method_of(&recv_ty, name)
        {
            return self.method_call_of(expr, *recv, &recv_ty, function, args);
        }
        let (row, params, result) = match Catalogue::method(&recv_ty, name) {
            Lookup::Row(row, params, result) => (row, params, result),
            Lookup::Payload(row) => return self.payload_row(expr, row, &recv_ty, args),
            miss => {
                if let Some(function) = self.method_of(&recv_ty, name) {
                    return self.method_call_of(expr, *recv, &recv_ty, function, args);
                }
                let type_name = recv_ty.name(self.design);
                // `clone` is no method of any type (§1.4.6): a value is copied where it is used.
                let copy_method = name == "clone" && !self.project_defines(name);
                // a struct or an enum has the project's methods and no others (§3.1.7), so a miss is absent
                let own_only = matches!(recv_ty, Ty::Struct(_) | Ty::Enum(_));
                if copy_method || own_only || (matches!(miss, Lookup::Absent) && !self.project_defines(name) && !CARRIER_METHODS.contains(&name)) {
                    let message = format!("no method `{name}` on `{type_name}`");
                    let diagnostic = crate::msl::Diagnostic::new("E0161", message, method.span, format!("`{type_name}` has no such method (Appendix H)"));
                    let help = if copy_method {
                        Some(crate::msl::diag::COPY_HELP)
                    } else if REMOVED_FLAG_METHODS.contains(&name) {
                        Some(crate::msl::diag::UNKNOWN_HELP)
                    } else {
                        None
                    };
                    self.report(file, help.map_or(diagnostic.clone(), |help| diagnostic.with_help(help)));
                } else {
                    self.refuse_with(file, method.span, ValueForm::MethodOn, [name.to_string(), type_name.to_string()]);
                }
                for arg in args {
                    self.infer(arg.value);
                }
                return None;
            }
        };
        // An argument of the receiver's type takes it when it is a literal (DIG-01 AC 2).
        self.apply(expr, &params, args)?;
        self.methods.insert(expr, row);
        Some(result)
    }

    /// A row of H.3 or H.4 that takes a closure, a message or a value of the payload's type:
    /// `expect`, `unwrap_or_else`, `map`, `and_then` on an `Option<T>`, `then` and `then_some` on
    /// a `bool`. A closure's parameter is the payload `T`; its body's type is the result's.
    fn payload_row(&mut self, expr: ExprId, row: Method, recv: &Ty, args: &[Arg]) -> Option<Ty> {
        if matches!(recv, Ty::Array(..) | Ty::Range { .. } | Ty::Iterator { .. }) {
            return self.sequence_row(expr, row, recv, args);
        }
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let payload = match recv {
            Ty::Option(inner) => (**inner).clone(),
            _ => Ty::Unit,
        };
        if args.len() != 1 {
            self.arity_mismatch(file, node.span, 1, args);
            return None;
        }
        let value = args[0].value;
        let result = match row {
            Method::Expect => {
                if !matches!(&self.design.expr_node(value).kind, ExprKind::Lit(lit) if matches!(lit.kind, LitKind::Str { .. })) {
                    self.infer(value);
                    self.refuse(file, self.design.expr_node(value).span, ValueForm::ExpectWithMessageThatIsNotString);
                    return None;
                }
                self.check(value, &Ty::Str);
                payload
            }
            Method::ThenSome => Ty::Option(Box::new(self.infer(value)?)),
            Method::UnwrapOrElse => {
                self.check_argument(value, &Ty::Fn(vec![], Box::new(payload.clone())));
                payload
            }
            Method::Map | Method::AndThen | Method::Then => {
                if !matches!(self.design.expr_node(value).kind, ExprKind::Closure { .. }) {
                    self.infer(value);
                    self.refuse_with(file, self.design.expr_node(value).span, ValueForm::WithArgumentThatIsNotClosure, [match row { Method::Map => "map", Method::AndThen => "and_then", _ => "then" }.to_string()]);
                    return None;
                }
                let params: Vec<Ty> = if row == Method::Then { vec![] } else { vec![payload] };
                let body = self.closure_typed(value, &params, None)?;
                match row {
                    Method::AndThen if matches!(body, Ty::Option(_)) => body,
                    Method::AndThen => {
                        let span = self.design.expr_node(value).span;
                        self.mismatch_text(file, span, "Option<_>", &body.name(self.design));
                        return None;
                    }
                    _ => Ty::Option(Box::new(body)),
                }
            }
            _ => return None,
        };
        self.methods.insert(expr, row);
        Some(result)
    }

    /// A closure passed where `expected` is the declared type of the parameter. Anywhere else
    /// it is `P0001`: the subset has no type variable to infer its parameters with.
    pub(super) fn closure(&mut self, expr: ExprId, expected: &Ty) -> Option<Ty> {
        let Ty::Fn(declared, result) = expected else { return self.closure_value(expr) };
        self.closure_typed(expr, declared, Some(result))?;
        Some(expected.clone())
    }

    /// A closure whose parameters take `declared`: its body is checked against `result`, or
    /// inferred when `result` is `None` (the rows of Appendix H.3 and H.4 that take a closure,
    /// whose result type is the body's). Gives the type of the body.
    pub(super) fn closure_typed(&mut self, expr: ExprId, declared: &[Ty], result: Option<&Ty>) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Closure { params, ret, body, .. } = &node.kind else { return None };
        if params.len() != declared.len() {
            let message = format!("expected a closure of {} parameter(s), found {}", declared.len(), params.len());
            self.report(file, crate::msl::Diagnostic::new("E0180", format!("mismatched types: {message}"), node.span, message));
            return None;
        }
        for (param, ty) in params.iter().zip(declared) {
            let pat = self.design.pat_node(param.pat).clone();
            let PatKind::Ident { sub: None, binding, res: Resolution::Unresolved, .. } = &pat.kind else {
                // §1.6.11: a closure parameter takes an irrefutable pattern
                self.irrefutable_pattern(file, param.pat, ty, "a closure parameter");
                continue;
            };
            if let Some(annotation) = param.ty
                && let Some(written) = self.lower_type(annotation)
                && written != *ty
            {
                let span = self.design.type_node(annotation).span;
                self.mismatch(file, span, ty, &written);
            }
            self.locals.insert(*binding, ty.clone());
        }
        let written = ret.and_then(|annotation| self.lower_type(annotation).map(|ty| (annotation, ty)));
        if let (Some(result), Some((annotation, written))) = (result, &written)
            && written != result
        {
            let span = self.design.type_node(*annotation).span;
            self.mismatch(file, span, result, written);
        }
        let outer = std::mem::replace(&mut self.context, Context::Function);
        self.places.enter_closure(node.span);
        let given = result.or(written.as_ref().map(|(_, ty)| ty));
        let enclosing = self.flow.enter_body(ExitFrame::new(given.cloned(), given.is_none()));
        let found = match given {
            Some(result) => {
                self.check(*body, result);
                Some(result.clone())
            }
            None => self.infer(*body),
        };
        let returned = self.flow.leave_body(enclosing).and_then(|frame| frame.returned);
        self.context = outer;
        self.places.leave_closure();
        let found = self.settle_result(file, node.span, found?, returned);
        self.exprs[expr.index()] = Some(Ty::Fn(declared.to_vec(), Box::new(found.clone())));
        self.record_closure(expr);
        Some(found)
    }

    /// The result type of a closure whose result was inferred: its body's, or, when the body
    /// never completes, the type of its first `return`; a `return` of another type than the body's
    /// is a mismatch.
    fn settle_result(&mut self, file: crate::pom::FileId, span: crate::msl::Span, body: Ty, returned: Option<Ty>) -> Ty {
        match returned {
            Some(returned) if body == Ty::Never => returned,
            Some(returned) if body != returned => {
                self.mismatch(file, span, &returned, &body);
                returned
            }
            _ => body,
        }
    }
}
