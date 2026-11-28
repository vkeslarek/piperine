//! Analog forms: probes (`v(a, b)`, `potential`), contributions (`+>`) and the core primitives
//! of the subset (`ddt`, `limit`, `ac_stim` and the environment functions of §14.4).

use crate::refusal::forms::{AnalogForm, ValueForm};
use crate::pom::{Arg, BuiltinId, BuiltinKind, DeclId, DeclKind, DeferReason, ExprId, ExprKind, QuantityRole, Resolution};

use crate::types::catalogue::{Allowed, Core};
use crate::types::check::Context;
use super::AnalogTyping;
use super::nature_rules::NatureRules;
use crate::types::{Callee, Ty};

impl AnalogTyping<'_, '_> {
    /// The statement `lhs +> rhs;`: `lhs` is a probe of a branch (`E0410`), `rhs` is an `f64`
    /// (`E0417`).
    pub(in crate::types) fn contribution(&mut self, file: crate::pom::FileId, span: crate::msl::Span, lhs: ExprId, rhs: ExprId) {
        if self.state.values.context() != Context::Analog {
            self.refuse(file, span, AnalogForm::ContributionOutsideAnalogBlock);
            return;
        }
        let target = self.state.design.expr_node(lhs).clone();
        let is_probe = matches!(&target.kind, ExprKind::Call { class: crate::pom::CallClass::Probe, .. });
        if is_probe {
            self.state.result.exprs[lhs.index()] = self.probe(lhs, true);
        } else {
            let message = "the left side of `+>` is not a potential or flow probe of a branch".to_string();
            self.coded(file, "E0410", target.span, message, "not a probe");
        }
        self.expressions().check_as(rhs, &Ty::F64, "E0417");
    }

    /// `p.inflow()` (part III §6 rule 5): the flow entering the module at its port `p`, of a nature
    /// with `flow`, an `f64`. Anything else is `E0243`; it is a value, so on the left of `+>` it is
    /// `E0410` (`contribution`).
    pub(in crate::types) fn inflow_read(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        if matches!(self.state.values.context(), Context::Function | Context::Const) {
            self.coded(file, "E0450", node.span, "`inflow()` is allowed in an `analog` block and a module body only".to_string(), "core primitive out of place");
        }
        self.expressions().apply(expr, &[], args)?;
        let measured = match self.declared_port(recv) {
            Some((decl, Ty::Node(nature))) => matches!(self.state.design.decl_data(decl).kind, DeclKind::Port(_)) && NatureRules::of(self.state.design, nature).flow,
            _ => false,
        };
        if !measured {
            let message = "`inflow()` applies to a port of a nature with `flow`: it measures the flow entering the module there (§6 rule 5)".to_string();
            self.coded(file, "E0243", node.span, message, "not a port of a nature with `flow`");
            self.expressions().infer(recv);
            return None;
        }
        self.expressions().infer(recv);
        self.state.result.builtins.insert(expr, crate::types::Builtin::Inflow);
        Some(Ty::F64)
    }

    /// A probe `v(a)`, `v(a, b)`, `potential(a, b)` or `flow(a, b)`: its nodes have one nature
    /// (`E0240`). Read as a value only a potential is in the subset; a flow probe is a target.
    pub(in crate::types) fn probe(&mut self, expr: ExprId, target: bool) -> Option<Ty> {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
        let (role, nature) = self.probe_access(*callee, args, node.span, file, target)?;
        // a function probes through a handle (§3 rule 7, §3.1.5); a probe of anything else is out of place
        let operands: Vec<ExprId> = args.iter().map(|arg| arg.value).collect();
        let through_handles = self.state.values.context() == Context::Function && self.modules().probes_only_handles(&operands);
        if matches!(self.state.values.context(), Context::Function | Context::Const) && !through_handles {
            self.coded(file, "E0450", node.span, "a probe is allowed in an `analog` block and a module body only".to_string(), "core primitive out of place");
        }
        if args.is_empty() || args.len() > 2 {
            self.expressions().arity_mismatch(file, node.span, "1 to 2", args);
            return None;
        }
        let mut natures = Vec::new();
        for arg in args {
            match self.expressions().infer(arg.value) {
                Some(Ty::Node(n)) => natures.push(n),
                Some(Ty::AnyNode) => {}
                // a handle generic over a nature: its nature is the instantiation's, known there
                Some(generic) if self.modules().is_nature_parameter(&generic) => {}
                Some(other) => {
                    let span = self.state.design.expr_node(arg.value).span;
                    self.checker.mismatch_text(file, span, "a node", &other.name(self.checker.state.design));
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
        if role == QuantityRole::Flow && !natures.is_empty() && natures.iter().all(|n| !NatureRules::of(self.state.design, *n).flow) {
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
            (QuantityRole::Flow, false) if self.contributes_potential_to(args) || self.state.analog.differentiated > 0 => Some(Ty::F64),
            (QuantityRole::Flow, false) => {
                self.refuse(file, node.span, AnalogForm::FlowProbeReadAsValue);
                None
            }
        }
    }

    /// Judge the place the core primitive `name` is written in against `allowed`, which says where it may stand: a diagnostic
    /// of the place (`E0323`, `E0350`, `E0352` or `E0450`) when it is not one of them.
    fn judge_core_place(&mut self, name: &str, allowed: Allowed, file: crate::pom::FileId, span: crate::msl::Span) {
        match (allowed, self.state.values.context()) {
            (Allowed::Everywhere, Context::Const) => {
                let message = format!("`{name}` is an environment function and is not allowed in a const context");
                self.coded(file, "E0323", span, message, "environment function in a const context");
            }
            (Allowed::Everywhere, _) | (Allowed::Instances, Context::Analog | Context::Module) | (Allowed::Analog, Context::Analog) => {}
            // §8.3: a core module placed in a reaction body is `E0350` as well.
            (Allowed::Instances, Context::Reaction) => {
                let message = format!("`{name}` is a core module and cannot be placed in a reaction body");
                self.coded(file, "E0350", span, message, "placement in a reaction body");
            }
            // §14 errors (§8.3) and §8.3.4: `ddt` and `limit` are core modules, so a placement in
            // a `fn` or a closure is `E0350` and one in a const context is `E0352`.
            (Allowed::Instances, Context::Function) => {
                let message = format!("`{name}` is a core module and cannot be placed in a `fn` or a closure");
                self.coded(file, "E0350", span, message, "placement in a `fn` or a closure");
            }
            (Allowed::Instances, Context::Const) => {
                let message = format!("`{name}` is a core module and cannot be placed in a const context");
                self.coded(file, "E0352", span, message, "placement in a const context");
            }
            (_, context) => {
                let place = match context {
                    Context::Function => "a `fn` or a closure",
                    Context::Reaction => "a reaction body",
                    Context::Const => "a param default, a `where` or a `const`",
                    Context::Module => "a module body",
                    Context::Analog => "an `analog` block",
                };
                self.coded(file, "E0450", span, format!("`{name}` is not allowed in {place}"), "core primitive out of place");
            }
        }
    }

    /// The role (`potential` or `flow`) a probe `callee(args)` reads and the nature its name fixes, when the name fixes one:
    /// read off the quantity the callee names, a built-in access function or, for a name no nature owns, the node it holds
    /// (§6 rule 4). `None` once the probe was refused.
    fn probe_access(&mut self, callee: ExprId, args: &[Arg], span: crate::msl::Span, file: crate::pom::FileId, target: bool) -> Option<(QuantityRole, Option<DeclId>)> {
        let ExprKind::Path(path) = &self.state.design.expr_node(callee).kind else { return None };
        let res = self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let result = match res {
            Resolution::Decl(q) => match &self.state.design.decl_data(q).kind {
                DeclKind::Quantity(data) => (data.role, self.state.design.decl_data(q).parent),
                _ => {
                    self.refuse(file, span, AnalogForm::ProbeThroughThisName);
                    return None;
                }
            },
            Resolution::Builtin(id) if id.info().name == "flow" => (QuantityRole::Flow, None),
            Resolution::Builtin(_) => (QuantityRole::Potential, None),
            // `v(bank.c0.sn)`: the nature of the item is the concrete module's, known at elaboration; a potential access
            // name is read as a potential probe until then
            Resolution::Deferred(DeferReason::AmbiguousAccess) if self.reads_a_template_potential(callee, args) => {
                self.state.result.access_roles.insert(callee, QuantityRole::Potential);
                (QuantityRole::Potential, None)
            }
            // `v(y)` with `let y = x;`: the resolver names no nature for a local, the node it holds does (§6 rule 4); the role is
            // recorded for the elaborator, which reads a probe and a contribution target alike through it
            Resolution::Deferred(DeferReason::AmbiguousAccess) => match self.access_of_operand(callee, args) {
                OperandAccess::Found(role, nature) => {
                    self.state.result.access_roles.insert(callee, role);
                    (role, Some(nature))
                }
                // `v(y)` with `let y = t;`, `t: Thermal`: what `v(t)` is at resolution (§6 rule 4, §6 rule 10, §11 rule 11)
                OperandAccess::Missing(name) => {
                    self.missing_access(file, callee, &name, target);
                    return None;
                }
                // an operand refused already (`E0133` for a chosen node, or a `let` of one) is no second diagnostic
                OperandAccess::NoNode if args.first().is_some_and(|arg| self.modules().operand_refused(arg.value)) => return None,
                OperandAccess::NoNode => {
                    self.refuse(file, span, AnalogForm::ProbeThroughAccessThatNamesSeveralNatures);
                    return None;
                }
            },
            Resolution::Deferred(_) | Resolution::Unresolved => {
                self.refuse(file, span, AnalogForm::ProbeThroughUnresolvedName);
                return None;
            }
        };
        Some(result)
    }

    /// Whether an `analog` block of the module being checked contributes a potential to the branch
    /// `args` name (the same nodes, in the same order).
    fn contributes_potential_to(&self, args: &[Arg]) -> bool {
        let Some(owner) = self.state.values.owner() else { return false };
        let DeclKind::Module(m) = &self.state.design.decl_data(owner).kind else { return false };
        let nodes = |args: &[Arg]| -> Vec<Resolution> {
            args.iter()
                .map(|a| match &self.state.design.expr_node(a.value).kind {
                    ExprKind::Path(path) => self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res),
                    _ => Resolution::Unresolved,
                })
                .collect()
        };
        let wanted = nodes(args);
        if wanted.contains(&Resolution::Unresolved) {
            return false;
        }
        m.behaviors.iter().any(|b| {
            let DeclKind::Behavior(block) = &self.state.design.decl_data(*b).kind else { return false };
            block.kind == crate::pom::BehaviorKind::Analog
                && crate::pom::BodyWalk::new(self.state.design).nested_statements(&block.body).iter().any(|stmt| match &self.state.design.stmt_node(*stmt).kind {
                    crate::pom::StmtKind::Contribution { lhs, .. } => match &self.state.design.expr_node(*lhs).kind {
                        ExprKind::Call { callee, args: lhs_args, class: crate::pom::CallClass::Probe, .. } => {
                            let role = match &self.state.design.expr_node(*callee).kind {
                                ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                                    Some(Resolution::Decl(q)) => match &self.state.design.decl_data(q).kind {
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
    pub(in crate::types) fn core_call(&mut self, expr: ExprId, id: BuiltinId, args: &[Arg]) -> Option<Ty> {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let Some(sig) = Core::signature(id) else {
            let what = if matches!(id.info().kind, BuiltinKind::Fn | BuiltinKind::Module | BuiltinKind::Statement | BuiltinKind::Event) { "core primitive" } else { "built-in" };
            self.refuse_with(file, node.span, ValueForm::PrimitiveWithNoSignatureRow, [what.to_string(), id.name().to_string()]);
            return None;
        };
        let name = id.name();
        if matches!(name.as_str(), "cross" | "at") && self.state.values.event_slot() != Some(expr) {
            self.core_event_out_of_place(expr, &name, args);
            return None;
        }
        self.judge_core_place(&name, sig.allowed, file, node.span);
        self.state.result.callees.insert(expr, Callee::Core(id));
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
            self.expressions().arity_mismatch(file, node.span, expected, args);
            return None;
        }
        let params: Vec<Ty> = sig.required.iter().chain(sig.optional).map(|p| p.ty()).collect();
        let names = Self::core_parameter_names(&name);
        let named: Vec<Option<usize>> = args.iter().map(|a| a.name.as_ref().and_then(|n| names.iter().position(|p| *p == n.text))).collect();
        self.state.modules.begin_placement(None);
        let bound = self.expressions().bind_arguments(expr, &name, &params, sig.required.len(), args, &named);
        self.state.modules.end_placement();
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
        let span = self.state.design.expr_node(expr).span;
        let [value, variable] = args else {
            self.expressions().arity_mismatch(file, span, 2, args);
            return None;
        };
        self.state.analog.differentiated += 1;
        self.expressions().check_argument(value.value, &Ty::F64);
        self.state.analog.differentiated -= 1;
        let probe_call = matches!(&self.state.design.expr_node(variable.value).kind, ExprKind::Call { class: crate::pom::CallClass::Probe, .. });
        if !probe_call {
            let at = self.state.design.expr_node(variable.value).span;
            self.coded(file, "E0451", at, "the second argument of `ddx` must be a probe call of a potential or a flow of a branch of this module (§14.2.2); `p.inflow()` is not accepted".to_string(), "not a probe call");
            self.expressions().infer(variable.value);
            return None;
        }
        self.state.result.exprs[variable.value.index()] = self.probe(variable.value, true);
        Some(Ty::F64)
    }

    /// `analysis(kind)` (§14.4): a `bool`. `Static` is true in an equilibrium solve; `Dc`,
    /// `Transient`, `Ac` and `Noise` tell which analysis runs. Any other argument is `P0001`
    /// naming `analysis`.
    fn analysis_call(&mut self, expr: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let [arg] = args else {
            self.expressions().arity_mismatch(file, span, 1, args);
            return None;
        };
        let kind = match &self.state.design.expr_node(arg.value).kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) => Some(id.name()),
                _ => None,
            },
            _ => None,
        };
        if !matches!(kind.as_deref(), Some("Static" | "Dc" | "Transient" | "Ac" | "Noise")) {
            self.refuse(file, span, AnalogForm::AnalysisOfKindOtherThanStaticDc);
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

impl AnalogTyping<'_, '_> {
    /// The quantity an access name `callee` (`v`) names in the nature of the node its first operand holds: a `let` bound to a node
    /// keeps the node's nature, so `v(y)` reads the potential of `y`'s nature.
    fn access_of_operand(&mut self, callee: ExprId, args: &[Arg]) -> OperandAccess {
        let Some(first) = args.first() else { return OperandAccess::NoNode };
        let Some(Ty::Node(nature)) = self.expressions().infer(first.value) else { return OperandAccess::NoNode };
        let Some(name) = self.access_name(callee) else { return OperandAccess::NoNode };
        match self.access_role_in(nature, &name) {
            Some(role) => OperandAccess::Found(role, nature),
            None => OperandAccess::Missing(name),
        }
    }

    /// The access name the callee `callee` of a probe writes (`v`, `i`).
    fn access_name(&self, callee: ExprId) -> Option<String> {
        let ExprKind::Path(path) = &self.state.design.expr_node(callee).kind else { return None };
        self.state.design.path_node(*path).segments.last().map(|segment| segment.name.text.clone())
    }

    /// The role of the quantity of `nature` that declares the access `name`, `None` when it declares none.
    fn access_role_in(&self, nature: DeclId, name: &str) -> Option<QuantityRole> {
        self.state.design.decl_data(nature).children.iter().find_map(|child| match &self.state.design.decl_data(*child).kind {
            DeclKind::Quantity(quantity) if self.is_accessed_as(quantity, name) => Some(quantity.role),
            _ => None,
        })
    }

    /// The diagnostic of the access `name` (the callee `callee`) that the nature of the probed node lacks, as resolution gives it for
    /// a node named directly: a flow access contributed to on a nature without `flow` is `E0413`, any other is `E0206`.
    fn missing_access(&mut self, file: crate::pom::FileId, callee: ExprId, name: &str, target: bool) {
        let span = self.state.design.expr_node(callee).span;
        let flow = (0..self.state.design.decl_count()).any(|index| match &self.state.design.decl_data(DeclId(index as u32)).kind {
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
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Path(path) => self.state.design.path_node(*path).segments.last().is_some_and(|segment| segment.name.text == name),
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
        let ExprKind::Path(path) = &self.state.design.expr_node(callee).kind else { return false };
        let Some(segment) = self.state.design.path_node(*path).segments.last() else { return false };
        let mut found = false;
        for i in 0..self.state.design.decl_count() {
            let decl = DeclId(i as u32);
            let DeclKind::Quantity(quantity) = &self.state.design.decl_data(decl).kind else { continue };
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
        while let ExprKind::Index { expr: inner, .. } = &self.state.design.expr_node(at).kind {
            at = *inner;
        }
        // the read itself, or a `let` bound to it (`let y = c0.sn; v(y)`, `template_items.rs`)
        self.modules().template_read_of(at).is_some()
    }
}
