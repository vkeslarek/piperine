//! Flag-total functions (part I §3.3.6): a function whose every use of a parameter passes
//! through `level` (or a read that is built on it) never sees an unknown as a value, so an
//! `analog` block may hand it a discrete carrier. Every other bare read of a carrier in
//! `analog` stays `E0150` (§3.3.5).

use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::carrier::flag_builtin;
use super::check::Checker;
use super::Builtin;

/// Whether a parameter of a function is read only through flags, followed through the calls the
/// function makes with a cycle guard.
pub(super) struct FlagTotal<'c, 'd> {
    checker: &'c Checker<'d>,
    /// The `(function, parameter)` pairs being decided: a call that comes back to one is taken as
    /// flag-total, since only a bare read elsewhere can make the answer `false`.
    deciding: Vec<(DeclId, usize)>,
}

impl<'c, 'd> FlagTotal<'c, 'd> {
    /// A decision procedure over the design `checker` reads.
    pub(super) fn new(checker: &'c Checker<'d>) -> Self {
        FlagTotal { checker, deciding: Vec::new() }
    }

    /// Whether parameter `index` of `function` is flag-total: every use of it in the body is the
    /// receiver of a flag read, or the argument of a flag-total parameter of another function.
    /// A parameter the body never reads is flag-total as well.
    pub(super) fn parameter(&mut self, function: DeclId, index: usize) -> bool {
        if self.deciding.contains(&(function, index)) {
            return true;
        }
        let design = self.checker.design;
        let data = design.decl_data(function);
        let DeclKind::Fn(f) = &data.kind else { return false };
        let Some(body) = &f.body else { return false };
        let Some(param) = data.children.iter().copied().filter(|c| matches!(design.decl_data(*c).kind, DeclKind::FnParam(_))).nth(index) else { return false };
        let mut exprs = Vec::new();
        for root in crate::pom::block_exprs(design, body) {
            crate::pom::preorder(design, root, &mut exprs);
        }
        self.deciding.push((function, index));
        let mut allowed: Vec<ExprId> = Vec::new();
        for expr in &exprs {
            match &design.expr_node(*expr).kind {
                ExprKind::MethodCall { recv, res, method, .. } if self.checker.is_flag_read(res, &method.text) => allowed.push(self.checker.through_parens(*recv)),
                ExprKind::Call { callee, args, class: CallClass::Function, .. } => {
                    if let Some(Resolution::Decl(g)) = self.checker.callee_resolution(*callee) {
                        for (i, arg) in args.iter().enumerate() {
                            // a named argument may bind another parameter than position `i`: not decided
                            if arg.name.is_none() && self.parameter(g, i) {
                                allowed.push(self.checker.through_parens(arg.value));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let flag_total = exprs.iter().all(|e| !self.checker.reads_parameter(*e, param) || allowed.contains(e));
        self.deciding.pop();
        flag_total
    }
}

impl Checker<'_> {
    /// Whether a method call resolved to `res` and named `name` is a read built on `level`: the
    /// stdlib's own `known`, `known_or`, `is_known` or `high`, or `level` itself (§3.3.4, §22.1.1).
    /// A project's own method of one of those names is not (§3.3.7).
    pub(super) fn is_flag_read(&self, res: &Resolution, name: &str) -> bool {
        match res {
            Resolution::Decl(f) => matches!(self.stdlib_builtin(*f), Some(Builtin::Known | Builtin::KnownOr | Builtin::IsKnown | Builtin::High)),
            // A receiver the POM could not type (a parameter, a local) leaves the method unresolved:
            // the checker reads it by name, as `method_call` does, unless the project defines one.
            _ => flag_builtin(name).is_some() && !self.project_methods.contains(name),
        }
    }

    /// Whether `function`'s parameter `index` is flag-total (§3.3.6).
    pub(super) fn is_flag_total(&self, function: DeclId, index: usize) -> bool {
        FlagTotal::new(self).parameter(function, index)
    }

    /// `expr` with its parentheses removed.
    pub(super) fn through_parens(&self, expr: ExprId) -> ExprId {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.through_parens(*inner),
            _ => expr,
        }
    }

    /// Whether `expr` is a plain read of the parameter `param`.
    fn reads_parameter(&self, expr: ExprId, param: DeclId) -> bool {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { return false };
        matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if d == param)
    }
}
