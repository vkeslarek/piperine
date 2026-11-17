//! Calls of functions and closures. A call is typed from the declared signature of its callee
//! after each argument is checked against its parameter; a closure takes its parameter types
//! from the function type of the parameter it is passed to.

use crate::pom::{Arg, CallClass, DeclId, DeclKind, DeferReason, ExprId, ExprKind, PatKind, Resolution};

use super::carrier::flag_builtin;
use super::catalogue::{Catalogue, Lookup};
use super::check::{Checker, Context};
use super::unsupported::Unsupported;
use super::{Builtin, Callee, Ty};

/// The carrier and flag methods of B.1: not rows of Appendix H, so a miss on them is not `E0161`.
const CARRIER_METHODS: &[&str] = &["known", "known_or", "is_known", "high", "changed", "prev", "rise", "fall", "others", "inflow", "level"];

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
            self.unsupported(file, node.span, "param block on a call");
            return None;
        }
        if matches!(self.callee_resolution(*callee), Some(Resolution::Builtin(id)) if id.info().name == "Some") {
            return self.some(expr, args, None);
        }
        match class {
            CallClass::Function => {}
            CallClass::Instance { module: Some(module), .. } => return self.placement(expr, *module),
            CallClass::Instance { module: None, .. } if matches!(self.callee_resolution(*callee), Some(Resolution::Builtin(_))) => {}
            CallClass::Instance { module: None, .. } => {
                self.unsupported(file, node.span, "placement through a template value or a function");
                return None;
            }
            CallClass::Probe => return self.probe(expr, false),
            CallClass::Unclassified => {
                self.unsupported(file, node.span, "call that is not a function call");
                return None;
            }
        }
        match self.callee_resolution(*callee) {
            Some(Resolution::Builtin(id)) => self.core_call(expr, id, args),
            Some(Resolution::Decl(decl)) => match &self.design.decl_data(decl).kind {
                DeclKind::Fn(_) => self.function_call(expr, *callee, decl, args),
                DeclKind::FnParam(_) | DeclKind::Binding(_) => self.indirect_call(expr, args),
                _ => {
                    self.unsupported(file, node.span, "call of this declaration");
                    None
                }
            },
            _ => {
                self.unsupported(file, node.span, Unsupported::expr(&node.kind));
                None
            }
        }
    }

    /// A call of a function of the reachable set: arguments against the declared parameters,
    /// the declared result as the type.
    fn function_call(&mut self, expr: ExprId, callee: ExprId, function: DeclId, args: &[Arg]) -> Option<Ty> {
        self.reach(function);
        self.emitting_call(expr, function);
        let (params, ret) = self.signature(function)?;
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
        Some(ret)
    }

    /// A call of a parameter or a local whose type is `fn(..) -> R`: there is no named function
    /// to record as its callee, so the form is outside the subset.
    fn indirect_call(&mut self, expr: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        self.unsupported(file, span, "call of a function value (a parameter or a local)");
        for arg in args {
            self.infer(arg.value);
        }
        None
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

    /// Check each argument against its parameter, in order. Only a function call binds by name
    /// (`bind_arguments`); the callers of this one take positional arguments.
    pub(super) fn apply(&mut self, expr: ExprId, params: &[Ty], args: &[Arg]) -> Option<()> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        if args.len() != params.len() {
            let message = format!("expected {} argument(s), found {}", params.len(), args.len());
            self.report(file, crate::msl::Diagnostic::new("P0002", format!("mismatched types: {message}"), span, message));
            for arg in args {
                self.infer(arg.value);
            }
            return None;
        }
        for (arg, ty) in args.iter().zip(params) {
            self.check_argument(arg.value, ty);
        }
        Some(())
    }

    /// A method call: a built-in the stdlib's own item stands for (`known_or`, `prev`, ..), or
    /// a `Deferred` or `Unresolved` one resolved against the Appendix H catalogue on the type of
    /// its receiver, the row recorded (DIG-02). Every other method is `P0001` naming it.
    pub(super) fn method_call(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::MethodCall { recv, method, generics, args, res } = &node.kind else { return None };
        let name = method.text.as_str();
        if generics.is_some() {
            self.unsupported(file, node.span, &format!("method `{name}`"));
            return None;
        }
        // The built-in a stdlib item resolved to, or the built-in row of the language.
        let builtin = match res {
            Resolution::Decl(f) => match self.stdlib_builtin(*f) {
                Some(builtin) => Some(builtin),
                None => {
                    let replaced = flag_builtin(name).is_some() || matches!(name, "rise" | "fall");
                    let what = if replaced { format!("method `{name}` defined by the project (the checker knows only the stdlib's own `{name}`)") } else { format!("method `{name}`") };
                    self.unsupported(file, node.span, &what);
                    return None;
                }
            },
            Resolution::Builtin(id) => match id.info().name {
                "prev" => Some(Builtin::Prev),
                "changed" => Some(Builtin::Changed),
                _ => {
                    self.unsupported(file, node.span, &format!("method `{name}`"));
                    return None;
                }
            },
            Resolution::Deferred(DeferReason::ReceiverTypeUnknown) | Resolution::Unresolved => None,
            Resolution::Deferred(_) => {
                self.unsupported(file, node.span, &format!("method `{name}`"));
                return None;
            }
        };
        if builtin == Some(Builtin::Prev) {
            return self.prev_read(expr, *recv, args);
        }
        if let Some(edge @ (Builtin::Changed | Builtin::Rise | Builtin::Fall)) = builtin {
            return self.edge_read(expr, edge, *recv, args);
        }
        // A receiver the POM could not type might be a value of a project's own `impl`: the
        // checker knows only the stdlib's flag reads (§3.3.7).
        if builtin.is_none() && flag_builtin(name).is_some() && self.project_methods.contains(name) {
            self.unsupported(file, node.span, &format!("method `{name}` defined by the project (the checker knows only the stdlib's own `{name}`)"));
            self.infer(*recv);
            return None;
        }
        let recv_ty = self.infer(*recv)?;
        // The four flag-total reads apply to any value and to a carrier's value.
        let builtin = builtin.or_else(|| flag_builtin(name));
        if let Some(builtin) = builtin {
            if let Some(result) = self.flag_read(expr, builtin, &recv_ty, args) {
                return result;
            }
        }
        let (row, params, result) = match Catalogue::method(&recv_ty, name) {
            Lookup::Row(row, params, result) => (row, params, result),
            miss => {
                let type_name = recv_ty.name(self.design);
                if matches!(miss, Lookup::Absent) && !self.project_methods.contains(name) && !CARRIER_METHODS.contains(&name) {
                    let message = format!("no method `{name}` on `{type_name}`");
                    self.report(file, crate::msl::Diagnostic::new("E0161", message, method.span, format!("`{type_name}` has no such method (Appendix H)")));
                } else {
                    self.unsupported(file, method.span, &format!("method `{name}` on `{type_name}`"));
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

    /// A closure passed where `expected` is the declared type of the parameter. Anywhere else
    /// it is `P0001`: the subset has no type variable to infer its parameters with.
    pub(super) fn closure(&mut self, expr: ExprId, expected: &Ty) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Closure { params, ret, body, .. } = &node.kind else { return None };
        let Ty::Fn(declared, result) = expected else {
            self.unsupported(file, node.span, "closure outside an argument whose parameter has a function type");
            return None;
        };
        if params.len() != declared.len() {
            let message = format!("expected a closure of {} parameter(s), found {}", declared.len(), params.len());
            self.report(file, crate::msl::Diagnostic::new("P0002", format!("mismatched types: {message}"), node.span, message));
            return None;
        }
        for (param, ty) in params.iter().zip(declared) {
            let pat = self.design.pat_node(param.pat).clone();
            let PatKind::Ident { by_ref: false, mutable: false, sub: None, binding, res: Resolution::Unresolved, .. } = &pat.kind else {
                self.unsupported(file, pat.span, Unsupported::pat(&pat.kind));
                return None;
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
        if let Some(annotation) = ret
            && let Some(written) = self.lower_type(*annotation)
            && written != **result
        {
            let span = self.design.type_node(*annotation).span;
            self.mismatch(file, span, result, &written);
        }
        let outer = std::mem::replace(&mut self.context, Context::Function);
        self.check(*body, result);
        self.context = outer;
        Some(expected.clone())
    }
}
