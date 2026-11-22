//! Modules: params, ports and signals as typed nodes, module-body `let`s, and the placement of
//! one module in another (§8.1): node arguments by nature and count, param block by type.

use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, LetData, ParamData, PatKind, PortData, Resolution, SignalData, StructClass, TypeId};

use super::check::{Checker, Context};
use super::derives::DerivedTrait;
use super::nature_rules::NatureRules;
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
            (Some(value), Some(_)) if !matches!(value, Ty::Node(_) | Ty::Carrier { .. }) => {
                self.coded(file, "E0310", span, "a value input takes no direction (§8.1.5)".to_string(), "unexpected direction");
            }
            _ => {}
        }
        let Some(default) = port.default else { return };
        match (&ty, port.dir) {
            (Some(value), _) if !matches!(value, Ty::Node(_) | Ty::Carrier { .. }) => {
                let span = self.design.expr_node(default).span;
                self.coded(file, "E0311", span, "a value input takes no initial value (§8.1.7)".to_string(), "no initial value here");
                self.infer(default);
            }
            // An `in` port of a top level starts at its written value too (§3.3.4, §23.1 rule 9).
            (Some(Ty::Carrier { value, .. }), Some(crate::msl::ast::Dir::Out | crate::msl::ast::Dir::In)) => {
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

    /// `name: T = default where cond;` — `T` is `f64`, `bool`, a module (its value a template
    /// `Name { params }`, PI-04) or a struct, tuple or array of plain data (CV-18); `where` is `bool` (`E0318`).
    pub(super) fn param(&mut self, decl: DeclId, param: &ParamData) {
        let ty = self.lower_type(param.ty).and_then(|ty| match ty {
            Ty::F64 | Ty::Bool | Ty::Module(_) => Some(ty),
            // An `Option` or enum param is `params-and-io`'s (L28).
            composite if self.is_param_composite(&composite) => Some(composite),
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
            self.where_reads_later_param(decl, cond);
        }
    }

    /// A `where` may read its own param and earlier ones only: a read of a param of the same
    /// module declared after `decl` is `E0318` naming it (§8.1.1).
    fn where_reads_later_param(&mut self, decl: DeclId, expr: ExprId) {
        let node = self.design.expr_node(expr).clone();
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(read)) = self.design.path_node(*path).segments.last().map(|s| s.res)
        {
            let (own, other) = (self.design.decl_data(decl), self.design.decl_data(read));
            if matches!(other.kind, DeclKind::Param(_)) && other.parent == own.parent && other.span.start > own.span.start {
                let name = other.name.as_ref().map_or("?", |n| n.text.as_str()).to_string();
                let file = self.file_of_expr(expr);
                let message = format!("a `where` reads the param `{name}`, declared after it (§8.1.1)");
                self.report(file, Diagnostic::new("E0318", message, node.span, format!("`{name}` is declared later")));
            }
        }
        for child in crate::pom::expr_children(&node.kind, self.design) {
            self.where_reads_later_param(decl, child);
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
            self.let_pattern(file, data.pat, None, Some(data.value), None);
            return;
        }
        let PatKind::Ident { sub: None, binding, res: Resolution::Unresolved, .. } = &pat.kind else {
            self.unsupported(file, pat.span, Unsupported::pat(&pat.kind));
            self.infer_event_position(data.value);
            return;
        };
        self.lets_open.insert(*binding, data.value);
        let ty = self.infer_event_position(data.value);
        self.lets_open.remove(binding);
        if let Some(closure) = self.closure_expr(data.value) {
            self.closure_locals.insert(*binding, closure);
        }
        match ty {
            Some(Ty::Str) => {
                self.unsupported(file, pat.span, "a string literal bound by a `let`");
                self.poisoned.insert(*binding);
            }
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
        // §8.3.4: a function, a closure and a reaction body place no instance, nor does a const
        // context.
        if matches!(self.context, Context::Function | Context::Reaction | Context::Const) {
            let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            if self.context == Context::Const {
                self.coded(file, "E0352", node.span, format!("`{name}` is placed in a const context: a result is not fixed during a solve (§8.3.4)"), "placement in a const context");
            } else {
                let place = if self.context == Context::Reaction { "a reaction body" } else { "a `fn` or a closure" };
                self.coded(file, "E0350", node.span, format!("`{name}` is placed in {place}: it places no instance (§8.3.4)"), "placement out of place");
            }
            for arg in args {
                self.infer(arg.value);
            }
            return None;
        }
        let callee_node = self.design.expr_node(*callee).clone();
        let fields: Vec<FieldInit> = match &callee_node.kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module && params.is_none() => fields.clone(),
            ExprKind::Path(_) if matches!(self.design.decl_data(module).kind, DeclKind::Module(_)) && self.callee_is(*callee, module) => params.clone().unwrap_or_default(),
            // `cell(c)` through a param of module type (§8.4.4): the module's ports are connected;
            // the param block belongs to the template, so one on the placement is `E0361`.
            ExprKind::Path(_) if self.callee_is_param_of(*callee, module) => {
                if params.is_some() {
                    let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                    self.coded(file, "E0361", node.span, format!("a placement through a param of module `{name}` takes no param block: the template holds the params (§8.4.4)"), "param block on a placement through a template value");
                }
                Vec::new()
            }
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
        self.exprs[callee.index()] = Some(Ty::Module(module));
        self.placing.push(Some(module));
        self.connections(expr, module, args, file);
        self.placing.pop();
        self.param_block(&fields);
        self.placement_type(module)
    }

    /// Whether `expr` is a name whose value already raised a diagnostic.
    fn reads_poisoned(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if self.poisoned.contains(&d)),
            _ => false,
        }
    }

    /// Whether `callee` names a param whose declared type is the module `module`.
    fn callee_is_param_of(&self, callee: ExprId, module: DeclId) -> bool {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return false };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(d)) => matches!(&self.design.decl_data(d).kind, DeclKind::Param(p) if self.design.type_resolution(p.ty) == Resolution::Decl(module)),
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
    /// an `out` port, an expression of its type for a value input (§3.2). Their count is the port
    /// count (`E0312`), or for a module with value inputs or a result, from the required ones to
    /// all of them, a trailing `Option` input being left out as `None` (`E0141`).
    fn connections(&mut self, expr: ExprId, module: DeclId, args: &[Arg], file: FileId) {
        let ports: Vec<(DeclId, TypeId, Option<crate::msl::ast::Dir>)> = self.design.decl_data(module).children.iter().filter_map(|c| match &self.design.decl_data(*c).kind {
            DeclKind::Port(p) => Some((*c, p.ty, p.dir)),
            _ => None,
        }).collect();
        let named = args.iter().any(|a| a.name.is_some());
        if named {
            for arg in args.iter().filter(|a| a.name.is_some()) {
                self.unsupported(file, arg.span, "named connection");
            }
        }
        let kinds: Vec<Option<Ty>> = ports.iter().map(|(_, ty, _)| self.port_type(*ty)).collect();
        let call_rule = matches!(&self.design.decl_data(module).kind, DeclKind::Module(m) if m.ret.is_some()) || ports.iter().any(|(d, _, _)| self.is_value_input(*d));
        let optional = if call_rule { kinds.iter().rev().take_while(|k| matches!(k, Some(Ty::Option(_)))).count() } else { 0 };
        let (min, max) = (ports.len() - optional, ports.len());
        if args.len() < min || args.len() > max {
            let span = self.design.expr_node(expr).span;
            let name = self.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            if call_rule {
                let message = if min == max { format!("`{name}` takes {max} argument(s), found {}", args.len()) } else { format!("`{name}` takes {min} to {max} arguments, found {}", args.len()) };
                self.coded(file, "E0141", span, message, "wrong number of arguments");
            } else {
                let message = format!("`{name}` has {} port(s), found {} connection(s)", ports.len(), args.len());
                self.coded(file, "E0312", span, message, "wrong number of connections");
            }
        }
        let counted = args.len() >= min && args.len() <= max;
        for (i, arg) in args.iter().enumerate() {
            let port = ports.get(i).copied().zip(kinds.get(i).cloned()).filter(|_| !named && counted);
            let Some(((port_decl, _, dir), kind)) = port else {
                self.infer(arg.value);
                continue;
            };
            match kind {
                Some(carrier @ Ty::Carrier { .. }) => self.connect_carrier(arg.value, dir, &carrier),
                Some(expected @ Ty::Node(_)) => {
                    let Some(found) = self.infer(arg.value) else { continue };
                    self.connect(arg.value, port_decl, &expected, &found);
                }
                // A value input: any expression of its type, a probe included (§8.1.13).
                Some(value) => {
                    self.check_argument(arg.value, &value);
                    self.definition_value_input(expr, module, port_decl, arg.value);
                }
                None => {
                    self.infer(arg.value);
                }
            }
        }
    }

    /// One node argument to the port `port_decl`: the nature of its port, or `gnd`. A value (an
    /// expression that is not a node) at a continuous carrier port is `E0315` naming the port:
    /// only an `in` port takes an expression, and a node port is never one (§8.1.11).
    fn connect(&mut self, arg: ExprId, port_decl: DeclId, port: &Ty, found: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.design.expr_node(arg).span;
        // A discrete carrier read as a value is typed by what it holds; the declaration tells.
        if let Some((_, Ty::Carrier { .. })) = self.declared_port(arg) {
            let message = format!("a discrete carrier is connected to a port of {} (§6.1)", port.name(self.design));
            self.coded(file, "E0250", span, message, "discrete carrier at a continuous port");
            return;
        }
        match (port, found) {
            (Ty::Node(p), Ty::Node(f)) if NatureRules::compatible(self.design, *p, *f) => {}
            (_, Ty::AnyNode) => {}
            (Ty::Node(_), Ty::Node(_)) => {
                let message = format!("expected {}, found {}", port.name(self.design), found.name(self.design));
                self.coded(file, "E0251", span, format!("a node of another nature is connected: {message}"), &message);
            }
            (Ty::Node(_), _) => {
                let name = self.design.decl_data(port_decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                let message = format!("an expression of type {} is connected to the node port `{name}`: only an `in` port of a discrete nature takes an expression (§8.1.11)", found.name(self.design));
                self.coded(file, "E0315", span, message, "expression at a node port");
            }
            _ => self.mismatch(file, span, port, found),
        }
    }

    /// One argument at a carrier port holding `value`. At an `in` port a carrier of the same
    /// type or a discrete expression (an anonymous definition, §8.2.9); at an `out` port a
    /// carrier the parent may hand to one driver, and nothing else (§8.1.11).
    fn connect_carrier(&mut self, arg: ExprId, dir: Option<crate::msl::ast::Dir>, port: &Ty) {
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
            (Some((decl, held @ Ty::Carrier { .. })), dir) => {
                if held != port {
                    let message = format!("a carrier of `{}` is connected to a port of `{}`", held.name(self.design), port.name(self.design));
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
                let Ty::Carrier { value, .. } = port else { return };
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

    /// Every carrier of this module that has a value from `(0, 0)` on (§3.3.4): a `signal` or an `out` port of a
    /// one-driver nature with no initial value needs a value type that implements `Default`, else `E0187` at the
    /// declaration naming the carrier and `Default` (once per declaration). The one driver may specify the value
    /// instead: an undelayed definition of the carrier, or an `out` port of a child instance it is handed to
    /// (§3.3.4, EU-04 AC 7); a delayed definition does not. An `in` port is driven by its caller.
    pub(super) fn missing_initial_values(&mut self, children: &[DeclId]) {
        for decl in children.iter().copied() {
            let item = self.design.decl_data(decl).clone();
            let is_carrier = match &item.kind {
                DeclKind::Signal(_) => true,
                DeclKind::Port(port) => port.dir == Some(crate::msl::ast::Dir::Out),
                _ => false,
            };
            let Some(Ty::Carrier { nature, value }) = self.carriers.get(&decl).cloned() else { continue };
            if !is_carrier || self.has_initial(decl) || !NatureRules::of(self.design, nature).one_driver || self.implements(&value, DerivedTrait::Default) {
                continue;
            }
            let specified_by_the_driver = self.book.undelayed.contains(&decl) || self.book.handed.iter().any(|(carrier, _, _)| *carrier == decl);
            if specified_by_the_driver {
                continue;
            }
            let name = item.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let found = Self::bare_type_name(&value.name(self.design));
            let message = format!("`{name}` has no initial value and its type `{found}` does not implement `Default`: a carrier has a value from (0, 0) on (§3.3.4)");
            self.report(item.file, Diagnostic::new("E0187", message, item.span, "no initial value").with_help(format!("write `= value` after the type of `{name}`, or carry an `Option<{found}>`")));
        }
    }

    /// `name` as a type is written, for a message that puts it between backticks: `enum `Mode`` is `Mode`.
    fn bare_type_name(name: &str) -> String {
        let mut bare = name.to_string();
        while let Some(at) = bare.find("enum `") {
            bare.replace_range(at..at + "enum `".len(), "");
            if let Some(close) = bare[at..].find('`') {
                bare.remove(at + close);
            }
        }
        bare
    }

    /// A template `Name { params }` where a value of module type is expected (PI-04): its params
    /// are checked like a placement's block, and its type is the module it names, so a template
    /// of another module is a mismatch (`E0180`) naming both.
    pub(super) fn template_value(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit]) -> Option<Ty> {
        self.param_block(fields);
        self.exprs[expr.index()] = Some(Ty::Module(module));
        Some(Ty::Module(module))
    }

    /// `{ name: value }` of a placement: each value against the declared type of its param.
    pub(super) fn param_block(&mut self, fields: &[FieldInit]) {
        for field in fields {
            self.no_placement_in(field.value);
            let param = match field.res {
                Resolution::Decl(d) => match &self.design.decl_data(d).kind {
                    DeclKind::Param(p) => Some(p.ty),
                    _ => None,
                },
                _ => None,
            };
            match param.and_then(|t| self.lower_type(t)) {
                Some(ty @ (Ty::F64 | Ty::Bool | Ty::Module(_))) => self.check(field.value, &ty),
                Some(composite) if self.is_param_composite(&composite) => self.check(field.value, &composite),
                _ => {
                    self.infer(field.value);
                }
            }
        }
    }
}

impl Checker<'_> {
    /// A bound param value places no instance (§8.3.4, `E0352`): a result is not fixed during a
    /// solve.
    fn no_placement_in(&mut self, value: ExprId) {
        let mut all = Vec::new();
        crate::pom::preorder(self.design, value, &mut all);
        for expr in all {
            let node = self.design.expr_node(expr).clone();
            if matches!(node.kind, ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. }) {
                let file = self.file_of_expr(expr);
                self.coded(file, "E0352", node.span, "a bound param value places no instance: a result is not fixed during a solve (§8.3.4)".to_string(), "placement in a bound param value");
            }
        }
    }
}
