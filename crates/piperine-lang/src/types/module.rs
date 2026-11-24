//! Modules: params, ports and signals as typed nodes, module-body `let`s, and the placement of
//! one module in another (§8.1): node arguments by nature and count, param block by type.

use std::collections::BTreeSet;

use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, LetData, ParamData, PatKind, PortData, Resolution, SignalData, StructClass, TypeId};

use super::check::{Checker, Context};
use super::coverage::ElementCoverage;
use super::derives::DerivedTrait;
use super::nature_rules::NatureRules;
use super::unsupported::Unsupported;
use super::{Callee, Substitution, Ty};

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
        if let Some(carrier) = ty.as_ref().filter(|t| matches!(t, Ty::Carrier { .. }) || (matches!(t, Ty::Array(..)) && t.is_carrier_element())) {
            self.carriers.insert(decl, carrier.clone());
        }
        let span = self.design.type_node(port.ty).span;
        // an array of carriers is directed as its element is (CV-23): the direction is the array's
        let kind = ty.as_ref().map(|t| t.innermost().clone());
        match (&kind, port.dir) {
            (Some(Ty::Carrier { nature, .. }), None) if NatureRules::of(self.design, *nature).one_driver => {
                self.coded(file, "E0310", span, "a port of a discrete nature takes `in` or `out` (§8.1.5)".to_string(), "no direction");
            }
            (Some(Ty::Carrier { nature, .. }), Some(_)) if NatureRules::of(self.design, *nature).resolved => {
                self.coded(file, "E0310", span, "a port of a resolved nature takes no direction: its drives are combined by `resolve` (§8.1.5)".to_string(), "unexpected direction");
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
            // an array of carriers takes an array of values: `[x; K]` or `[a, b, c]` (§5.1.8)
            (Some(array @ Ty::Array(..)), Some(crate::msl::ast::Dir::Out | crate::msl::ast::Dir::In)) if array.is_carrier_element() && matches!(array.innermost(), Ty::Carrier { .. }) => {
                self.initial_reads(default);
                let value = array.read_value();
                self.within(Context::Const, |this| this.check_as(default, &value, "E0210"));
            }
            // the initial value of a resolved port is the module's first drive of it, of the driven type (§5.2.3)
            (Some(Ty::Carrier { nature, .. }), None) if NatureRules::of(self.design, *nature).resolved => {
                self.initial_reads(default);
                self.resolved_initial(decl, *nature, default);
            }
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

    /// `name: T = default where cond;` — `T` is `f64`, `bool`, an integer, a unit enum, an `Option` (PI-01 to PI-03), a module (its value a template
    /// `Name { params }`, PI-04) or a struct, tuple or array of plain data (CV-18); `where` is `bool` (`E0318`).
    pub(super) fn param(&mut self, decl: DeclId, param: &ParamData) {
        let ty = self.lower_type(param.ty).and_then(|ty| match ty {
            Ty::F64 | Ty::Bool | Ty::Module(_) => Some(ty),
            // An integer, enum, `Option` or composite param (PI-01 to PI-03, CV-18).
            plain if self.is_param_type(&plain) => Some(plain),
            // a param of a type parameter (`reset: T`, §8.1.15): typed under the parameter's bounds
            Ty::Param(parameter) if self.is_value_type_parameter(parameter) => Some(Ty::Param(parameter)),
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
        if let Some(carrier) = ty.as_ref().filter(|t| matches!(t, Ty::Carrier { .. }) || (matches!(t, Ty::Array(..)) && t.is_carrier_element())) {
            let held = self.generated_type(decl, carrier.clone());
            self.carriers.insert(decl, held);
        }
        // a generated node is an array of nodes, one level for each generation `for` around it (§8.2.23)
        if let Some(node @ Ty::Node(_)) = ty.as_ref()
            && !self.generation_loops(decl).is_empty()
        {
            let held = self.generated_type(decl, node.clone());
            self.carriers.insert(decl, held);
        }
        let (Some(ty), Some(init)) = (ty, signal.init) else { return };
        self.initial_reads(init);
        if let Ty::Carrier { nature, .. } = &ty
            && NatureRules::of(self.design, *nature).resolved
        {
            return self.resolved_initial(decl, *nature, init);
        }
        let value = match &ty {
            Ty::Carrier { value, .. } => (**value).clone(),
            // an array of carriers: the array of their values (`[0; 8]`); an array of nodes: an array of `f64`
            array @ Ty::Array(..) if matches!(array.innermost(), Ty::Carrier { .. }) => array.read_value(),
            Ty::Array(_, length) => Ty::Array(Box::new(Ty::F64), length.clone()),
            _ => Ty::F64,
        };
        self.within(Context::Const, |this| this.check_as(init, &value, "E0210"));
    }

    /// `let x = value;` in a module body: the binding takes the type of the value (a placement
    /// gives it the placed module).
    pub(super) fn module_let(&mut self, decl: DeclId, data: &LetData) {
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
                // inside a generation `for` the binding is the array of its element (§8.2.23)
                let held = self.generated_type(decl, ty);
                self.locals.insert(*binding, held);
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
        // pushed before the arguments are typed: a `let` that feeds back through its own placement is judged by §8.3.5
        self.placing.push(Some(module));
        // recorded before the arguments are typed, which infer the generic arguments: a feedback through the placement reads it
        self.callees.insert(expr, Callee::Module(module));
        self.exprs[callee.index()] = Some(Ty::Module(module));
        let substitution = if self.design.decl_data(module).generics_span.is_some() {
            let Some(given) = self.generic_placement(expr, *callee, module, args) else {
                self.placing.pop();
                self.keep_handed_after_failure(module, args);
                return None;
            };
            Some(given)
        } else {
            None
        };
        self.reach(module);
        let outer = std::mem::replace(&mut self.placement_substitution, substitution.clone());
        self.connections(expr, module, args, file);
        self.placement_substitution = outer;
        self.placing.pop();
        self.param_block(&fields, substitution.as_ref());
        let placed = self.placement_type(module)?;
        Some(substitution.map_or(placed.clone(), |substitution| substitution.apply(&placed)))
    }

    /// The placement of `module` failed its generic arguments and typed no connection: the carriers it was to take at an `out`
    /// port stay handed over, so the failure is reported once, at the placement, and not again as a carrier nothing drives
    /// (§5.2.6; the placement's own diagnostic is the cause).
    fn keep_handed_after_failure(&mut self, module: DeclId, args: &[Arg]) {
        let outs: Vec<Option<crate::msl::ast::Dir>> = self.design.decl_data(module).children.iter().filter_map(|c| match &self.design.decl_data(*c).kind {
            DeclKind::Port(p) => Some(p.dir),
            _ => None,
        }).collect();
        let bound = crate::pom::PortArguments::bind(self.design, module, args);
        for (index, dir) in outs.into_iter().enumerate() {
            let Some(arg) = bound.at(index) else { continue };
            if dir != Some(crate::msl::ast::Dir::Out) {
                continue;
            }
            if let Some((decl, Ty::Carrier { .. })) = self.declared_port(arg.value) {
                let (file, span) = (self.file_of_expr(arg.value), self.design.expr_node(arg.value).span);
                // a placement is typed once per path that reaches it: it hands the carrier over once
                if !self.book.handed.iter().any(|(held, _, at, _)| *held == decl && *at == span) {
                    self.book.handed.push((decl, super::element_target::Element::Whole, span, file));
                }
            }
        }
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
        // a bundle port has no type of its own: its fields are the leaves it stands for (`bundle.rs`)
        let kinds: Vec<Option<Ty>> = ports.iter().map(|(decl, ty, _)| if self.design.is_bundle_owner(*decl) { None } else { self.port_type_under_placement(*ty) }).collect();
        let call_rule = matches!(&self.design.decl_data(module).kind, DeclKind::Module(m) if m.ret.is_some()) || ports.iter().any(|(d, _, _)| self.is_value_input(*d));
        let optional = if call_rule { kinds.iter().rev().take_while(|k| matches!(k, Some(Ty::Option(_)))).count() } else { 0 };
        let (min, max) = (ports.len() - optional, ports.len());
        // a named connection that is no trailing `Option` input is `E0140` from the POM (`classify.rs`), before this check
        let positional = args.iter().filter(|arg| arg.name.is_none()).count();
        if positional < min || args.len() > max {
            let span = self.design.expr_node(expr).span;
            let name = self.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            if call_rule {
                let message = if min == max { format!("`{name}` takes {max} argument(s), found {}", args.len()) } else { format!("`{name}` takes {min} to {max} arguments, found {}", args.len()) };
                self.coded(file, "E0141", span, message, "wrong number of arguments");
            } else {
                let message = format!("`{name}` has {} port(s), found {} connection(s)", ports.len(), args.len());
                self.coded(file, "E0312", span, message, "wrong number of connections");
            }
            for arg in args {
                self.infer(arg.value);
            }
            return;
        }
        let bound = crate::pom::PortArguments::bind(self.design, module, args);
        for (index, kind) in kinds.iter().cloned().enumerate() {
            let Some(arg) = bound.at(index) else { continue };
            let (port_decl, _, dir) = ports[index];
            if self.design.is_bundle_owner(port_decl) {
                self.connect_bundle(arg.value, port_decl);
                continue;
            }
            match kind {
                Some(carrier @ Ty::Carrier { .. }) => self.connect_carrier(arg.value, dir, &carrier),
                Some(array @ Ty::Array(..)) if array.is_carrier_element() => self.connect_array(arg.value, dir, &array),
                Some(expected @ Ty::Node(_)) => {
                    let Some(found) = self.infer(arg.value) else { continue };
                    self.connect(arg.value, port_decl, &expected, &found);
                }
                // a nature parameter of the enclosing generic module, passed on to a placement: a node of that nature, or `gnd`
                Some(expected) if self.is_nature_parameter(&expected) => {
                    let Some(found) = self.infer(arg.value) else { continue };
                    if found != expected && found != Ty::AnyNode {
                        let span = self.design.expr_node(arg.value).span;
                        self.mismatch(file, span, &expected, &found);
                    }
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
                    self.hand_over(*decl, arg, span, file);
                }
            }
            // an array of carriers where one carrier goes: the shape differs (§8.1.10)
            (Some((_, held @ Ty::Array(..))), _) if held.is_carrier_element() => self.shape_mismatch(file, span, &held, port),
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
    pub(super) fn hand_over(&mut self, decl: DeclId, arg: ExprId, span: Span, file: FileId) {
        if matches!(&self.design.decl_data(decl).kind, DeclKind::Port(p) if p.dir == Some(crate::msl::ast::Dir::In)) {
            let message = "an `in` port of the parent is connected to an `out` port: the parent may not hand over what it does not drive (§8.1.11)".to_string();
            self.coded(file, "E0314", span, message, "an `in` port at an `out` port");
            return;
        }
        let named = self.element_named_by(arg);
        let element = self.judged_element(arg, decl, named);
        self.book.handed.push((decl, element, span, file));
    }

    /// [`Checker::hand_over`] of the elements `element` of a carrier no expression names: a field of a bundle given whole
    /// (`s.valid`), or of an element of an array of bundles (`lanes[k]` hands the element `k` of the leaf `lanes.a`). `place` is
    /// the expression that names the bundle: one written inside generation reaches the carrier in the iterations elaboration
    /// takes only.
    pub(super) fn hand_over_element(&mut self, decl: DeclId, element: super::element_target::Element, place: ExprId, span: Span, file: FileId) {
        if matches!(&self.design.decl_data(decl).kind, DeclKind::Port(p) if p.dir == Some(crate::msl::ast::Dir::In)) {
            let message = "an `in` port of the parent is connected to an `out` port: the parent may not hand over what it does not drive (§8.1.11)".to_string();
            self.coded(file, "E0314", span, message, "an `in` port at an `out` port");
            return;
        }
        let element = self.judged_element(place, decl, element);
        self.book.handed.push((decl, element, span, file));
    }

    /// One argument at an array port (§8.1.10): an array of carriers (or of nodes) of the port's shape, or a part of
    /// a nested one that has it. Another length or shape is `E0313` naming both; another element type `E0251`.
    fn connect_array(&mut self, arg: ExprId, dir: Option<crate::msl::ast::Dir>, port: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.design.expr_node(arg).span;
        let carrier = self.declared_port(arg);
        let found = self.infer(arg);
        match carrier {
            Some((decl, held)) if held.is_carrier_element() => {
                if !self.same_shape(&held, port) {
                    self.shape_mismatch(file, span, &held, port);
                } else if !self.same_elements(&held, port) {
                    let message = format!("an array of `{}` is connected to a port of `{}`", held.name(self.design), port.name(self.design));
                    self.coded(file, "E0251", span, message, "array of another type");
                } else if dir == Some(crate::msl::ast::Dir::Out) {
                    self.hand_over(decl, arg, span, file);
                }
            }
            Some((_, held)) => self.shape_mismatch(file, span, &held, port),
            None if dir == Some(crate::msl::ast::Dir::Out) => {
                let message = "an expression is connected to an `out` port: it takes only a carrier the parent may hand to one driver (§8.1.11)".to_string();
                self.coded(file, "E0314", span, message, "expression at an `out` port");
            }
            None => {
                if let Some(found) = found.filter(|f| f.is_carrier_element()) {
                    self.shape_mismatch(file, span, &found, port);
                } else {
                    self.check_as(arg, &port.read_value(), "E0337");
                }
            }
        }
    }

    /// Whether `found` and `port` are arrays of the same lengths at every level (a generic length agrees until the
    /// item is instantiated).
    fn same_shape(&self, found: &Ty, port: &Ty) -> bool {
        match (found, port) {
            (Ty::Array(found, found_length), Ty::Array(wanted, wanted_length)) => found_length.fits(wanted_length) && self.same_shape(found, wanted),
            (Ty::Array(..), _) | (_, Ty::Array(..)) => false,
            _ => true,
        }
    }

    /// Whether the innermost elements of two arrays of one shape are the same carrier or compatible nodes.
    fn same_elements(&self, found: &Ty, port: &Ty) -> bool {
        match (found.innermost(), port.innermost()) {
            (Ty::Node(f), Ty::Node(p)) => NatureRules::compatible(self.design, *p, *f),
            (found, port) => found == port,
        }
    }

    /// `E0313`: a connection of another array shape, naming both.
    fn shape_mismatch(&mut self, file: FileId, span: Span, found: &Ty, port: &Ty) {
        let message = format!("`{}` is connected to a port of `{}`: the lengths or the shape differ (§8.1.10)", found.name(self.design), port.name(self.design));
        self.coded(file, "E0313", span, message, "another array shape");
    }

    /// A carrier handed to an `out` port has one drive, the instance's: a second hand-over, or
    /// a definition or a commit of this module, is `E0220`. An initial value is the value of the elements no `out` port
    /// holds (§5.2.6, §8.2.21), so it is `E0223` only once every element is handed (`handed_initial_values`).
    pub(super) fn handed_drives(&mut self) {
        let handed = std::mem::take(&mut self.book.handed);
        let mut given: Vec<(DeclId, super::element_target::Element)> = Vec::new();
        let mut clean: Vec<(DeclId, super::element_target::Element, Span, FileId)> = Vec::new();
        for (decl, element, span, file) in handed {
            // the element as the source names it: `mem[2]`, `lanes[1].a` for a field of an array of bundles
            let name = self.design.element_naming(decl).name(&element.subscript());
            // a resolved carrier has as many drives as modules hand it down or define it (§5.2.1): nothing here is a second one
            if matches!(self.carriers.get(&decl).map(Ty::innermost), Some(Ty::Carrier { nature, .. }) if NatureRules::of(self.design, *nature).resolved) {
                continue;
            }
            let twice = given.iter().any(|(d, e)| *d == decl && e.overlaps(element));
            given.push((decl, element));
            // an element only the elaborator folds is judged there against the whole-carrier drives too (`elab/drive_overlaps.rs`)
            let whole_drive = element != super::element_target::Element::Unfolded && (self.book.defined.contains_key(&decl) || self.book.committed.contains_key(&decl));
            let driven_here = whole_drive || self.book.elements.iter().any(|(c, e, _, _)| *c == decl && e.overlaps(element));
            let nature = match self.carriers.get(&decl).map(Ty::innermost) {
                Some(Ty::Carrier { nature, .. }) => self.design.decl_data(*nature).name.as_ref().map_or_else(String::new, |n| n.text.clone()),
                _ => String::new(),
            };
            if twice {
                self.coded(file, "E0220", span, format!("`{name}` is handed to a second `out` port: a carrier of `{nature}` has one drive, since `{nature}` has no `impl Resolve` to combine two (§5.2.1)"), "a second drive");
            } else if driven_here {
                self.coded(file, "E0220", span, format!("`{name}` is handed to an `out` port and driven by this module too: a carrier of `{nature}` has one drive, since `{nature}` has no `impl Resolve` to combine two (§5.2.6)"), "a second drive");
            } else {
                clean.push((decl, element, span, file));
            }
        }
        self.handed_initial_values(&clean);
    }

    /// `E0223` for a carrier with an initial value that `out` ports hold every element of: the initial value would apply
    /// to no element, the instance holds the drive (§5.2.6). With an element no `out` port holds, the initial value is
    /// that element's. A length not known yet leaves the judgement to the elaborator of each instance.
    fn handed_initial_values(&mut self, handed: &[(DeclId, super::element_target::Element, Span, FileId)]) {
        let mut judged = BTreeSet::new();
        for (decl, _, span, file) in handed {
            if !judged.insert(*decl) || !self.has_initial(*decl) {
                continue;
            }
            let Some(held) = self.carriers.get(decl).cloned() else { continue };
            let mut coverage = ElementCoverage::of(&held);
            for (_, element, _, _) in handed.iter().filter(|(carrier, ..)| carrier == decl) {
                coverage.cover(*element);
            }
            if coverage.is_deferred() {
                self.deferred_coverage.handed_initial.insert(*decl);
            } else if coverage.is_complete() {
                let name = self.design.decl_data(*decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.coded(*file, "E0223", *span, format!("`{name}` has an initial value and every element is handed to an `out` port: the instance holds the drive, so the declaring module may give none (§5.2.6)"), "initial value without the drive");
            }
        }
    }

    /// Every carrier of this module that has a value from `(0, 0)` on (§3.3.4): a `signal` or an `out` port of a
    /// one-driver nature with no initial value needs a value type that implements `Default`, else `E0187` at the
    /// declaration naming the carrier and `Default` (once per declaration). The one driver may specify the value
    /// instead: an undelayed definition of the carrier, or an `out` port of a child instance it is handed to
    /// (§3.3.4, EU-04 AC 7); a delayed definition does not. An array of carriers is judged per element: the message
    /// names the elements no driver names (`coverage.rs`). An `in` port is driven by its caller.
    pub(super) fn missing_initial_values(&mut self, children: &[DeclId]) {
        for decl in children.iter().copied() {
            let item = self.design.decl_data(decl).clone();
            let is_carrier = match &item.kind {
                DeclKind::Signal(_) => true,
                DeclKind::Port(port) => port.dir == Some(crate::msl::ast::Dir::Out),
                _ => false,
            };
            let Some(held) = self.carriers.get(&decl).cloned() else { continue };
            // an array of carriers has the rule of its elements: every element starts at the `Default` of the carried type
            let Some(Ty::Carrier { nature, value }) = (match held.innermost() {
                carrier @ Ty::Carrier { .. } => Some(carrier.clone()),
                _ => None,
            }) else {
                continue;
            };
            if NatureRules::of(self.design, nature).resolved {
                self.missing_resolved_initial_value(decl);
                continue;
            }
            // an array with no element holds nothing to start
            if held.has_fixed_zero_length() {
                continue;
            }
            // a carrier with no initial value starts at its `Default`: a hand-written one is a function the elaborator folds
            if !self.has_initial(decl) {
                self.reach_written_defaults(&value);
            }
            if !is_carrier || self.has_initial(decl) || !NatureRules::of(self.design, nature).one_driver || self.implements(&value, DerivedTrait::Default) {
                continue;
            }
            // each element needs its own driver: the elements no driver names have neither value nor initial value (§3.3.4, §8.2.20)
            let name = item.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let coverage = self.driven_coverage(decl, &held);
            if coverage.is_deferred() {
                self.deferred_coverage.missing_initial.insert(decl);
                continue;
            }
            let Some(unnamed) = coverage.unnamed(&self.design.element_naming(decl)) else { continue };
            let found = value.name(self.design);
            let message = format!("`{unnamed}` has no initial value and its type `{found}` does not implement `Default`: a carrier has a value from (0, 0) on (§3.3.4)");
            self.report(item.file, Diagnostic::new("E0187", message, item.span, "no initial value").with_help(format!("write `= value` after the type of `{name}`, or carry an `Option<{found}>`")));
        }
    }

    /// A template `Name { params }` where a value of module type is expected (PI-04): its params
    /// are checked like a placement's block, and its type is the module it names, so a template
    /// of another module is a mismatch (`E0180`) naming both.
    pub(super) fn template_value(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit]) -> Option<Ty> {
        self.param_block(fields, None);
        self.exprs[expr.index()] = Some(Ty::Module(module));
        Some(Ty::Module(module))
    }

    /// `{ name: value }` of a placement: each value against the declared type of its param.
    pub(super) fn param_block(&mut self, fields: &[FieldInit], substitution: Option<&Substitution>) {
        for field in fields {
            self.no_placement_in(field.value);
            let param = match field.res {
                Resolution::Decl(d) => match &self.design.decl_data(d).kind {
                    DeclKind::Param(p) => Some(p.ty),
                    _ => None,
                },
                _ => None,
            };
            // a param of a generic module's type `T` takes the type the placement gave `T`
            let declared = param.and_then(|t| self.lower_type(t)).map(|ty| substitution.map_or(ty.clone(), |given| given.apply(&ty)));
            match declared {
                Some(ty @ (Ty::F64 | Ty::Bool | Ty::Module(_))) => self.check(field.value, &ty),
                // a value of `T` bound to an `Option<T>` param is `Some` of it (§3.2.3, PI-03 AC 2)
                Some(Ty::Option(inner)) => {
                    let option = Ty::Option(inner.clone());
                    self.option_argument(field.value, &option, &inner)
                }
                Some(plain) if self.is_param_type(&plain) => self.check(field.value, &plain),
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

impl Checker<'_> {
    /// Brings every hand-written `impl Default` the default of `ty` reads into the check: the `default` function of a struct
    /// that writes one, and of the structs, tuples and arrays holding such a struct.
    pub(super) fn reach_written_defaults(&mut self, ty: &Ty) {
        match ty {
            Ty::Struct(nominal) => {
                let table = std::rc::Rc::clone(&self.impls);
                if let Some(function) = table.builtin_functions("Default", "default", ty).first() {
                    self.reach(*function);
                } else if self.derives(nominal.decl, "Default") {
                    let substitution = self.nominal_substitution(nominal);
                    let fields: Vec<Ty> = self.fields_of(nominal.decl).unwrap_or_default().into_iter().map(|field| self.substitute(&substitution, &field.ty)).collect();
                    fields.iter().for_each(|field| self.reach_written_defaults(field));
                }
            }
            Ty::Tuple(items) => items.iter().for_each(|item| self.reach_written_defaults(item)),
            Ty::Array(element, _) => self.reach_written_defaults(element),
            _ => {}
        }
    }
}
