//! The `analog` block of a body: its statements lowered in source order. Probes, environment
//! reads and sites are lowered here; the contribution to a branch is `contribute.rs`.

use piperine_ir::{AnalysisKind, Branch, BranchKind, Env, Expr, ExprId as LoweredExpr, Origin, Site, StmtId as LoweredStmt, Terminal, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::nested_stmts;
use crate::types::Callee;
use crate::pom::{BehaviorKind, BuiltinId, BuiltinKind, DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtKind};

impl Elaboration<'_, '_> {
    /// The statements of the `analog` block bound to `module`, in source order; `None` when it
    /// has none. Every `analog` block of the module is lowered in source order into the one body
    /// (§10 rule 7); each binding is its own local, so two blocks keep their names apart.
    pub(super) fn lower_analog(&mut self, scope: &mut Scope, module: DeclId) -> Option<Vec<LoweredStmt>> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        let blocks = data.behaviors.clone();
        let mut stmts = None;
        for block in blocks {
            let decl = self.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            // A `digital` block is lowered as reactions (`events.rs`).
            if behavior.kind != BehaviorKind::Analog {
                continue;
            }
            // After a diagnostic the block is empty: no `Ir` is built anyway.
            scope.analog = true;
            scope.in_analog_block = true;
            let (mut lowered, value) = self.lower_statements(scope, &behavior.body).unwrap_or_default();
            scope.in_analog_block = false;
            scope.analog = false;
            // At the top of the block a trailing expression without `;` is a statement like any other.
            if let Some(value) = value {
                lowered.push(self.stmt(piperine_ir::Stmt::Expr(value)));
            }
            stmts.get_or_insert_with(Vec::new).extend(lowered);
        }
        // The `bound_step` calls met under a condition or in a nested block go last, at the top.
        let steps = std::mem::take(&mut scope.hoisted_steps);
        if !steps.is_empty() {
            stmts.get_or_insert_with(Vec::new).extend(steps);
        }
        stmts
    }

    /// A potential probe `v(a)` or `v(a, b)`: a `Probe` over the terminals of its nodes (one
    /// node is a probe against ground).
    pub(super) fn lower_probe(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
        // The type checker lets a module read the flow of a branch it drives with a potential.
        let flow = match &self.design.expr_node(*callee).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(q)) => matches!(&self.design.decl_data(q).kind, DeclKind::Quantity(data) if data.role == crate::pom::QuantityRole::Flow),
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
                self.unsupported(scope.file, node.span, "a probe of more than two nodes");
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
    /// A branch with no potential contribution is `P0001` (a flow branch has no flow read yet).
    fn lower_flow_read(&mut self, scope: &mut Scope, expr: ExprId, plus: Terminal, minus: Terminal, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        let Some((branch, reversed)) = self.potential_branch(scope, plus, minus) else {
            self.unsupported(scope.file, span, "a flow probe of a branch without a potential contribution");
            return None;
        };
        let flow = self.emit(Expr::Flow { branch }, ValueType::F64, origin.clone());
        if !reversed {
            return Some(flow);
        }
        Some(self.emit(Expr::Unary { op: piperine_ir::UnaryOp::Neg, expr: flow }, ValueType::F64, origin))
    }

    /// The potential branch between `plus` and `minus`, and whether it is named the other way
    /// round. A branch is numbered at its first contribution; a flow read written before that
    /// contribution numbers it from the potential contribution the module's `analog` block
    /// writes, wherever it sits in it (under an `if`, in a nested block).
    pub(super) fn potential_branch(&mut self, scope: &mut Scope, plus: Terminal, minus: Terminal) -> Option<(u32, bool)> {
        let same = |b: &Branch| (b.plus, b.minus) == (plus, minus) || (b.plus, b.minus) == (minus, plus);
        let existing = scope.branches.iter().position(same);
        if let Some(at) = existing
            && scope.branches[at].kind == BranchKind::Potential
        {
            return Some((at as u32, scope.branches[at].plus != plus));
        }
        let (p, m) = self.written_potential(scope, plus, minus)?;
        match existing {
            Some(at) => Some((at as u32, scope.branches[at].plus != plus)),
            None => {
                scope.branches.push(Branch { plus: p, minus: m, kind: BranchKind::Potential });
                Some((scope.branches.len() as u32 - 1, p != plus))
            }
        }
    }

    /// The ends, as the module's `analog` block writes them, of a potential contribution to the
    /// branch between `plus` and `minus`; `None` when the block has none.
    fn written_potential(&mut self, scope: &Scope, plus: Terminal, minus: Terminal) -> Option<(Terminal, Terminal)> {
        let module = self.frames.get(scope.frame)?.module;
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        for block in data.behaviors.clone() {
            let DeclKind::Behavior(behavior) = &self.design.decl_data(block).kind.clone() else { continue };
            if behavior.kind != BehaviorKind::Analog {
                continue;
            }
            for stmt in nested_stmts(self.design, &behavior.body) {
                let StmtKind::Contribution { lhs, .. } = self.design.stmt_node(stmt).kind.clone() else { continue };
                let ExprKind::Call { callee, args, .. } = self.design.expr_node(lhs).kind.clone() else { continue };
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
    pub(super) fn lower_core(&mut self, scope: &mut Scope, expr: ExprId, id: BuiltinId, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match id.info().name {
            "temperature" => Some(self.emit(Expr::Env(Env::Temperature), ty, origin)),
            "gmin" => Some(self.emit(Expr::Env(Env::Gmin), ty, origin)),
            "source_scale" => Some(self.emit(Expr::Env(Env::SourceScale), ty, origin)),
            "now" => Some(self.emit(Expr::Env(Env::Time), ty, origin)),
            // The analog block reads the sweep's frequency; a function reads its caller's, a reaction sees 0 (§14.4.4).
            "freq" if scope.analog => Some(self.emit(Expr::Env(Env::Freq), ty, origin)),
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
            name => {
                self.unsupported(scope.file, span, &format!("core primitive `{name}`"));
                None
            }
        }
    }

    /// The name of the built-in `expr` is a path to (`Static`, `Dc`, ...).
    fn builtin_name(&self, expr: ExprId) -> Option<String> {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
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
        let span = self.design.expr_node(expr).span;
        if !self.core_site_allowed(scope, span, "limit") {
            return None;
        }
        let enabled = self.standalone_guard(scope, span, "a `limit`")?;
        let site = scope.limits.len() as u32;
        let name = match scope.limit_name.take() {
            Some((at, name)) if at == expr => name,
            _ => format!("limit{site}"),
        };
        scope.limits.push(Site { name, origin: origin.clone(), enabled });
        let (x, f, seed) = match enabled {
            None => {
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.lower_expr(scope, *arg));
                }
                let mut lowered = lowered.into_iter();
                (lowered.next()??, lowered.next()??, lowered.next().flatten())
            }
            Some(_) => {
                // `f` is a closure whose parameters are locals of the body: it stays where it is.
                let mut standalone = Self::standalone_scope(scope);
                let x = self.lower_expr(&mut standalone, *args.first()?);
                let f = self.lower_expr(scope, *args.get(1)?);
                let seed = match args.get(2) {
                    Some(seed) => Some(self.lower_expr(&mut standalone, *seed)),
                    None => None,
                };
                if !standalone.locals.is_empty() {
                    self.unsupported(scope.file, span, "a guarded `limit` whose input binds a local");
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
        let span = self.design.expr_node(expr).span;
        if !self.core_site_allowed(scope, span, "ac_stim") {
            return None;
        }
        let site = scope.stims.len() as u32;
        scope.stims.push(Site { name: "ac".to_string(), origin: origin.clone(), enabled: None });
        let guarded = !scope.guards.is_empty();
        let mut standalone = Self::standalone_scope(scope);
        let mut lowered = Vec::new();
        for arg in args {
            let value = if guarded { self.lower_expr(&mut standalone, *arg) } else { self.lower_expr(scope, *arg) };
            lowered.push(value);
        }
        if !standalone.locals.is_empty() {
            self.unsupported(scope.file, span, "a guarded `ac_stim` whose input binds a local");
            return None;
        }
        let mut lowered = lowered.into_iter();
        let (mag, phase) = (lowered.next()??, lowered.next().flatten());
        Some(self.emit(Expr::Stim { site, mag, phase }, ty, origin))
    }

    /// Whether a `limit` or `ac_stim` may stand where `scope` is: not inside an `Option` method
    /// (`P0001`; a closure is `E0350` from the type checker before this runs).
    fn core_site_allowed(&mut self, scope: &Scope, span: crate::msl::Span, name: &str) -> bool {
        if scope.nesting > 0 {
            self.unsupported(scope.file, span, &format!("a `{name}` inside an `Option` method"));
            return false;
        }
        true
    }

    /// When `expr`, a statement of a block, is `bound_step(dt)` (§14.10): a `BoundStep` appended
    /// to `out`, and `Some(false)` after a diagnostic (a `bound_step` under an `if`, a `match` or
    /// in a nested block is not lowered). `None` for any other expression.
    pub(super) fn lower_bound_step(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, expr: ExprId) -> Option<bool> {
        let expr = self.peel_parens(expr);
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Call { args, .. } = &node.kind else { return None };
        match self.types.callee(expr) {
            Some(Callee::Core(id)) if id.info().name == "bound_step" => {}
            _ => return None,
        }
        if scope.nesting > 0 {
            self.unsupported(scope.file, node.span, "a `bound_step` inside a closure or an `Option` method");
            return Some(false);
        }
        let [arg] = &args[..] else { return Some(false) };
        // Under a condition or in a nested block the call moves to the top of the analog block with
        // `when`, the conjunction of its conditions (§14.10.2), its step lowered standalone.
        let nested = !scope.guards.is_empty() || scope.block_depth > 0;
        let when = if nested { self.standalone_guard(scope, node.span, "a `bound_step`") } else { Some(None) };
        let Some(when) = when else { return Some(false) };
        let mut standalone = Self::standalone_scope(scope);
        let value = if nested { self.lower_expr(&mut standalone, arg.value) } else { self.lower_expr(scope, arg.value) }?;
        if !standalone.locals.is_empty() {
            self.unsupported(scope.file, node.span, "a `bound_step` under a condition whose step binds a local");
            return Some(false);
        }
        let site = scope.bounds;
        scope.bounds += 1;
        let stmt = self.stmt(piperine_ir::Stmt::BoundStep { site, value, when });
        if nested {
            scope.hoisted_steps.push(stmt);
        } else {
            out.push(stmt);
        }
        Some(true)
    }

    /// A node argument: a port or signal of the body, or ground.
    pub(super) fn lower_terminal(&mut self, scope: &Scope, arg: ExprId) -> Option<Terminal> {
        let node = self.design.expr_node(arg).clone();
        let file = self.design.decl_data(node.owner).file;
        if let ExprKind::Field { .. } = &node.kind {
            return self.reached_terminal(scope, arg);
        }
        let ExprKind::Path(path) = &node.kind else {
            self.unsupported(file, node.span, "a node argument that is not a name");
            return None;
        };
        match self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            // a handle is the node the caller passed (`handles.rs`)
            Resolution::Decl(decl) if let Some(bound) = scope.handles.get(&decl).copied() => self.lower_terminal(scope, bound),
            Resolution::Decl(decl) => match self.frame_node(scope.frame, decl) {
                Some(terminal) => Some(terminal),
                None => {
                    self.unsupported(file, node.span, "a name that is not a node of this module");
                    None
                }
            },
            Resolution::Builtin(builtin) if builtin.info().kind == BuiltinKind::Node => Some(Terminal::Ground),
            _ => {
                self.unsupported(file, node.span, "a node argument that is not a node");
                None
            }
        }
    }
}
