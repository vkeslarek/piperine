//! The `analog` block of a body: its statements lowered in source order. Probes, environment
//! reads and sites are lowered here; the contribution to a branch is `contribute.rs`.

use super::AnalogLowering;
use piperine_ir::{AnalysisKind, Branch, BranchKind, Env, Expr, ExprId as LoweredExpr, Origin, Site, StmtId as LoweredStmt, Terminal, ValueType};

use crate::elab::elaborator::Scope;
use crate::refusal::forms::AnalogForm;
use crate::types::Callee;
use crate::pom::{BehaviorKind, BuiltinId, BuiltinKind, DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtKind};

impl AnalogLowering<'_, '_, '_> {
    /// The statements of the `analog` block bound to `module`, in source order; `None` when it
    /// has none. Every `analog` block of the module is lowered in source order into the one body
    /// (§10 rule 7); each binding is its own local, so two blocks keep their names apart.
    pub(in crate::elab) fn lower_analog(&mut self, scope: &mut Scope, module: DeclId) -> Option<Vec<LoweredStmt>> {
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return None };
        let blocks = data.behaviors.clone();
        let mut stmts = None;
        for block in blocks {
            let decl = self.state.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            // A `digital` block is lowered as reactions (`events.rs`).
            if behavior.kind != BehaviorKind::Analog {
                continue;
            }
            // After a diagnostic the block is empty: no `Ir` is built anyway.
            scope.analog.in_analog_code = true;
            scope.analog.in_analog_block = true;
            let (mut lowered, value) = self.values().lower_statements(scope, &behavior.body).unwrap_or_default();
            scope.analog.in_analog_block = false;
            scope.analog.in_analog_code = false;
            // At the top of the block a trailing expression without `;` is a statement like any other.
            if let Some(value) = value {
                lowered.push(self.stmt(piperine_ir::Stmt::Expr(value)));
            }
            stmts.get_or_insert_with(Vec::new).extend(lowered);
        }
        // The `bound_step` calls met under a condition or in a nested block go last, at the top.
        let steps = std::mem::take(&mut scope.analog.hoisted_steps);
        if !steps.is_empty() {
            stmts.get_or_insert_with(Vec::new).extend(steps);
        }
        stmts
    }

    /// A potential probe `v(a)` or `v(a, b)`: a `Probe` over the terminals of its nodes (one
    /// node is a probe against ground).
    pub(in crate::elab) fn lower_probe(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(expr).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
        // The type checker lets a module read the flow of a branch it drives with a potential.
        let flow = match &self.state.design.expr_node(*callee).kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(q)) => matches!(&self.state.design.decl_data(q).kind, DeclKind::Quantity(data) if data.role == crate::pom::QuantityRole::Flow),
                Some(Resolution::Builtin(id)) => id.info().name == "flow",
                _ => false,
            },
            _ => false,
        };
        let mut terminals = Vec::new();
        for arg in args {
            terminals.push(self.lower_terminal(scope, arg.value)?);
        }
        let (plus, minus) = match terminals[..] {
            [plus] => (plus, Terminal::Ground),
            [plus, minus] => (plus, minus),
            _ => {
                self.refuse(scope.values.file, node.span, AnalogForm::ProbeOfMoreThanTwoNodes);
                return None;
            }
        };
        if flow {
            return self.lower_flow_read(scope, expr, plus, minus, origin);
        }
        Some(self.emit(Expr::Probe { plus, minus }, ty, origin))
    }

    /// `i(a, b)` read as a value: the flow of the branch `(a, b)` the module gives a potential
    /// contribution (amendment A5), negated when the read names the branch the other way round.
    /// A branch with no potential contribution is `P0001` (a flow branch has no flow read yet), except in
    /// a value that is differentiated, where it is a leaf (`flow_leaf_branch`).
    fn lower_flow_read(&mut self, scope: &mut Scope, expr: ExprId, plus: Terminal, minus: Terminal, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        let (branch, reversed) = match self.potential_branch(scope, plus, minus) {
            Some(found) => found,
            None if scope.analog.differentiated => self.flow_leaf_branch(scope, plus, minus),
            None => {
                self.refuse(scope.values.file, span, AnalogForm::FlowProbeOfBranchWithoutPotentialContribution);
                return None;
            }
        };
        let flow = self.emit(Expr::Flow { branch }, ValueType::F64, origin.clone());
        if !reversed {
            return Some(flow);
        }
        Some(self.emit(Expr::Unary { op: piperine_ir::UnaryOp::Neg, expr: flow }, ValueType::F64, origin))
    }

    /// The branch between `plus` and `minus`, whether it is named the other way round, for the flow a `ddx` reads as a
    /// leaf of its derivative though the module gives the branch no potential contribution: the branch a flow
    /// contribution made, or a flow branch the read makes. The derivative of the leaf against itself is 1 (§14.2.3),
    /// and the leaf is recorded in `flow_leaves`, since the value of such a flow is not read anywhere yet.
    pub(super) fn flow_leaf_branch(&mut self, scope: &mut Scope, plus: Terminal, minus: Terminal) -> (u32, bool) {
        let existing = scope.analog.branches.iter().position(|b| (b.plus, b.minus) == (plus, minus) || (b.plus, b.minus) == (minus, plus));
        let (branch, reversed) = match existing {
            Some(at) => (at as u32, scope.analog.branches[at].plus != plus),
            None => {
                scope.analog.branches.push(Branch { plus, minus, kind: BranchKind::Flow });
                (scope.analog.branches.len() as u32 - 1, false)
            }
        };
        scope.analog.flow_leaves.push(branch);
        (branch, reversed)
    }

    /// The potential branch between `plus` and `minus`, and whether it is named the other way
    /// round. A branch is numbered at its first contribution; a flow read written before that
    /// contribution numbers it from the potential contribution the module's `analog` block
    /// writes, wherever it sits in it (under an `if`, in a nested block).
    pub(super) fn potential_branch(&mut self, scope: &mut Scope, plus: Terminal, minus: Terminal) -> Option<(u32, bool)> {
        let same = |b: &Branch| (b.plus, b.minus) == (plus, minus) || (b.plus, b.minus) == (minus, plus);
        let existing = scope.analog.branches.iter().position(same);
        if let Some(at) = existing
            && scope.analog.branches[at].kind == BranchKind::Potential
        {
            return Some((at as u32, scope.analog.branches[at].plus != plus));
        }
        let (p, m) = self.written_potential(scope, plus, minus)?;
        match existing {
            Some(at) => Some((at as u32, scope.analog.branches[at].plus != plus)),
            None => {
                scope.analog.branches.push(Branch { plus: p, minus: m, kind: BranchKind::Potential });
                Some((scope.analog.branches.len() as u32 - 1, p != plus))
            }
        }
    }

    /// The ends, as the module's `analog` block writes them, of a potential contribution to the
    /// branch between `plus` and `minus`; `None` when the block has none.
    fn written_potential(&mut self, scope: &Scope, plus: Terminal, minus: Terminal) -> Option<(Terminal, Terminal)> {
        let module = self.state.structure.frames.get(scope.values.frame)?.module;
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return None };
        for block in data.behaviors.clone() {
            let DeclKind::Behavior(behavior) = &self.state.design.decl_data(block).kind.clone() else { continue };
            if behavior.kind != BehaviorKind::Analog {
                continue;
            }
            for stmt in crate::pom::BodyWalk::new(self.state.design).nested_statements(&behavior.body) {
                let StmtKind::Contribution { lhs, .. } = self.state.design.stmt_node(stmt).kind.clone() else { continue };
                let ExprKind::Call { callee, args, .. } = self.state.design.expr_node(lhs).kind.clone() else { continue };
                let potential = self.quantity_role(callee) == Some(crate::pom::QuantityRole::Potential);
                if !potential {
                    continue;
                }
                let ends: Option<Vec<Terminal>> = args.iter().map(|a| self.lower_terminal(scope, a.value)).collect();
                let (p, m) = match ends.as_deref() {
                    Some([p]) => (*p, Terminal::Ground),
                    Some([p, m]) => (*p, *m),
                    _ => continue,
                };
                if (p, m) == (plus, minus) || (p, m) == (minus, plus) {
                    return Some((p, m));
                }
            }
        }
        None
    }

    /// A call of a core primitive of the subset: an environment read, a `limit` or an `ac_stim`
    /// site. A site is numbered when it is met, so numbering follows the source.
    pub(in crate::elab) fn lower_core(&mut self, scope: &mut Scope, expr: ExprId, id: BuiltinId, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        match id.info().name {
            "temperature" => Some(self.lower_temperature(origin)),
            "gmin" => Some(self.emit(Expr::Env(Env::Gmin), ty, origin)),
            "source_scale" => Some(self.emit(Expr::Env(Env::SourceScale), ty, origin)),
            "now" => Some(self.emit(Expr::Env(Env::Time), ty, origin)),
            // The analog block reads the sweep's frequency; a function reads its caller's, a reaction sees 0 (§14.4.4).
            "freq" if scope.analog.in_analog_code => Some(self.emit(Expr::Env(Env::Freq), ty, origin)),
            "freq" => Some(self.emit(Expr::Real(piperine_ir::Real::new(0.0)), ty, origin)),
            // The checker lets `analysis` take `Static` or a kind of analysis.
            "analysis" => {
                let kind = args.first().and_then(|a| self.builtin_name(*a));
                let env = match kind.as_deref() {
                    Some("Dc") => Env::Analysis(AnalysisKind::Dc),
                    Some("Transient") => Env::Analysis(AnalysisKind::Transient),
                    Some("Ac") => Env::Analysis(AnalysisKind::Ac),
                    Some("Noise") => Env::Analysis(AnalysisKind::Noise),
                    _ => Env::Static,
                };
                Some(self.emit(Expr::Env(env), ty, origin))
            }
            "limit" => self.lower_limit(scope, expr, args, ty, origin),
            "ac_stim" => self.lower_stim(scope, expr, args, ty, origin),
            "ddt" => self.lower_derivative(scope, expr, args, origin),
            "ddx" => self.lower_ddx(scope, expr, args, origin),
            "delay" => self.lower_delay(scope, expr, args, origin),
            "noise" => self.lower_noise(scope, expr, args, origin),
            "simparam" => self.lower_simparam(scope, args, origin),
            name => {
                self.refuse_with(scope.values.file, span, AnalogForm::CorePrimitive, [name.to_string()]);
                None
            }
        }
    }

    /// The name of the built-in `expr` is a path to (`Static`, `Dc`, ...).
    fn builtin_name(&self, expr: ExprId) -> Option<String> {
        let ExprKind::Path(path) = &self.state.design.expr_node(expr).kind else { return None };
        match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Builtin(id)) => Some(id.name()),
            _ => None,
        }
    }

    /// A `limit` read as a value. Under a condition (an `if` or `match` arm, the right of `&&` or
    /// `||`) it carries `enabled`, the conjunction of the enclosing conditions lowered standalone,
    /// and its `x` and `seed` are lowered standalone too (§14.7.4, §14 rule 9): the solver's `limit`
    /// function evaluates them whatever control flow surrounds the call. In a closure it is `E0350`
    /// from the type checker (§8.3.4); in any other nested form (an `Option` method) it is `P0001`.
    fn lower_limit(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        if !self.core_site_allowed(scope, span, "limit") {
            return None;
        }
        // read at a converged solution, `limit(x, f, seed)` is `x`: the limited iterate is the unlimited one
        if scope.analog.at_solution {
            return self.values().lower_expr(scope, *args.first()?);
        }
        let enabled = self.standalone_guard(scope, span, "a `limit`")?;
        let site = scope.analog.limits.len() as u32;
        let name = match scope.analog.limit_name.take() {
            Some((at, name)) if at == expr => name,
            _ => format!("limit{site}"),
        };
        scope.analog.limits.push(Site { name, origin: origin.clone(), enabled });
        let (x, f, seed) = match enabled {
            None => {
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.values().lower_expr(scope, *arg));
                }
                let mut lowered = lowered.into_iter();
                (lowered.next()??, lowered.next()??, lowered.next().flatten())
            }
            Some(_) => {
                // `f` is a closure whose parameters are locals of the body: it stays where it is.
                let mut standalone = Self::standalone_scope(scope);
                let x = self.values().lower_expr(&mut standalone, *args.first()?);
                let f = self.values().lower_expr(scope, *args.get(1)?);
                let seed = match args.get(2) {
                    Some(seed) => Some(self.values().lower_expr(&mut standalone, *seed)),
                    None => None,
                };
                if !standalone.values.locals.is_empty() {
                    self.refuse(scope.values.file, span, AnalogForm::GuardedLimitWhoseInputBindsLocal);
                    return None;
                }
                let seed = match seed {
                    Some(seed) => Some(seed?),
                    None => None,
                };
                (x?, f?, seed)
            }
        };
        Some(self.emit(Expr::Limit { site, x, f, seed }, ty, origin))
    }

    /// An `ac_stim` read as a value. Under a condition its magnitude and phase are lowered
    /// standalone; it needs no enable, since a term in an arm not taken does not read the stimulus
    /// (AS-06 AC 4).
    fn lower_stim(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        if !self.core_site_allowed(scope, span, "ac_stim") {
            return None;
        }
        let site = scope.analog.stims.len() as u32;
        // the first site is the analysis `ac`; a later one is numbered, since the backends print a site by its name
        let name = if site == 0 { "ac".to_string() } else { format!("ac{site}") };
        scope.analog.stims.push(Site { name, origin: origin.clone(), enabled: None });
        let guarded = !scope.analog.guards.is_empty();
        let mut standalone = Self::standalone_scope(scope);
        let mut lowered = Vec::new();
        for arg in args {
            let value = if guarded { self.values().lower_expr(&mut standalone, *arg) } else { self.values().lower_expr(scope, *arg) };
            lowered.push(value);
        }
        if guarded {
            scope.adopt_standalone(&mut standalone);
        }
        if !standalone.values.locals.is_empty() {
            self.refuse(scope.values.file, span, AnalogForm::GuardedAcStimWhoseInputBindsLocal);
            return None;
        }
        let mut lowered = lowered.into_iter();
        let (mag, phase) = (lowered.next()??, lowered.next().flatten());
        let stimulus = self.emit(Expr::Stim { site, mag, phase }, ty, origin);
        if scope.analog.value_input {
            // the site function is found in a statement of the block: the value input reads the site, this writes it
            let write = self.stmt(piperine_ir::Stmt::Expr(stimulus));
            scope.analog.hoisted_steps.push(write);
        }
        Some(stimulus)
    }

    /// Whether a `limit` or `ac_stim` may stand where `scope` is: not inside an `Option` method
    /// (`P0001`; a closure is `E0350` from the type checker before this runs).
    fn core_site_allowed(&mut self, scope: &Scope, span: crate::msl::Span, name: &str) -> bool {
        if scope.analog.nesting > 0 {
            self.refuse_with(scope.values.file, span, AnalogForm::InsideOptionMethod, [name.to_string()]);
            return false;
        }
        true
    }

    /// When `expr`, a statement of a block, is `bound_step(dt)` (§14.10): a `BoundStep` appended
    /// to `out`, and `Some(false)` after a diagnostic (a `bound_step` under an `if`, a `match` or
    /// in a nested block is not lowered). `None` for any other expression.
    pub(in crate::elab) fn lower_bound_step(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, expr: ExprId) -> Option<bool> {
        let expr = self.peel_parens(expr);
        let node = self.state.design.expr_node(expr).clone();
        let ExprKind::Call { args, .. } = &node.kind else { return None };
        match self.state.types.callee(expr) {
            Some(Callee::Core(id)) if id.info().name == "bound_step" => {}
            _ => return None,
        }
        if scope.analog.nesting > 0 {
            self.refuse(scope.values.file, node.span, AnalogForm::BoundStepInsideClosureOrOptionMethod);
            return Some(false);
        }
        let [arg] = &args[..] else { return Some(false) };
        // Under a condition or in a nested block the call moves to the top of the analog block with
        // `when`, the conjunction of its conditions (§14.10.2), its step lowered standalone.
        let nested = !scope.analog.guards.is_empty() || scope.analog.block_depth > 0;
        let when = if nested { self.standalone_guard(scope, node.span, "a `bound_step`") } else { Some(None) };
        let Some(when) = when else { return Some(false) };
        let mut standalone = Self::standalone_scope(scope);
        let value = if nested { self.values().lower_expr(&mut standalone, arg.value) } else { self.values().lower_expr(scope, arg.value) }?;
        if !standalone.values.locals.is_empty() {
            self.refuse(scope.values.file, node.span, AnalogForm::BoundStepUnderConditionWhoseStepBinds);
            return Some(false);
        }
        let site = scope.analog.bounds;
        scope.analog.bounds += 1;
        let stmt = self.stmt(piperine_ir::Stmt::BoundStep { site, value, when });
        if nested {
            scope.analog.hoisted_steps.push(stmt);
        } else {
            out.push(stmt);
        }
        Some(true)
    }

    /// A node argument: a port or signal of the body, or ground; parentheses around it name the same node (`v((x))`).
    pub(in crate::elab) fn lower_terminal(&mut self, scope: &Scope, arg: ExprId) -> Option<Terminal> {
        let arg = self.peel_parens(arg);
        // `if k > 0.5 { x } else { p }`: the node the choice folds to at elaboration (`node_choice.rs`)
        if let Some(chosen) = self.chosen_node(scope, arg) {
            return self.lower_terminal(scope, chosen?);
        }
        let node = self.state.design.expr_node(arg).clone();
        let file = self.state.design.decl_data(node.owner).file;
        if let ExprKind::Field { .. } = &node.kind {
            // a node field of an element of an array of bundles (`d[k].p`) is that node of its leaf (struct of arrays, `bundles.rs`)
            if self.bundle_leaf(scope, arg).is_some_and(|place| !place.elements.is_empty()) {
                return self.node_element(scope, arg)?.or_else(|| {
                    self.refuse(file, node.span, AnalogForm::FieldOfElementOfArrayOfBundles);
                    None
                });
            }
            return self.reached_terminal(scope, arg);
        }
        if let ExprKind::Index { .. } = &node.kind {
            // one node of an array of nodes: its index folds, per iteration of an unrolled loop (CV-26)
            return self.node_element(scope, arg)?.or_else(|| {
                self.refuse(file, node.span, AnalogForm::IndexOfValueThatIsNoArray);
                None
            });
        }
        let ExprKind::Path(path) = &node.kind else {
            self.refuse(file, node.span, AnalogForm::NodeArgumentThatIsNotName);
            return None;
        };
        match self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            // a handle is the node the caller passed (`handles.rs`)
            Resolution::Decl(decl) if let Some(bound) = scope.digital.handles.get(&decl).copied() => self.lower_terminal(scope, bound),
            // a node declared in generation: the iteration's element, or the arm was not elaborated (`generation.rs`)
            Resolution::Decl(decl) if self.state.structure.generated.of(decl).is_some_and(|iterations| iterations.is_empty() || self.state.types.node_array(decl).is_some()) && matches!(self.state.design.decl_data(decl).kind, DeclKind::Signal(_)) => {
                if self.state.structure.generated.of(decl).is_some_and(Vec::is_empty) {
                    self.refer_outside_generation(file, node.span, decl, None);
                    return None;
                }
                self.node_element(scope, arg)?.or_else(|| {
                    self.refuse(file, node.span, AnalogForm::GeneratedNodeOutsideGenerationThatDeclaresIt);
                    None
                })
            }
            Resolution::Decl(decl) => match self.frame_node(scope.values.frame, decl) {
                Some(terminal) => Some(terminal),
                None => {
                    self.refuse(file, node.span, AnalogForm::NameThatIsNotNodeOfThis);
                    None
                }
            },
            Resolution::Builtin(builtin) if builtin.info().kind == BuiltinKind::Node => Some(Terminal::Ground),
            _ => {
                self.refuse(file, node.span, AnalogForm::NodeArgumentThatIsNotNode);
                None
            }
        }
    }
}
