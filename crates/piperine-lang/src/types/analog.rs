//! Analog forms: probes (`v(a, b)`, `potential`), contributions (`+>`) and the core primitives
//! of the subset (`ddt`, `limit`, `ac_stim` and the environment functions of §14.4).

use crate::pom::{Arg, BuiltinId, BuiltinKind, DeclId, DeclKind, DeferReason, ExprId, ExprKind, QuantityRole, Resolution};

use super::catalogue::{Allowed, Core};
use super::check::{Checker, Context};
use super::nature_rules::NatureRules;
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

    /// `p.inflow()` (part III §6 rule 5): the flow entering the module at its port `p`, of a nature
    /// with `flow`, an `f64`. Anything else is `E0243`; it is a value, so on the left of `+>` it is
    /// `E0410` (`contribution`).
    pub(super) fn inflow_read(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        if matches!(self.context, Context::Function | Context::Const) {
            self.coded(file, "E0450", node.span, "`inflow()` is allowed in an `analog` block and a module body only".to_string(), "core primitive out of place");
        }
        self.apply(expr, &[], args)?;
        let measured = match self.declared_port(recv) {
            Some((decl, Ty::Node(nature))) => matches!(self.design.decl_data(decl).kind, DeclKind::Port(_)) && NatureRules::of(self.design, nature).flow,
            _ => false,
        };
        if !measured {
            let message = "`inflow()` applies to a port of a nature with `flow`: it measures the flow entering the module there (§6 rule 5)".to_string();
            self.coded(file, "E0243", node.span, message, "not a port of a nature with `flow`");
            self.infer(recv);
            return None;
        }
        self.infer(recv);
        self.builtins.insert(expr, super::Builtin::Inflow);
        Some(Ty::F64)
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
            // `v(bank.c0.sn)`: the nature of the item is the concrete module's, known at elaboration; a potential access
            // name is read as a potential probe until then
            Resolution::Deferred(DeferReason::AmbiguousAccess) if self.reads_a_template_potential(*callee, args) => {
                self.access_roles.insert(*callee, QuantityRole::Potential);
                (QuantityRole::Potential, None)
            }
            // `v(y)` with `let y = x;`: the resolver names no nature for a local, the node it holds does (§6 rule 4); the role is
            // recorded for the elaborator, which reads a probe and a contribution target alike through it
            Resolution::Deferred(DeferReason::AmbiguousAccess) => match self.access_of_operand(*callee, args) {
                OperandAccess::Found(role, nature) => {
                    self.access_roles.insert(*callee, role);
                    (role, Some(nature))
                }
                // `v(y)` with `let y = t;`, `t: Thermal`: what `v(t)` is at resolution (§6 rule 4, §6 rule 10, §11 rule 11)
                OperandAccess::Missing(name) => {
                    self.missing_access(file, *callee, &name, target);
                    return None;
                }
                // an operand refused already (`E0133` for a chosen node, or a `let` of one) is no second diagnostic
                OperandAccess::NoNode if args.first().is_some_and(|arg| self.operand_refused(arg.value)) => return None,
                OperandAccess::NoNode => {
                    self.unsupported(file, node.span, "probe through an access that names several natures");
                    return None;
                }
            },
            Resolution::Deferred(_) | Resolution::Unresolved => {
                self.unsupported(file, node.span, "probe through an unresolved name");
                return None;
            }
        };
        // a function probes through a handle (§3 rule 7, §3.1.5); a probe of anything else is out of place
        let operands: Vec<ExprId> = args.iter().map(|arg| arg.value).collect();
        let through_handles = self.context == Context::Function && self.probes_only_handles(&operands);
        if matches!(self.context, Context::Function | Context::Const) && !through_handles {
            self.coded(file, "E0450", node.span, "a probe is allowed in an `analog` block and a module body only".to_string(), "core primitive out of place");
        }
        if args.is_empty() || args.len() > 2 {
            self.arity_mismatch(file, node.span, "1 to 2", args);
            return None;
        }
        let mut natures = Vec::new();
        for arg in args {
            match self.infer(arg.value) {
                Some(Ty::Node(n)) => natures.push(n),
                Some(Ty::AnyNode) => {}
                // a handle generic over a nature: its nature is the instantiation's, known there
                Some(generic) if self.is_nature_parameter(&generic) => {}
                Some(other) => {
                    let span = self.design.expr_node(arg.value).span;
                    self.mismatch_text(file, span, "a node", &other.name(self.design));
                }
                None => {}
            }
        }
        // `i(y)` with `let y = t;`: the resolver named the one quantity of this access, the node's nature lacks it (§6 rule 10)
        if let (Some(declared), Some(first)) = (nature, natures.first().copied())
            && first != declared
            && let Some(name) = self.access_name(*callee)
            && self.access_role_in(first, &name).is_none()
        {
            self.missing_access(file, *callee, &name, target);
            return None;
        }
        if natures.windows(2).any(|w| w[0] != w[1]) || nature.is_some_and(|q| natures.iter().any(|n| *n != q)) {
            self.coded(file, "E0240", node.span, "the nodes of a probe have different natures".to_string(), "nodes of two natures");
            return None;
        }
        // §11 rule 11, §6 rule 10: a node of a nature without `flow` has no flow to contribute to
        // (`E0413`) or to probe (`E0206`).
        if role == QuantityRole::Flow && !natures.is_empty() && natures.iter().all(|n| !NatureRules::of(self.design, *n).flow) {
            if target {
                self.coded(file, "E0413", node.span, "a flow contribution needs a nature with a `flow`: a signal-flow node takes potential contributions only (§11 rule 11)".to_string(), "no flow on this nature");
            } else {
                self.coded(file, "E0206", node.span, "a flow probe needs a nature with a `flow`: the nature of this node declares none (§6 rule 10)".to_string(), "no flow on this nature");
            }
            return None;
        }
        match (role, target) {
            (QuantityRole::Potential, _) | (QuantityRole::Flow, true) => Some(Ty::F64),
            // The flow of a branch the module drives with a potential (`v(p, n) +> ddt(l * i(p, n))`,
            // the inductor) is a value; the flow of a flow branch is no value, but a leaf of a `ddx` value (`differentiated`).
            (QuantityRole::Flow, false) if self.contributes_potential_to(args) || self.differentiated > 0 => Some(Ty::F64),
            (QuantityRole::Flow, false) => {
                self.unsupported(file, node.span, "flow probe read as a value");
                None
            }
        }
    }

    /// Whether an `analog` block of the module being checked contributes a potential to the branch
    /// `args` name (the same nodes, in the same order).
    fn contributes_potential_to(&self, args: &[Arg]) -> bool {
        let Some(owner) = self.owner else { return false };
        let DeclKind::Module(m) = &self.design.decl_data(owner).kind else { return false };
        let nodes = |args: &[Arg]| -> Vec<Resolution> {
            args.iter()
                .map(|a| match &self.design.expr_node(a.value).kind {
                    ExprKind::Path(path) => self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res),
                    _ => Resolution::Unresolved,
                })
                .collect()
        };
        let wanted = nodes(args);
        if wanted.contains(&Resolution::Unresolved) {
            return false;
        }
        m.behaviors.iter().any(|b| {
            let DeclKind::Behavior(block) = &self.design.decl_data(*b).kind else { return false };
            block.kind == crate::pom::BehaviorKind::Analog
                && crate::pom::nested_stmts(self.design, &block.body).iter().any(|stmt| match &self.design.stmt_node(*stmt).kind {
                    crate::pom::StmtKind::Contribution { lhs, .. } => match &self.design.expr_node(*lhs).kind {
                        ExprKind::Call { callee, args: lhs_args, class: crate::pom::CallClass::Probe, .. } => {
                            let role = match &self.design.expr_node(*callee).kind {
                                ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                                    Some(Resolution::Decl(q)) => match &self.design.decl_data(q).kind {
                                        DeclKind::Quantity(data) => Some(data.role),
                                        _ => None,
                                    },
                                    Some(Resolution::Builtin(id)) if id.info().name == "potential" => Some(QuantityRole::Potential),
                                    _ => None,
                                },
                                _ => None,
                            };
                            role == Some(QuantityRole::Potential) && nodes(lhs_args) == wanted
                        }
                        _ => false,
                    },
                    _ => false,
                })
        })
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
        if matches!(name.as_str(), "cross" | "at") && self.event_slot != Some(expr) {
            self.core_event_out_of_place(expr, &name, args);
            return None;
        }
        match (sig.allowed, self.context) {
            (Allowed::Everywhere, Context::Const) => {
                let message = format!("`{name}` is an environment function and is not allowed in a const context");
                self.coded(file, "E0323", node.span, message, "environment function in a const context");
            }
            (Allowed::Everywhere, _) | (Allowed::Instances, Context::Analog | Context::Module) | (Allowed::Analog, Context::Analog) => {}
            // §8.3: a core module placed in a reaction body is `E0350` as well.
            (Allowed::Instances, Context::Reaction) => {
                let message = format!("`{name}` is a core module and cannot be placed in a reaction body");
                self.coded(file, "E0350", node.span, message, "placement in a reaction body");
            }
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
                    Context::Reaction => "a reaction body",
                    Context::Const => "a param default, a `where` or a `const`",
                    Context::Module => "a module body",
                    Context::Analog => "an `analog` block",
                };
                self.coded(file, "E0450", node.span, format!("`{name}` is not allowed in {place}"), "core primitive out of place");
            }
        }
        self.callees.insert(expr, Callee::Core(id));
        if name == "analysis" {
            return self.analysis_call(expr, args);
        }
        if name == "ddx" {
            return self.ddx_call(expr, args);
        }
        // §3.2: the arguments bind as a function's do, by position or by the name the row gives
        // its parameters. The arguments of a core module are read through a result with a direct
        // dependence (§8.3.6): a `let` that reads itself through one is a cycle with no unknown
        // in it.
        let (min, max) = (sig.required.len(), sig.required.len() + sig.optional.len());
        if args.len() < min || args.len() > max {
            let expected = if min == max { min.to_string() } else { format!("{min} to {max}") };
            self.arity_mismatch(file, node.span, expected, args);
            return None;
        }
        let params: Vec<Ty> = sig.required.iter().chain(sig.optional).map(|p| p.ty()).collect();
        let names = Self::core_parameter_names(&name);
        let named: Vec<Option<usize>> = args.iter().map(|a| a.name.as_ref().and_then(|n| names.iter().position(|p| *p == n.text))).collect();
        self.placing.push(None);
        let bound = self.bind_arguments(expr, &name, &params, sig.required.len(), args, &named);
        self.placing.pop();
        bound?;
        self.event_arguments(&name, args);
        Some(sig.result.ty())
    }

    /// The names of the parameters of a core primitive that takes `name:` arguments (§3.2), in
    /// order; the others take positional arguments only.
    fn core_parameter_names(name: &str) -> &'static [&'static str] {
        match name {
            "cross" => &["e", "ttol", "etol"],
            _ => &[],
        }
    }

    /// `ddx(e, probe)` (§14.2): `e` is an `f64`, and the second argument is a probe call of a
    /// potential or a flow of a branch of this module (`E0451`: `p.inflow()` is no probe call,
    /// and neither is a value or a handle). Both are read as `f64`; the result is an `f64`.
    fn ddx_call(&mut self, expr: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let [value, variable] = args else {
            self.arity_mismatch(file, span, 2, args);
            return None;
        };
        self.differentiated += 1;
        self.check_argument(value.value, &Ty::F64);
        self.differentiated -= 1;
        let probe_call = matches!(&self.design.expr_node(variable.value).kind, ExprKind::Call { class: crate::pom::CallClass::Probe, .. });
        if !probe_call {
            let at = self.design.expr_node(variable.value).span;
            self.coded(file, "E0451", at, "the second argument of `ddx` must be a probe call of a potential or a flow of a branch of this module (§14.2.2); `p.inflow()` is not accepted".to_string(), "not a probe call");
            self.infer(variable.value);
            return None;
        }
        self.exprs[variable.value.index()] = self.probe(variable.value, true);
        Some(Ty::F64)
    }

    /// `analysis(kind)` (§14.4): a `bool`. `Static` is true in an equilibrium solve; `Dc`,
    /// `Transient`, `Ac` and `Noise` tell which analysis runs. Any other argument is `P0001`
    /// naming `analysis`.
    fn analysis_call(&mut self, expr: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let [arg] = args else {
            self.arity_mismatch(file, span, 1, args);
            return None;
        };
        let kind = match &self.design.expr_node(arg.value).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) => Some(id.name()),
                _ => None,
            },
            _ => None,
        };
        if !matches!(kind.as_deref(), Some("Static" | "Dc" | "Transient" | "Ac" | "Noise")) {
            self.unsupported(file, span, "`analysis` of a kind other than `Static`, `Dc`, `Transient`, `Ac` or `Noise`");
            return None;
        }
        Some(Ty::Bool)
    }
}

/// What the node a probe's first operand holds says of an access name the resolver left ambiguous.
enum OperandAccess {
    /// The nature has a quantity of that access: its role, and the nature.
    Found(QuantityRole, DeclId),
    /// The nature declares no access of that name, which is named.
    Missing(String),
    /// The operand is no node of a nature.
    NoNode,
}

impl Checker<'_> {
    /// The quantity an access name `callee` (`v`) names in the nature of the node its first operand holds: a `let` bound to a node
    /// keeps the node's nature, so `v(y)` reads the potential of `y`'s nature.
    fn access_of_operand(&mut self, callee: ExprId, args: &[Arg]) -> OperandAccess {
        let Some(first) = args.first() else { return OperandAccess::NoNode };
        let Some(Ty::Node(nature)) = self.infer(first.value) else { return OperandAccess::NoNode };
        let Some(name) = self.access_name(callee) else { return OperandAccess::NoNode };
        match self.access_role_in(nature, &name) {
            Some(role) => OperandAccess::Found(role, nature),
            None => OperandAccess::Missing(name),
        }
    }

    /// The access name the callee `callee` of a probe writes (`v`, `i`).
    fn access_name(&self, callee: ExprId) -> Option<String> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        self.design.path_node(*path).segments.last().map(|segment| segment.name.text.clone())
    }

    /// The role of the quantity of `nature` that declares the access `name`, `None` when it declares none.
    fn access_role_in(&self, nature: DeclId, name: &str) -> Option<QuantityRole> {
        self.design.decl_data(nature).children.iter().find_map(|child| match &self.design.decl_data(*child).kind {
            DeclKind::Quantity(quantity) if self.is_accessed_as(quantity, name) => Some(quantity.role),
            _ => None,
        })
    }

    /// The diagnostic of the access `name` (the callee `callee`) that the nature of the probed node lacks, as resolution gives it for
    /// a node named directly: a flow access contributed to on a nature without `flow` is `E0413`, any other is `E0206`.
    fn missing_access(&mut self, file: crate::pom::FileId, callee: ExprId, name: &str, target: bool) {
        let span = self.design.expr_node(callee).span;
        let flow = (0..self.design.decl_count()).any(|index| match &self.design.decl_data(DeclId(index as u32)).kind {
            DeclKind::Quantity(quantity) => quantity.role == QuantityRole::Flow && self.is_accessed_as(quantity, name),
            _ => false,
        });
        if target && flow {
            let message = format!(
                "a flow contribution needs a nature with a `flow`: the nature of the first argument declares none, so `{name}` cannot be contributed to it (§11 rule 11)"
            );
            self.coded(file, "E0413", span, message, "no flow on this nature");
        } else {
            let message = format!("the nature of the first argument declares no access `{name}`");
            self.coded(file, "E0206", span, message, "not an access of that nature");
        }
    }

    /// Whether the quantity `quantity` declares the access name `name` (`access: v`).
    fn is_accessed_as(&self, quantity: &crate::pom::QuantityData, name: &str) -> bool {
        quantity.attrs.iter().filter(|attribute| attribute.name.text == "access").any(|attribute| self.names_as_path(attribute.value, name))
    }

    /// Whether `expr` is a path whose last segment is `name`.
    fn names_as_path(&self, expr: ExprId, name: &str) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => self.design.path_node(*path).segments.last().is_some_and(|segment| segment.name.text == name),
            _ => false,
        }
    }

    /// Whether the probe `callee(args)` reads a potential through a template (`v(c0.sn)`, or a `let` bound to it): an access name
    /// that names only potentials, whatever nature elaboration finds the node to have.
    fn reads_a_template_potential(&mut self, callee: ExprId, args: &[Arg]) -> bool {
        let Some(first) = args.first() else { return false };
        self.names_only_potentials(callee) && self.reads_through_callable(first.value)
    }

    /// Whether every quantity the access name of `callee` stands for is a potential: the nature does not change the role.
    fn names_only_potentials(&self, callee: ExprId) -> bool {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return false };
        let Some(segment) = self.design.path_node(*path).segments.last() else { return false };
        let mut found = false;
        for i in 0..self.design.decl_count() {
            let decl = DeclId(i as u32);
            let DeclKind::Quantity(quantity) = &self.design.decl_data(decl).kind else { continue };
            if self.is_accessed_as(quantity, &segment.name.text) {
                if quantity.role != QuantityRole::Potential {
                    return false;
                }
                found = true;
            }
        }
        found
    }

    /// Whether `operand` (through indexes) is an item `inst.name` of an instance placed through a callable param, or a `let` bound
    /// to one.
    fn reads_through_callable(&mut self, operand: ExprId) -> bool {
        let mut at = operand;
        while let ExprKind::Index { expr: inner, .. } = &self.design.expr_node(at).kind {
            at = *inner;
        }
        // the read itself, or a `let` bound to it (`let y = c0.sn; v(y)`, `template_items.rs`)
        self.template_read_of(at).is_some()
    }
}
