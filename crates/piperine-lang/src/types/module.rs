//! Modules: params, ports and signals as typed nodes, module-body `let`s, and the placement of
//! one module in another (§8.1): node arguments by nature and count, param block by type.

use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, LetData, ParamData, PatKind, PortData, Resolution, SignalData, StructClass, TypeId};

use super::check::{Checker, Context};
use super::unsupported::Unsupported;
use super::{Callee, Ty};

impl Checker<'_> {
    /// A diagnostic of a rule of the specification that carries its own code.
    pub(super) fn coded(&mut self, file: FileId, code: &'static str, span: Span, message: String, label: &str) {
        self.report(file, Diagnostic::new(code, message, span, label));
    }

    /// `dir name: Nature;` — a node, or a carrier of `Bit<T>` that takes a direction (`E0310`).
    /// A default is the first commit of an `out` carrier (`E0210`); on any other port it is
    /// outside the subset.
    pub(super) fn port(&mut self, decl: DeclId, port: &PortData, file: FileId) {
        let ty = self.port_type(port.ty);
        if let Some(carrier @ Ty::Carrier { .. }) = &ty {
            self.carriers.insert(decl, carrier.clone());
        }
        let span = self.design.type_node(port.ty).span;
        match (&ty, port.dir) {
            (Some(Ty::Carrier { .. }), None) => {
                self.coded(file, "E0310", span, "a port of a discrete nature takes `in` or `out` (§8.1.5)".to_string(), "no direction");
            }
            (Some(Ty::Node(_)), Some(_)) => {
                self.coded(file, "E0310", span, "a port of a continuous nature takes no direction (§8.1.5)".to_string(), "unexpected direction");
            }
            _ => {}
        }
        let Some(default) = port.default else { return };
        match (&ty, port.dir) {
            (Some(Ty::Carrier { value, .. }), Some(crate::msl::ast::Dir::Out)) => {
                self.initial_reads(default);
                self.within(Context::Const, |this| this.check_as(default, value, "E0210"));
            }
            _ => {
                let span = self.design.expr_node(default).span;
                self.unsupported(file, span, "port default value");
                self.infer(default);
            }
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

    /// `signal name: Nature = init;` — a node whose initial value is an `f64`, or a carrier of
    /// `Bit<T>` whose initial value is a `T`; either is `E0210` when the type differs or the
    /// value reads a carrier or a probe (§5.1.5).
    pub(super) fn signal(&mut self, decl: DeclId, signal: &SignalData) {
        let ty = self.port_type(signal.ty);
        if let Some(carrier @ Ty::Carrier { .. }) = &ty {
            self.carriers.insert(decl, carrier.clone());
        }
        let (Some(ty), Some(init)) = (ty, signal.init) else { return };
        self.initial_reads(init);
        let value = match &ty {
            Ty::Carrier { value, .. } => (**value).clone(),
            _ => Ty::F64,
        };
        self.within(Context::Const, |this| this.check_as(init, &value, "E0210"));
    }

    /// `let x = value;` in a module body: the binding takes the type of the value (a placement
    /// gives it the placed module).
    pub(super) fn module_let(&mut self, data: &LetData) {
        let pat = self.design.pat_node(data.pat).clone();
        let file = self.design.decl_data(pat.owner).file;
        if matches!(pat.kind, PatKind::Tuple(_)) {
            self.tuple_let(file, &pat, data.value);
            return;
        }
        let ty = self.infer_event_position(data.value);
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
        // §8.3.4: a function, a closure and a reaction body place no instance.
        if matches!(self.context, Context::Function | Context::Reaction) {
            let place = if self.context == Context::Reaction { "a reaction body" } else { "a `fn` or a closure" };
            let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            self.coded(file, "E0350", node.span, format!("`{name}` is placed in {place}: it places no instance (§8.3.4)"), "placement out of place");
            for arg in args {
                self.infer(arg.value);
            }
            return None;
        }
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

    /// The arguments of a placement against the ports of the module: a node by nature, a
    /// carrier or a discrete expression at an `in` port, a carrier the parent may hand over at
    /// an `out` port. Their count is the port count (`E0312`).
    fn connections(&mut self, expr: ExprId, module: DeclId, args: &[Arg], file: FileId) {
        let ports: Vec<(TypeId, Option<crate::msl::ast::Dir>)> = self.design.decl_data(module).children.iter().filter_map(|c| match &self.design.decl_data(*c).kind {
            DeclKind::Port(p) => Some((p.ty, p.dir)),
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
            let port = ports.get(i).copied().filter(|_| !named && args.len() == ports.len());
            let Some((port_ty, dir)) = port else {
                self.infer(arg.value);
                continue;
            };
            match self.port_type(port_ty) {
                Some(Ty::Carrier { value, .. }) => self.connect_carrier(arg.value, dir, &value),
                Some(expected) => {
                    let Some(found) = self.infer(arg.value) else { continue };
                    self.connect(arg.value, &expected, &found);
                }
                None => {
                    self.infer(arg.value);
                }
            }
        }
    }

    /// One node argument: the nature of its port, or `gnd`.
    fn connect(&mut self, arg: ExprId, port: &Ty, found: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.design.expr_node(arg).span;
        // A discrete carrier read as a value is typed by what it holds; the declaration tells.
        if let Some((_, Ty::Carrier { .. })) = self.declared_port(arg) {
            let message = format!("a discrete carrier is connected to a port of {} (§6.1)", port.name(self.design));
            self.coded(file, "E0250", span, message, "discrete carrier at a continuous port");
            return;
        }
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

    /// One argument at a carrier port holding `value`. At an `in` port a carrier of the same
    /// type or a discrete expression (an anonymous definition, §8.2.9); at an `out` port a
    /// carrier the parent may hand to one driver, and nothing else (§8.1.11).
    fn connect_carrier(&mut self, arg: ExprId, dir: Option<crate::msl::ast::Dir>, value: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.design.expr_node(arg).span;
        let carrier = self.declared_port(arg);
        let found = self.infer(arg);
        match (&carrier, dir) {
            (Some((_, Ty::Node(_))), _) => {
                self.coded(file, "E0250", span, "a node is connected to a port of a discrete nature (§6.1)".to_string(), "node at a discrete port");
            }
            (None, _) if found == Some(Ty::AnyNode) => {
                self.coded(file, "E0250", span, "`gnd` is connected to a port of a discrete nature (§6.1)".to_string(), "`gnd` at a discrete port");
            }
            (Some((decl, Ty::Carrier { value: held, .. })), dir) => {
                if **held != *value {
                    let message = format!("a carrier of `Bit<{}>` is connected to a port of `Bit<{}>`", held.name(self.design), value.name(self.design));
                    self.coded(file, "E0251", span, message, "carrier of another type");
                } else if dir == Some(crate::msl::ast::Dir::Out) {
                    self.hand_over(*decl, span, file);
                }
            }
            (Some(_), _) => {}
            (None, Some(crate::msl::ast::Dir::Out)) => {
                let message = "an expression is connected to an `out` port: it takes only a carrier the parent may hand to one driver (§8.1.11)".to_string();
                self.coded(file, "E0314", span, message, "expression at an `out` port");
            }
            (None, _) => {
                self.check_as(arg, value, "E0337");
                if self.continuous(arg) {
                    let message = "a continuous expression is connected to an `in` port: it reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)` (§8.1.12)".to_string();
                    self.coded(file, "E0316", span, message, "a continuous expression at an `in` port");
                }
            }
        }
    }

    /// The parent hands the carrier `decl` to an `out` port of an instance: a signal of the
    /// parent or an `out` port of the parent (an `in` port is not the parent's to hand over,
    /// `E0314`). The drive is the instance's from here on.
    fn hand_over(&mut self, decl: DeclId, span: Span, file: FileId) {
        if matches!(&self.design.decl_data(decl).kind, DeclKind::Port(p) if p.dir == Some(crate::msl::ast::Dir::In)) {
            let message = "an `in` port of the parent is connected to an `out` port: the parent may not hand over what it does not drive (§8.1.11)".to_string();
            self.coded(file, "E0314", span, message, "an `in` port at an `out` port");
            return;
        }
        self.book.handed.push((decl, span, file));
    }

    /// A carrier handed to an `out` port has one drive, the instance's: a second hand-over, or
    /// a definition or a commit of this module, is `E0220`; an initial value is `E0223`.
    pub(super) fn handed_drives(&mut self) {
        let handed = std::mem::take(&mut self.book.handed);
        let mut seen = std::collections::BTreeSet::new();
        for (decl, span, file) in handed {
            let name = self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            if !seen.insert(decl) {
                self.coded(file, "E0220", span, format!("`{name}` is handed to a second `out` port: a one-driver carrier has one drive (§5.2.1)"), "a second drive");
            } else if self.book.defined.contains_key(&decl) || self.book.committed.contains_key(&decl) {
                self.coded(file, "E0220", span, format!("`{name}` is handed to an `out` port and driven by this module too (§5.2.6)"), "a second drive");
            } else if self.has_initial(decl) {
                self.coded(file, "E0223", span, format!("`{name}` has an initial value and is handed to an `out` port: the instance holds the drive, so the declaring module may give none (§5.2.6)"), "initial value without the drive");
            }
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
