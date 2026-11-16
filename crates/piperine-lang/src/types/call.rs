//! Calls of functions and closures. A call is typed from the declared signature of its callee
//! after each argument is checked against its parameter; a closure takes its parameter types
//! from the function type of the parameter it is passed to.

use crate::pom::{Arg, CallClass, DeclId, DeclKind, DeferReason, ExprId, ExprKind, PatKind, Resolution};

use super::catalogue::Catalogue;
use super::check::{Checker, Context};
use super::unsupported::Unsupported;
use super::{Callee, Ty};

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
        if params.is_some() {
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
        let (params, ret) = self.signature(function)?;
        self.callees.insert(expr, Callee::Function(function));
        self.exprs[callee.index()] = Some(Ty::Fn(params.clone(), Box::new(ret.clone())));
        self.apply(expr, &params, args)?;
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

    /// Check each argument against its parameter, in order. A function takes positional
    /// arguments only (the POM refuses `name: value` on a call that is not a placement).
    fn apply(&mut self, expr: ExprId, params: &[Ty], args: &[Arg]) -> Option<()> {
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

    /// A method call the POM left `Deferred` for want of the receiver's type: resolved against the
    /// Appendix H catalogue, the row recorded.
    pub(super) fn method_call(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::MethodCall { recv, method, generics, args, res } = &node.kind else { return None };
        if !matches!(res, Resolution::Deferred(DeferReason::ReceiverTypeUnknown)) || generics.is_some() {
            self.unsupported(file, node.span, &format!("method `{}`", method.text));
            return None;
        }
        let recv_ty = self.infer(*recv)?;
        let Some((row, params, result)) = Catalogue::method(&recv_ty, &method.text) else {
            self.unsupported(file, method.span, &format!("method `{}` on `{}`", method.text, recv_ty.name(self.design)));
            for arg in args {
                self.infer(arg.value);
            }
            return None;
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
