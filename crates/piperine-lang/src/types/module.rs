//! Modules: params, ports and signals as typed nodes, module-body `let`s, and the placement of
//! one module in another (§8.1): node arguments by nature and count, param block by type.

use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, LetData, NatureKind, ParamData, PatKind, PortData, Resolution, SignalData, StructClass, TypeId};

use super::check::{Checker, Context};
use super::unsupported::Unsupported;
use super::{Callee, Ty};

impl Checker<'_> {
    /// A diagnostic of a rule of the specification that carries its own code.
    pub(super) fn coded(&mut self, file: FileId, code: &'static str, span: Span, message: String, label: &str) {
        self.report(file, Diagnostic::new(code, message, span, label));
    }

    /// The type of a port or a signal: a node of a conservative nature, or `P0001` and `None`.
    pub(super) fn node_type(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.design.type_node(ty).clone();
        let file = self.design.decl_data(node.owner).file;
        match self.lower_type(ty)? {
            Ty::Node(nature) => {
                let DeclKind::Nature(data) = &self.design.decl_data(nature).kind else { return None };
                let name = self.design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                match data.kind {
                    NatureKind::Conservative => Some(Ty::Node(nature)),
                    NatureKind::Discrete => {
                        self.unsupported(file, node.span, &format!("discrete nature `{name}`"));
                        None
                    }
                    NatureKind::Resolved => {
                        self.unsupported(file, node.span, &format!("resolved nature `{name}`"));
                        None
                    }
                    NatureKind::SignalFlow => {
                        self.unsupported(file, node.span, &format!("signal-flow nature `{name}`"));
                        None
                    }
                    NatureKind::Invalid => None,
                }
            }
            other => {
                self.unsupported(file, node.span, &format!("value input port of type `{}`", other.name(self.design)));
                None
            }
        }
    }

    /// `dir name: Nature;` — a node. A default, or a value input, is outside the subset.
    pub(super) fn port(&mut self, port: &PortData, file: FileId) {
        self.node_type(port.ty);
        if let Some(default) = port.default {
            let span = self.design.expr_node(default).span;
            self.unsupported(file, span, "port default value");
            self.infer(default);
        }
    }

    /// `name: T = default where cond;` — `T` is `f64` or `bool`; `where` is `bool` (`E0318`).
    pub(super) fn param(&mut self, param: &ParamData) {
        let ty = self.lower_type(param.ty).and_then(|ty| match ty {
            Ty::F64 | Ty::Bool => Some(ty),
            other => {
                let node = self.design.type_node(param.ty).clone();
                let file = self.design.decl_data(node.owner).file;
                self.unsupported(file, node.span, &format!("param of type `{}`", other.name(self.design)));
                None
            }
        });
        if let (Some(ty), Some(default)) = (&ty, param.default) {
            self.within(Context::Const, |this| this.check(default, ty));
        }
        if let Some(cond) = param.where_ {
            self.within(Context::Const, |this| this.check_as(cond, &Ty::Bool, "E0318"));
        }
    }

    /// `signal name: Nature = init;` — a node; an initial value is an `f64` (`E0210`).
    pub(super) fn signal(&mut self, signal: &SignalData) {
        let node = self.node_type(signal.ty);
        if let (Some(_), Some(init)) = (node, signal.init) {
            self.check_as(init, &Ty::F64, "E0210");
        }
    }

    /// `let x = value;` in a module body: the binding takes the type of the value (a placement
    /// gives it the placed module).
    pub(super) fn module_let(&mut self, data: &LetData) {
        let pat = self.design.pat_node(data.pat).clone();
        let file = self.design.decl_data(pat.owner).file;
        let ty = self.infer(data.value);
        let PatKind::Ident { by_ref: false, mutable: false, sub: None, binding, res: Resolution::Unresolved, .. } = &pat.kind else {
            self.unsupported(file, pat.span, Unsupported::pat(&pat.kind));
            return;
        };
        match ty {
            Some(ty) => {
                self.locals.insert(*binding, ty);
            }
            None => {
                self.poisoned.insert(*binding);
            }
        }
    }

    /// `Module { param: value }(nodes)` or `Module(nodes) { param: value }`: the module is
    /// reached; each node argument has the nature of its port, their count is the port count
    /// (`E0312`), each param value the type of its param.
    pub(super) fn placement(&mut self, expr: ExprId, module: DeclId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Call { callee, args, params, .. } = &node.kind else { return None };
        let callee_node = self.design.expr_node(*callee).clone();
        let fields: Vec<FieldInit> = match &callee_node.kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module && params.is_none() => fields.clone(),
            ExprKind::Path(_) if matches!(self.design.decl_data(module).kind, DeclKind::Module(_)) && self.callee_is(*callee, module) => params.clone().unwrap_or_default(),
            _ => {
                if !self.reads_poisoned(*callee) {
                    self.unsupported(file, node.span, "placement through a template value or a function");
                }
                for arg in args {
                    self.infer(arg.value);
                }
                return None;
            }
        };
        if self.design.decl_data(module).generics_span.is_some() {
            self.unsupported(file, node.span, "generic module");
            return None;
        }
        self.reach(module);
        self.callees.insert(expr, Callee::Module(module));
        let ty = Ty::Module(module);
        self.exprs[callee.index()] = Some(ty.clone());
        self.connections(expr, module, args, file);
        self.param_block(&fields);
        Some(ty)
    }

    /// Whether `expr` is a name whose value already raised a diagnostic.
    fn reads_poisoned(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if self.poisoned.contains(&d)),
            _ => false,
        }
    }

    fn callee_is(&self, callee: ExprId, module: DeclId) -> bool {
        match &self.design.expr_node(callee).kind {
            ExprKind::Path(path) => self.design.path_node(*path).segments.last().map(|s| s.res) == Some(Resolution::Decl(module)),
            _ => false,
        }
    }

    /// The node arguments of a placement against the ports of the module.
    fn connections(&mut self, expr: ExprId, module: DeclId, args: &[Arg], file: FileId) {
        let ports: Vec<TypeId> = self.design.decl_data(module).children.iter().filter_map(|c| match &self.design.decl_data(*c).kind {
            DeclKind::Port(p) => Some(p.ty),
            _ => None,
        }).collect();
        let named = args.iter().any(|a| a.name.is_some());
        if named {
            for arg in args.iter().filter(|a| a.name.is_some()) {
                self.unsupported(file, arg.span, "named connection");
            }
        }
        if args.len() != ports.len() {
            let span = self.design.expr_node(expr).span;
            let name = self.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let message = format!("`{name}` has {} port(s), found {} connection(s)", ports.len(), args.len());
            self.coded(file, "E0312", span, message, "wrong number of connections");
        }
        for (i, arg) in args.iter().enumerate() {
            let found = self.infer(arg.value);
            let (Some(found), Some(port)) = (found, ports.get(i)) else { continue };
            if named || args.len() != ports.len() {
                continue;
            }
            let Some(expected) = self.node_type(*port) else { continue };
            self.connect(arg.value, &expected, &found);
        }
    }

    /// One node argument: the nature of its port, or `gnd`.
    fn connect(&mut self, arg: ExprId, port: &Ty, found: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.design.expr_node(arg).span;
        match (port, found) {
            (Ty::Node(p), Ty::Node(f)) if p == f => {}
            (_, Ty::AnyNode) => {}
            (Ty::Node(_), Ty::Node(_)) => {
                let message = format!("expected {}, found {}", port.name(self.design), found.name(self.design));
                self.coded(file, "E0251", span, format!("a node of another nature is connected: {message}"), &message);
            }
            _ => self.mismatch(file, span, port, found),
        }
    }

    /// `{ name: value }` of a placement: each value against the declared type of its param.
    fn param_block(&mut self, fields: &[FieldInit]) {
        for field in fields {
            let param = match field.res {
                Resolution::Decl(d) => match &self.design.decl_data(d).kind {
                    DeclKind::Param(p) => Some(p.ty),
                    _ => None,
                },
                _ => None,
            };
            match param.and_then(|t| self.lower_type(t)) {
                Some(ty @ (Ty::F64 | Ty::Bool)) => self.check(field.value, &ty),
                _ => {
                    self.infer(field.value);
                }
            }
        }
    }
}
