//! Analog forms: probes (`v(a, b)`, `potential`), contributions (`+>`) and the core primitives
//! of the subset (`ddt`, `limit`, `ac_stim` and the environment functions of §14.4).

use crate::pom::{Arg, BuiltinId, BuiltinKind, DeclKind, DeferReason, ExprId, ExprKind, QuantityRole, Resolution};

use super::catalogue::{Allowed, Core};
use super::check::{Checker, Context};
use super::{Callee, Ty};

impl Checker<'_> {
    /// The statement `lhs +> rhs;`: `lhs` is a probe of a branch (`E0410`), `rhs` is an `f64`
    /// (`E0417`).
    pub(super) fn contribution(&mut self, file: crate::pom::FileId, span: crate::msl::Span, lhs: ExprId, rhs: ExprId) {
        if self.context != Context::Analog {
            self.unsupported(file, span, "contribution outside an `analog` block");
            return;
        }
        let target = self.design.expr_node(lhs).clone();
        let is_probe = matches!(&target.kind, ExprKind::Call { class: crate::pom::CallClass::Probe, .. });
        if is_probe {
            self.exprs[lhs.index()] = self.probe(lhs, true);
        } else {
            let message = "the left side of `+>` is not a potential or flow probe of a branch".to_string();
            self.coded(file, "E0410", target.span, message, "not a probe");
        }
        self.check_as(rhs, &Ty::F64, "E0417");
    }

    /// A probe `v(a)`, `v(a, b)`, `potential(a, b)` or `flow(a, b)`: its nodes have one nature
    /// (`E0240`). Read as a value only a potential is in the subset; a flow probe is a target.
    pub(super) fn probe(&mut self, expr: ExprId, target: bool) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
        let ExprKind::Path(path) = &self.design.expr_node(*callee).kind else { return None };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let (role, nature) = match res {
            Resolution::Decl(q) => match &self.design.decl_data(q).kind {
                DeclKind::Quantity(data) => (data.role, self.design.decl_data(q).parent),
                _ => {
                    self.unsupported(file, node.span, "probe through this name");
                    return None;
                }
            },
            Resolution::Builtin(id) if id.info().name == "flow" => (QuantityRole::Flow, None),
            Resolution::Builtin(_) => (QuantityRole::Potential, None),
            Resolution::Deferred(DeferReason::AmbiguousAccess) => {
                self.unsupported(file, node.span, "probe through an access that names several natures");
                return None;
            }
            Resolution::Deferred(_) | Resolution::Unresolved => {
                self.unsupported(file, node.span, "probe through an unresolved name");
                return None;
            }
        };
        if matches!(self.context, Context::Function | Context::Const) {
            self.coded(file, "E0450", node.span, "a probe is allowed in an `analog` block and a module body only".to_string(), "core primitive out of place");
        }
        if args.is_empty() || args.len() > 2 {
            let message = format!("a probe takes 1 or 2 nodes, found {}", args.len());
            self.report(file, crate::msl::Diagnostic::new("P0002", format!("mismatched types: {message}"), node.span, message));
            return None;
        }
        let mut natures = Vec::new();
        for arg in args {
            match self.infer(arg.value) {
                Some(Ty::Node(n)) => natures.push(n),
                Some(Ty::AnyNode) => {}
                Some(other) => {
                    let span = self.design.expr_node(arg.value).span;
                    self.mismatch_text(file, span, "a node", &other.name(self.design));
                }
                None => {}
            }
        }
        if natures.windows(2).any(|w| w[0] != w[1]) || nature.is_some_and(|q| natures.iter().any(|n| *n != q)) {
            self.coded(file, "E0240", node.span, "the nodes of a probe have different natures".to_string(), "nodes of two natures");
            return None;
        }
        match (role, target) {
            (QuantityRole::Potential, _) | (QuantityRole::Flow, true) => Some(Ty::F64),
            (QuantityRole::Flow, false) => {
                self.unsupported(file, node.span, "flow probe read as a value");
                None
            }
        }
    }

    /// A call of a core primitive: arguments against its row of the table, `f64` as the result.
    /// A primitive with no row is `P0001`; one the table does not allow where it is written is `E0450`, except the
    /// core modules `ddt` and `limit`, whose misplacement is `E0350` (`fn`, closure) or `E0352`
    /// (const context).
    pub(super) fn core_call(&mut self, expr: ExprId, id: BuiltinId, args: &[Arg]) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let Some(sig) = Core::signature(id) else {
            let what = if matches!(id.info().kind, BuiltinKind::Fn | BuiltinKind::Module | BuiltinKind::Statement | BuiltinKind::Event) { "core primitive" } else { "built-in" };
            self.unsupported(file, node.span, &format!("{what} `{}`", id.name()));
            return None;
        };
        let name = id.name();
        match (sig.allowed, self.context) {
            (Allowed::Everywhere, Context::Const) => {
                let message = format!("`{name}` is an environment function and is not allowed in a const context");
                self.coded(file, "E0323", node.span, message, "environment function in a const context");
            }
            (Allowed::Everywhere, _) | (Allowed::Instances, Context::Analog | Context::Module) | (Allowed::Analog, Context::Analog) => {}
            // §14 errors (§8.3) and §8.3.4: `ddt` and `limit` are core modules, so a placement in
            // a `fn` or a closure is `E0350` and one in a const context is `E0352`.
            (Allowed::Instances, Context::Function) => {
                let message = format!("`{name}` is a core module and cannot be placed in a `fn` or a closure");
                self.coded(file, "E0350", node.span, message, "placement in a `fn` or a closure");
            }
            (Allowed::Instances, Context::Const) => {
                let message = format!("`{name}` is a core module and cannot be placed in a const context");
                self.coded(file, "E0352", node.span, message, "placement in a const context");
            }
            (_, context) => {
                let place = match context {
                    Context::Function => "a `fn` or a closure",
                    Context::Const => "a param default, a `where` or a `const`",
                    Context::Module => "a module body",
                    Context::Analog => "an `analog` block",
                };
                self.coded(file, "E0450", node.span, format!("`{name}` is not allowed in {place}"), "core primitive out of place");
            }
        }
        self.callees.insert(expr, Callee::Core(id));
        if let Some(named) = args.iter().find(|a| a.name.is_some()) {
            self.unsupported(file, named.span, "named argument of a core primitive");
            return None;
        }
        let (min, max) = (sig.required.len(), sig.required.len() + sig.optional.len());
        if args.len() < min || args.len() > max {
            let message = format!("`{}` takes {min} to {max} argument(s), found {}", id.name(), args.len());
            self.report(file, crate::msl::Diagnostic::new("P0002", format!("mismatched types: {message}"), node.span, message));
            return None;
        }
        for (arg, param) in args.iter().zip(sig.required.iter().chain(sig.optional)) {
            self.check_argument(arg.value, &param.ty());
        }
        Some(Ty::F64)
    }
}
