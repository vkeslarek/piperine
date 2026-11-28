//! Modules: params, ports and signals as typed nodes, module-body `let`s, and the placement of
//! one module in another (§8.1): node arguments by nature and count, param block by type.

use std::collections::BTreeSet;

use crate::refusal::RefusedForm;
use crate::refusal::forms::StructureForm;
use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, Dir, ExprId, ExprKind, FieldInit, FileId, LetData, ParamData, PatKind, PortData, Resolution, SignalData, StructClass, TypeId};

use crate::types::check::Context;
use super::ModuleTyping;
use crate::types::carrier::coverage::ElementCoverage;
use crate::types::traits::derives::DerivedTrait;
use crate::types::analog::nature_rules::NatureRules;
use crate::types::{Callee, Substitution, Ty};

impl ModuleTyping<'_, '_> {
    /// `dir name: Nature;` — a node, or a carrier of `Bit<T>` that takes a direction (`E0310`).
    /// A default is the first commit of an `out` carrier (`E0210`); on any other port it is
    /// outside the subset.
    pub(in crate::types) fn port(&mut self, decl: DeclId, port: &PortData, file: FileId) {
        let ty = self.carriers().port_type(port.ty);
        if let Some(carrier) = ty.as_ref().filter(|t| matches!(t, Ty::Carrier { .. }) || (matches!(t, Ty::Array(..)) && t.is_carrier_element())) {
            self.state.result.carriers.insert(decl, carrier.clone());
        }
        let span = self.state.design.type_node(port.ty).span;
        // an array of carriers is directed as its element is (CV-23): the direction is the array's
        let kind = ty.as_ref().map(|t| t.innermost().clone());
        match (&kind, port.dir) {
            (Some(Ty::Carrier { nature, .. }), None) if NatureRules::of(self.state.design, *nature).one_driver => {
                self.coded(file, "E0310", span, "a port of a discrete nature takes `in` or `out` (§8.1.5)".to_string(), "no direction");
            }
            (Some(Ty::Carrier { nature, .. }), Some(_)) if NatureRules::of(self.state.design, *nature).resolved => {
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
                self.carriers().initial_reads(default);
                let value = array.read_value();
                self.within(Context::Const, |this| this.expressions().check_as(default, &value, "E0210"));
            }
            // the initial value of a resolved port is the module's first drive of it, of the driven type (§5.2.3)
            (Some(Ty::Carrier { nature, .. }), None) if NatureRules::of(self.state.design, *nature).resolved => {
                self.carriers().initial_reads(default);
                self.analog().resolved_initial(decl, *nature, default);
            }
            (Some(value), _) if !matches!(value, Ty::Node(_) | Ty::Carrier { .. }) => {
                let span = self.state.design.expr_node(default).span;
                self.coded(file, "E0311", span, "a value input takes no initial value (§8.1.7)".to_string(), "no initial value here");
                self.expressions().infer(default);
            }
            // An `in` port of a top level starts at its written value too (§3.3.4, §23.1 rule 9).
            (Some(Ty::Carrier { value, .. }), Some(crate::msl::ast::Dir::Out | crate::msl::ast::Dir::In)) => {
                self.carriers().initial_reads(default);
                self.within(Context::Const, |this| this.expressions().check_as(default, value, "E0210"));
            }
            _ => {
                let span = self.state.design.expr_node(default).span;
                self.refuse(file, span, StructureForm::PortDefaultValue);
                self.expressions().infer(default);
            }
        }
    }

    /// `name: T = default where cond;` — `T` is `f64`, `bool`, an integer, a unit enum, an `Option` (PI-01 to PI-03), a module (its value a template
    /// `Name { params }`, PI-04) or a struct, tuple or array of plain data (CV-18); `where` is `bool` (`E0318`).
    pub(in crate::types) fn param(&mut self, decl: DeclId, param: &ParamData) {
        let ty = self.expressions().lower_type(param.ty).and_then(|ty| match ty {
            Ty::F64 | Ty::Bool | Ty::Module(_) => Some(ty),
            // an array of templates (`pair: [Cell; 2]`, §8.4.1): each element a template, the param structural as one template is
            Ty::Array(ref element, _) if matches!(**element, Ty::Module(_)) => Some(ty),
            // An integer, enum, `Option` or composite param (PI-01 to PI-03, CV-18).
            plain if self.expressions().is_param_type(&plain) => Some(plain),
            // a param of a type parameter (`reset: T`, §8.1.15): typed under the parameter's bounds
            Ty::Param(parameter) if self.expressions().is_value_type_parameter(parameter) => Some(Ty::Param(parameter)),
            // a param of a type parameter bound by `Fn(P)` (`cell: C`, §8.4.1) holds a template of a module that fits `P`
            Ty::Param(parameter) if self.generics().callable_of(parameter).is_some() => Some(Ty::Param(parameter)),
            other => {
                let node = self.state.design.type_node(param.ty).clone();
                let file = self.state.design.decl_data(node.owner).file;
                self.checker.refuse_with(file, node.span, StructureForm::ParamOfType, [other.name(self.checker.state.design).to_string()]);
                None
            }
        });
        if let (Some(ty), Some(default)) = (&ty, param.default) {
            self.within(Context::Const, |this| this.expressions().check(default, ty));
        }
        if let Some(cond) = param.where_ {
            self.within(Context::Const, |this| this.expressions().check_as(cond, &Ty::Bool, "E0318"));
            self.where_reads_later_param(decl, cond);
        }
    }

    /// A `where` may read its own param and earlier ones only: a read of a param of the same
    /// module declared after `decl` is `E0318` naming it (§8.1.1).
    fn where_reads_later_param(&mut self, decl: DeclId, expr: ExprId) {
        let node = self.state.design.expr_node(expr).clone();
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(read)) = self.state.design.path_node(*path).segments.last().map(|s| s.res)
        {
            let (own, other) = (self.state.design.decl_data(decl), self.state.design.decl_data(read));
            if matches!(other.kind, DeclKind::Param(_)) && other.parent == own.parent && other.span.start > own.span.start {
                let name = other.name.as_ref().map_or("?", |n| n.text.as_str()).to_string();
                let file = self.file_of_expr(expr);
                let message = format!("a `where` reads the param `{name}`, declared after it (§8.1.1)");
                self.report(file, Diagnostic::new("E0318", message, node.span, format!("`{name}` is declared later")));
            }
        }
        for child in crate::pom::BodyWalk::new(self.state.design).expression_children(&node.kind) {
            self.where_reads_later_param(decl, child);
        }
    }

    /// `signal name: Nature = init;` — a node whose initial value is an `f64`, or a carrier of
    /// `Bit<T>` whose initial value is a `T`; either is `E0210` when the type differs or the
    /// value reads a carrier or a probe (§5.1.5).
    pub(in crate::types) fn signal(&mut self, decl: DeclId, signal: &SignalData) {
        let ty = self.carriers().port_type(signal.ty);
        if let Some(carrier) = ty.as_ref().filter(|t| matches!(t, Ty::Carrier { .. }) || (matches!(t, Ty::Array(..)) && t.is_carrier_element())) {
            let held = self.generated_type(decl, carrier.clone());
            self.state.result.carriers.insert(decl, held);
        }
        // a generated node is an array of nodes, one level for each generation `for` around it (§8.2.23)
        if let Some(node @ Ty::Node(_)) = ty.as_ref()
            && !self.generation_loops(decl).is_empty()
        {
            let held = self.generated_type(decl, node.clone());
            self.state.result.carriers.insert(decl, held);
        }
        let (Some(ty), Some(init)) = (ty, signal.init) else { return };
        self.carriers().initial_reads(init);
        if let Ty::Carrier { nature, .. } = &ty
            && NatureRules::of(self.state.design, *nature).resolved
        {
            return self.analog().resolved_initial(decl, *nature, init);
        }
        let value = match &ty {
            Ty::Carrier { value, .. } => (**value).clone(),
            // an array of carriers: the array of their values (`[0; 8]`); an array of nodes: an array of `f64`
            array @ Ty::Array(..) if matches!(array.innermost(), Ty::Carrier { .. }) => array.read_value(),
            Ty::Array(_, length) => Ty::Array(Box::new(Ty::F64), length.clone()),
            _ => Ty::F64,
        };
        self.within(Context::Const, |this| this.expressions().check_as(init, &value, "E0210"));
    }

    /// Types the `let` item `decl` of a module body once: in the body's order, or earlier where a read of its name needs its
    /// type (§8.2: the items of a body appear in any order, and every name is visible in the whole body).
    pub(in crate::types) fn type_module_let(&mut self, decl: DeclId) {
        if !self.state.modules.lets_typed.insert(decl) {
            return;
        }
        let DeclKind::Let(data) = self.state.design.decl_data(decl).kind.clone() else { return };
        // typed as the body types it, whatever position reads it first (a param default, a `where`)
        self.within(crate::types::check::Context::Module, |this| this.modules().module_let(decl, &data));
    }

    /// The type of the local `binding`, the one entry every read of a local goes through: a module-body `let` read before its
    /// text position is typed here first (§8.2: the items of a body appear in any order), so no site depends on text order.
    pub(in crate::types) fn binding_type(&mut self, binding: DeclId) -> Option<Ty> {
        self.type_module_let_read(binding);
        self.state.result.locals.get(&binding).cloned()
    }

    /// Types the module-body `let` that binds `binding` when a read of it comes before it in the body (§8.2: items appear in any
    /// order); nothing for a binding typed already, one being typed (a self read, §8.2.16) or any other name.
    pub(super) fn type_module_let_read(&mut self, binding: DeclId) {
        if !self.state.result.locals.contains_key(&binding)
            && !self.state.modules.lets_open.contains_key(&binding)
            && let Some(item) = self.untyped_module_let(binding)
        {
            self.type_module_let(item);
        }
    }

    /// The `let` item of a module body that binds `binding` and is not typed yet, outside generation (generation types its own).
    pub(super) fn untyped_module_let(&self, binding: DeclId) -> Option<DeclId> {
        let item = self.state.design.decl_data(binding).parent?;
        let DeclKind::Let(_) = self.state.design.decl_data(item).kind else { return None };
        let owner = self.state.design.decl_data(item).parent?;
        (matches!(self.state.design.decl_data(owner).kind, DeclKind::Module(_)) && !self.state.modules.lets_typed.contains(&item)).then_some(item)
    }

    /// `let x = value;` in a module body: the binding takes the type of the value (a placement
    /// gives it the placed module).
    pub(super) fn module_let(&mut self, decl: DeclId, data: &LetData) {
        let pat = self.state.design.pat_node(data.pat).clone();
        let file = self.state.design.decl_data(pat.owner).file;
        if matches!(pat.kind, PatKind::Tuple(_)) {
            self.patterns().let_pattern(file, data.pat, None, Some(data.value), None);
            return;
        }
        let PatKind::Ident { sub: None, binding, res: Resolution::Unresolved, .. } = &pat.kind else {
            self.refuse(file, pat.span, RefusedForm::pattern(&pat.kind));
            self.carriers().infer_event_position(data.value);
            return;
        };
        self.state.modules.lets_open.insert(*binding, data.value);
        let reported = self.state.diagnostics.count();
        let ty = self.carriers().infer_event_position(data.value);
        self.state.modules.lets_open.remove(binding);
        // a template whose value was refused (a choice of two modules, `E0180`) holds no template to place: its reads and
        // calls add nothing to that diagnostic
        let ty = match ty {
            Some(Ty::Module(_)) if self.state.diagnostics.count() > reported && !self.binds_an_instance(*binding) => None,
            other => other,
        };
        if let Some(closure) = self.expressions().closure_expr(data.value) {
            self.state.values.bind_closure(*binding, closure);
        }
        match ty {
            Some(Ty::Str) => {
                self.refuse(file, pat.span, StructureForm::StringLiteralBoundByLet);
                self.state.values.poison(*binding);
            }
            Some(ty) => {
                // inside a generation `for` the binding is the array of its element (§8.2.23)
                let held = self.generated_type(decl, ty);
                self.state.result.locals.insert(*binding, held);
            }
            None => {
                self.state.values.poison(*binding);
            }
        }
    }

    /// `Module { param: value }(nodes)` or `Module(nodes) { param: value }`: the module is
    /// reached; each node argument has the nature of its port, their count is the port count
    /// (`E0312`), each param value the type of its param.
    pub(in crate::types) fn placement(&mut self, expr: ExprId, module: DeclId) -> Option<Ty> {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Call { callee, args, params, .. } = &node.kind else { return None };
        // §8.3.4: a function, a closure and a reaction body place no instance, nor does a const
        // context.
        if matches!(self.state.values.context(), Context::Function | Context::Reaction | Context::Const) {
            let name = self.state.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            if self.state.values.context() == Context::Const {
                self.coded(file, "E0352", node.span, format!("`{name}` is placed in a const context: a result is not fixed during a solve (§8.3.4)"), "placement in a const context");
            } else {
                let place = if self.state.values.context() == Context::Reaction { "a reaction body" } else { "a `fn` or a closure" };
                self.coded(file, "E0350", node.span, format!("`{name}` is placed in {place}: it places no instance (§8.3.4)"), "placement out of place");
            }
            for arg in args {
                self.expressions().infer(arg.value);
            }
            return None;
        }
        let callee_node = self.state.design.expr_node(*callee).clone();
        let mut through_value = false;
        let fields: Vec<FieldInit> = match &callee_node.kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module && params.is_none() => fields.clone(),
            ExprKind::Path(_) if matches!(self.state.design.decl_data(module).kind, DeclKind::Module(_)) && self.callee_is(*callee, module) => params.clone().unwrap_or_default(),
            // `cell(c)`, `C1(c)` or `nch(l, w)(c)`: a call through a template value, a param, a `const` or a function's
            // result of module type (§8.4.4); the module's ports are connected, the param block belongs to the template, so
            // one on the placement is `E0361`
            _ if self.callee_is_template_of(*callee, module) => {
                if params.is_some() {
                    let name = self.state.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                    self.coded(file, "E0361", node.span, format!("a placement through a template value of module `{name}` takes no param block: the template holds the params (§8.4.4)"), "param block on a placement through a template value");
                }
                through_value = true;
                Vec::new()
            }
            // the head's value is no template of the module: a value of another type is a mismatch (§1.5); a head that has no
            // type was refused where it is typed, and the placement adds nothing to that
            _ => {
                if let Some(found) = self.state.result.exprs[callee.index()].clone() {
                    let message = format!("the head's type `{}` is no template of the module this call places (§8.4.4)", found.name(self.state.design));
                    self.coded(file, "E0180", node.span, message, "a placement through a value that is not a template");
                }
                for arg in args {
                    self.expressions().infer(arg.value);
                }
                return None;
            }
        };
        // the arguments of a generic module placed through a template value are the ones its type names, `G<2>` (§8.4.3)
        let generic = self.state.design.decl_data(module).generics_span.is_some();
        let through_template = (generic && through_value).then(|| self.generics().template_substitution(*callee, module));
        // pushed before the arguments are typed: a `let` that feeds back through its own placement is judged by §8.3.5
        self.state.modules.placing.push(Some(module));
        // recorded before the arguments are typed, which infer the generic arguments: a feedback through the placement reads it
        self.state.result.callees.insert(expr, Callee::Module(module));
        self.state.result.exprs[callee.index()] = Some(Ty::module(module));
        let substitution = if through_template.is_some() {
            through_template
        } else if generic {
            let Some(given) = self.generics().generic_placement(expr, *callee, module, args, &fields) else {
                self.state.modules.placing.pop();
                self.keep_handed_after_failure(module, args);
                return None;
            };
            Some(given)
        } else {
            None
        };
        self.reach(module);
        let outer = self.state.generics.replace_placement_substitution(substitution.clone());
        self.connections(expr, module, args, file);
        self.state.generics.replace_placement_substitution(outer);
        self.state.modules.placing.pop();
        self.param_block(&fields, substitution.as_ref());
        let placed = self.placement_type(module)?;
        Some(substitution.map_or(placed.clone(), |substitution| substitution.apply(&placed)))
    }

    /// The placement of `module` failed its generic arguments and typed no connection: the carriers it was to take at an `out`
    /// port stay handed over, so the failure is reported once, at the placement, and not again as a carrier nothing drives
    /// (§5.2.6; the placement's own diagnostic is the cause).
    fn keep_handed_after_failure(&mut self, module: DeclId, args: &[Arg]) {
        let outs: Vec<Option<crate::msl::ast::Dir>> = self.state.design.decl_data(module).children.iter().filter_map(|c| match &self.state.design.decl_data(*c).kind {
            DeclKind::Port(p) => Some(p.dir),
            _ => None,
        }).collect();
        let bound = crate::pom::PortArguments::bind(self.state.design, module, args);
        for (index, dir) in outs.into_iter().enumerate() {
            let Some(arg) = bound.at(index) else { continue };
            if dir != Some(crate::msl::ast::Dir::Out) {
                continue;
            }
            if let Some((decl, Ty::Carrier { .. })) = self.carriers().declared_port(arg.value) {
                let (file, span) = (self.file_of_expr(arg.value), self.state.design.expr_node(arg.value).span);
                // a placement is typed once per path that reaches it: it hands the carrier over once
                if !self.state.carriers.is_handed_over_at(decl, span) {
                    self.state.carriers.hand_over(decl, crate::types::carrier::element_target::Element::Whole, span, file);
                }
            }
        }
    }

    /// Whether `expr` is a name whose value already raised a diagnostic.
    pub(super) fn reads_poisoned(&self, expr: ExprId) -> bool {
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Path(path) => matches!(self.state.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if self.state.values.is_poisoned(d)),
            _ => false,
        }
    }

    /// Whether `callee` is a const expression whose value is a template of `module` (§8.4.4, §8.4.6): a param, a `const`, a
    /// function's result, a template written in place. A callee that reads a port, a signal or a probe is `E0362`.
    fn callee_is_template_of(&mut self, callee: ExprId, module: DeclId) -> bool {
        if !matches!(self.expressions().infer(callee), Some(Ty::Module(found)) if found.decl == module) {
            return false;
        }
        // through the `let`s that hold it (`let t = if sel { .. } else { .. };`): a choice of templates the run makes is no
        // const expression either
        if let Some(read) = self.run_read(callee, &mut Vec::new()) {
            let (file, span) = (self.file_of_expr(callee), self.state.design.expr_node(callee).span);
            self.coded(file, "E0362", span, format!("a template is a const expression and this placement's head reads {read} (§8.4.6)"), "not a const expression");
        }
        true
    }

    fn callee_is(&self, callee: ExprId, module: DeclId) -> bool {
        match &self.state.design.expr_node(callee).kind {
            ExprKind::Path(path) => self.state.design.path_node(*path).segments.last().map(|s| s.res) == Some(Resolution::Decl(module)),
            _ => false,
        }
    }

    /// The arguments of a placement against the ports of the module: a node by nature, a
    /// carrier or a discrete expression at an `in` port, a carrier the parent may hand over at
    /// an `out` port, an expression of its type for a value input (§3.2). Their count is the port
    /// count (`E0312`), or for a module with value inputs or a result, from the required ones to
    /// all of them, a trailing `Option` input being left out as `None` (`E0141`).
    fn connections(&mut self, expr: ExprId, module: DeclId, args: &[Arg], file: FileId) {
        let ports: Vec<(DeclId, TypeId, Option<crate::msl::ast::Dir>)> = self.state.design.decl_data(module).children.iter().filter_map(|c| match &self.state.design.decl_data(*c).kind {
            DeclKind::Port(p) => Some((*c, p.ty, p.dir)),
            _ => None,
        }).collect();
        // a bundle port has no type of its own: its fields are the leaves it stands for (`bundle.rs`)
        let kinds: Vec<Option<Ty>> = ports.iter().map(|(decl, ty, _)| if self.state.design.is_bundle_owner(*decl) { None } else { self.generics().port_type_under_placement(*ty) }).collect();
        let call_rule = matches!(&self.state.design.decl_data(module).kind, DeclKind::Module(m) if m.ret.is_some()) || ports.iter().any(|(d, _, _)| self.is_value_input(*d));
        let optional = if call_rule { kinds.iter().rev().take_while(|k| matches!(k, Some(Ty::Option(_)))).count() } else { 0 };
        let (min, max) = (ports.len() - optional, ports.len());
        // a named connection that is no trailing `Option` input is `E0140` from the POM (`classify.rs`), before this check
        let positional = args.iter().filter(|arg| arg.name.is_none()).count();
        if positional < min || args.len() > max {
            let span = self.state.design.expr_node(expr).span;
            let name = self.state.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            if call_rule {
                let message = if min == max { format!("`{name}` takes {max} argument(s), found {}", args.len()) } else { format!("`{name}` takes {min} to {max} arguments, found {}", args.len()) };
                self.coded(file, "E0141", span, message, "wrong number of arguments");
            } else {
                let message = format!("`{name}` has {} port(s), found {} connection(s)", ports.len(), args.len());
                self.coded(file, "E0312", span, message, "wrong number of connections");
            }
            for arg in args {
                self.expressions().infer(arg.value);
            }
            return;
        }
        let bound = crate::pom::PortArguments::bind(self.state.design, module, args);
        for (index, kind) in kinds.iter().cloned().enumerate() {
            let Some(arg) = bound.at(index) else { continue };
            let (port_decl, _, dir) = ports[index];
            if self.state.design.is_bundle_owner(port_decl) {
                self.connect_bundle(arg.value, port_decl);
                continue;
            }
            match kind {
                Some(carrier @ Ty::Carrier { .. }) => self.connect_carrier(arg.value, dir, &carrier),
                Some(array @ Ty::Array(..)) if array.is_carrier_element() => self.connect_array(arg.value, dir, &array),
                Some(expected @ Ty::Node(_)) => {
                    let Some(found) = self.expressions().infer(arg.value) else { continue };
                    let name = self.state.design.decl_data(port_decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                    self.connect(arg.value, &format!("the node port `{name}`"), &expected, &found);
                }
                // a nature parameter of the enclosing generic module, passed on to a placement: a node of that nature, or `gnd`
                Some(expected) if self.is_nature_parameter(&expected) => {
                    let Some(found) = self.expressions().infer(arg.value) else { continue };
                    if found != expected && found != Ty::AnyNode {
                        let span = self.state.design.expr_node(arg.value).span;
                        self.mismatch(file, span, &expected, &found);
                    }
                }
                // A value input: any expression of its type, a probe included (§8.1.13).
                Some(value) => {
                    self.expressions().check_argument(arg.value, &value);
                    self.carriers().definition_value_input(expr, module, port_decl, arg.value);
                }
                None => {
                    self.expressions().infer(arg.value);
                }
            }
        }
    }

    /// One node argument to the port `port` that `where_` names ("the node port `p`"): the nature of its port, or `gnd`
    /// (§8.1.10). A value (an expression that is not a node) at a continuous carrier port is `E0315` naming the port: only an
    /// `in` port takes an expression, and a node port is never one (§8.1.11).
    pub(in crate::types) fn connect(&mut self, arg: ExprId, where_: &str, port: &Ty, found: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.state.design.expr_node(arg).span;
        // A discrete carrier read as a value is typed by what it holds; the declaration tells.
        if let Some((_, Ty::Carrier { .. })) = self.carriers().declared_port(arg) {
            let message = format!("a discrete carrier is connected to a port of {} (§6.1)", port.name(self.state.design));
            self.coded(file, "E0250", span, message, "discrete carrier at a continuous port");
            return;
        }
        match (port, found) {
            (Ty::Node(p), Ty::Node(f)) if NatureRules::compatible(self.state.design, *p, *f) => {}
            (_, Ty::AnyNode) => {}
            (Ty::Node(_), Ty::Node(_)) => {
                let message = format!("expected {}, found {}", port.name(self.state.design), found.name(self.state.design));
                self.coded(file, "E0251", span, format!("a node of another nature is connected: {message}"), &message);
            }
            (Ty::Node(_), _) => {
                let message = format!("an expression of type {} is connected to {where_}: only an `in` port of a discrete nature takes an expression (§8.1.11)", found.name(self.state.design));
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
        let span = self.state.design.expr_node(arg).span;
        let carrier = self.carriers().declared_port(arg);
        let found = self.expressions().infer(arg);
        match (&carrier, dir) {
            (Some((_, Ty::Node(_))), _) => {
                self.coded(file, "E0250", span, "a node is connected to a port of a discrete nature (§6.1)".to_string(), "node at a discrete port");
            }
            (None, _) if found == Some(Ty::AnyNode) => {
                self.coded(file, "E0250", span, "`gnd` is connected to a port of a discrete nature (§6.1)".to_string(), "`gnd` at a discrete port");
            }
            (Some((decl, held @ Ty::Carrier { .. })), dir) => {
                if held != port {
                    let message = format!("a carrier of `{}` is connected to a port of `{}`", held.name(self.state.design), port.name(self.state.design));
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
                self.expressions().check_as(arg, value, "E0337");
                if self.analog().continuous(arg) {
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
        if matches!(&self.state.design.decl_data(decl).kind, DeclKind::Port(p) if p.dir == Some(crate::msl::ast::Dir::In)) {
            let message = "an `in` port of the parent is connected to an `out` port: the parent may not hand over what it does not drive (§8.1.11)".to_string();
            self.coded(file, "E0314", span, message, "an `in` port at an `out` port");
            return;
        }
        let named = self.carriers().element_named_by(arg);
        let element = self.carriers().judged_element(arg, decl, named);
        self.state.carriers.hand_over(decl, element, span, file);
    }

    /// [`ModuleTyping::hand_over`] of the elements `element` of a carrier no expression names: a field of a bundle given whole
    /// (`s.valid`), or of an element of an array of bundles (`lanes[k]` hands the element `k` of the leaf `lanes.a`). `place` is
    /// the expression that names the bundle: one written inside generation reaches the carrier in the iterations elaboration
    /// takes only.
    pub(super) fn hand_over_element(&mut self, decl: DeclId, element: crate::types::carrier::element_target::Element, place: ExprId, span: Span, file: FileId) {
        if matches!(&self.state.design.decl_data(decl).kind, DeclKind::Port(p) if p.dir == Some(crate::msl::ast::Dir::In)) {
            let message = "an `in` port of the parent is connected to an `out` port: the parent may not hand over what it does not drive (§8.1.11)".to_string();
            self.coded(file, "E0314", span, message, "an `in` port at an `out` port");
            return;
        }
        let element = self.carriers().judged_element(place, decl, element);
        self.state.carriers.hand_over(decl, element, span, file);
    }

    /// One argument at an array port (§8.1.10): an array of carriers (or of nodes) of the port's shape, or a part of
    /// a nested one that has it. Another length or shape is `E0313` naming both; another element type `E0251`.
    fn connect_array(&mut self, arg: ExprId, dir: Option<crate::msl::ast::Dir>, port: &Ty) {
        let file = self.file_of_expr(arg);
        let span = self.state.design.expr_node(arg).span;
        let carrier = self.carriers().declared_port(arg);
        let found = self.expressions().infer(arg);
        match carrier {
            Some((decl, held)) if held.is_carrier_element() => {
                if !self.same_shape(&held, port) {
                    self.shape_mismatch(file, span, &held, port);
                } else if !self.same_elements(&held, port) {
                    let message = format!("an array of `{}` is connected to a port of `{}`", held.name(self.state.design), port.name(self.state.design));
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
                    self.expressions().check_as(arg, &port.read_value(), "E0337");
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
            (Ty::Node(f), Ty::Node(p)) => NatureRules::compatible(self.state.design, *p, *f),
            (found, port) => found == port,
        }
    }

    /// `E0313`: a connection of another array shape, naming both.
    fn shape_mismatch(&mut self, file: FileId, span: Span, found: &Ty, port: &Ty) {
        let message = format!("`{}` is connected to a port of `{}`: the lengths or the shape differ (§8.1.10)", found.name(self.state.design), port.name(self.state.design));
        self.coded(file, "E0313", span, message, "another array shape");
    }

    /// A carrier handed to an `out` port has one drive, the instance's: a second hand-over, or
    /// a definition or a commit of this module, is `E0220`. An initial value is the value of the elements no `out` port
    /// holds (§5.2.6, §8.2.21), so it is `E0223` only once every element is handed (`handed_initial_values`).
    pub(in crate::types) fn handed_drives(&mut self) {
        let handed = self.state.carriers.take_handed();
        let mut given: Vec<(DeclId, crate::types::carrier::element_target::Element)> = Vec::new();
        let mut clean: Vec<(DeclId, crate::types::carrier::element_target::Element, Span, FileId)> = Vec::new();
        for (decl, element, span, file) in handed {
            // the element as the source names it: `mem[2]`, `lanes[1].a` for a field of an array of bundles
            let name = self.state.design.element_naming(decl).name(&element.subscript());
            // a resolved carrier has as many drives as modules hand it down or define it (§5.2.1): nothing here is a second one
            if matches!(self.state.result.carriers.get(&decl).map(Ty::innermost), Some(Ty::Carrier { nature, .. }) if NatureRules::of(self.state.design, *nature).resolved) {
                continue;
            }
            let twice = given.iter().any(|(d, e)| *d == decl && e.overlaps(element));
            given.push((decl, element));
            // an element only the elaborator folds is judged there against the whole-carrier drives too (`elab/drive_overlaps.rs`)
            let whole_drive = element != crate::types::carrier::element_target::Element::Unfolded && self.state.carriers.drives_whole(decl);
            let driven_here = whole_drive || self.state.carriers.drives_element(decl, element);
            let nature = match self.state.result.carriers.get(&decl).map(Ty::innermost) {
                Some(Ty::Carrier { nature, .. }) => self.state.design.decl_data(*nature).name.as_ref().map_or_else(String::new, |n| n.text.clone()),
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
    fn handed_initial_values(&mut self, handed: &[(DeclId, crate::types::carrier::element_target::Element, Span, FileId)]) {
        let mut judged = BTreeSet::new();
        for (decl, _, span, file) in handed {
            // a leaf of a bundle starts at the default of its field, which the instance that holds the drive repeats: no second initial value
            if !judged.insert(*decl) || !self.carriers().has_initial(*decl) || self.state.design.leaf_owner(*decl).is_some() {
                continue;
            }
            let Some(held) = self.state.result.carriers.get(decl).cloned() else { continue };
            let mut coverage = ElementCoverage::of(&held);
            for (_, element, _, _) in handed.iter().filter(|(carrier, ..)| carrier == decl) {
                coverage.cover(*element);
            }
            if coverage.is_deferred() {
                self.state.result.deferred_coverage.handed_initial.insert(*decl);
            } else if coverage.is_complete() {
                let name = self.state.design.decl_data(*decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
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
    pub(in crate::types) fn missing_initial_values(&mut self, children: &[DeclId]) {
        for decl in children.iter().copied() {
            let item = self.state.design.decl_data(decl).clone();
            let is_carrier = match &item.kind {
                DeclKind::Signal(_) => true,
                DeclKind::Port(port) => port.dir == Some(crate::msl::ast::Dir::Out),
                _ => false,
            };
            let Some(held) = self.state.result.carriers.get(&decl).cloned() else { continue };
            // an array of carriers has the rule of its elements: every element starts at the `Default` of the carried type
            let Some(Ty::Carrier { nature, value }) = (match held.innermost() {
                carrier @ Ty::Carrier { .. } => Some(carrier.clone()),
                _ => None,
            }) else {
                continue;
            };
            if NatureRules::of(self.state.design, nature).resolved {
                self.analog().missing_resolved_initial_value(decl);
                continue;
            }
            // an array with no element holds nothing to start
            if held.has_fixed_zero_length() {
                continue;
            }
            // a carrier with no initial value starts at its `Default`: a hand-written one is a function the elaborator folds
            if !self.carriers().has_initial(decl) {
                self.reach_written_defaults(&value);
            }
            if !is_carrier || self.carriers().has_initial(decl) || !NatureRules::of(self.state.design, nature).one_driver || self.trait_typing().implements(&value, DerivedTrait::Default) {
                continue;
            }
            // each element needs its own driver: the elements no driver names have neither value nor initial value (§3.3.4, §8.2.20)
            let name = item.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let coverage = self.carriers().driven_coverage(decl, &held);
            if coverage.is_deferred() {
                self.state.result.deferred_coverage.missing_initial.insert(decl);
                continue;
            }
            let Some(unnamed) = coverage.unnamed(&self.state.design.element_naming(decl)) else { continue };
            let found = value.name(self.state.design);
            let message = format!("`{unnamed}` has no initial value and its type `{found}` does not implement `Default`: a carrier has a value from (0, 0) on (§3.3.4)");
            // a bundle port takes no initial value (§7 rule 10): the field's own declaration gives it, written with the direction the
            // `struct` writes (`in ready: Bit = false` for a `flip` consumer that drives `ready`)
            let help = match self.state.design.leaf_field(decl) {
                Some((field, dir)) => {
                    let written = match dir {
                        Some(Dir::In) => "in ",
                        Some(Dir::Out) => "out ",
                        None => "",
                    };
                    format!("write `= value` after the type of the field `{written}{field}` in its `struct` (the initial value of the carrier on the side that drives the field, §7 rule 10), or carry an `Option<{found}>`")
                }
                None => format!("write `= value` after the type of `{name}`, or carry an `Option<{found}>`"),
            };
            self.report(item.file, Diagnostic::new("E0187", message, item.span, "no initial value").with_help(help));
        }
    }

    /// `{ name: value }` of a placement: each value against the declared type of its param.
    pub(super) fn param_block(&mut self, fields: &[FieldInit], substitution: Option<&Substitution>) {
        for field in fields {
            self.no_placement_in(field.value);
            let param = match field.res {
                Resolution::Decl(d) => match &self.state.design.decl_data(d).kind {
                    DeclKind::Param(p) => Some(p.ty),
                    _ => None,
                },
                _ => None,
            };
            // a param of a generic module's type `T` takes the type the placement gave `T`
            let declared = param.and_then(|t| self.expressions().lower_type(t)).map(|ty| substitution.map_or(ty.clone(), |given| given.apply(&ty)));
            match declared {
                Some(ty @ (Ty::F64 | Ty::Bool | Ty::Module(_))) => self.expressions().check(field.value, &ty),
                // a value of `T` bound to an `Option<T>` param is `Some` of it (§3.2.3, PI-03 AC 2)
                Some(Ty::Option(inner)) => {
                    let option = Ty::Option(inner.clone());
                    self.expressions().option_argument(field.value, &option, &inner)
                }
                Some(plain) if self.expressions().is_param_type(&plain) => self.expressions().check(field.value, &plain),
                _ => {
                    self.expressions().infer(field.value);
                }
            }
        }
    }
}

impl ModuleTyping<'_, '_> {
    /// A bound param value places no instance (§8.3.4, `E0352`): a result is not fixed during a
    /// solve.
    fn no_placement_in(&mut self, value: ExprId) {
        let mut all = Vec::new();
        crate::pom::BodyWalk::new(self.state.design).descendants_depth_first(value, &mut all);
        for expr in all {
            let node = self.state.design.expr_node(expr).clone();
            if matches!(node.kind, ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. }) {
                let file = self.file_of_expr(expr);
                self.coded(file, "E0352", node.span, "a bound param value places no instance: a result is not fixed during a solve (§8.3.4)".to_string(), "placement in a bound param value");
            }
        }
    }
}

impl ModuleTyping<'_, '_> {
    /// Brings every hand-written `impl Default` the default of `ty` reads into the check: the `default` function of a struct
    /// that writes one, and of the structs, tuples and arrays holding such a struct.
    pub(in crate::types) fn reach_written_defaults(&mut self, ty: &Ty) {
        match ty {
            Ty::Struct(nominal) => {
                let table = std::rc::Rc::clone(&self.state.traits);
                if let Some(function) = table.builtin_functions("Default", "default", ty).first() {
                    self.reach(*function);
                } else if self.expressions().derives(nominal.decl, "Default") {
                    let substitution = self.generics().nominal_substitution(nominal);
                    let fields: Vec<Ty> = self.expressions().fields_of(nominal.decl).unwrap_or_default().into_iter().map(|field| self.generics().substitute(&substitution, &field.ty)).collect();
                    fields.iter().for_each(|field| self.reach_written_defaults(field));
                }
            }
            Ty::Tuple(items) => items.iter().for_each(|item| self.reach_written_defaults(item)),
            Ty::Array(element, _) => self.reach_written_defaults(element),
            _ => {}
        }
    }
}
