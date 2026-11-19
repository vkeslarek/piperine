//! The `analog` block of a body: its statements lowered in source order. Probes, environment
//! reads and sites are lowered here; the contribution to a branch is `contribute.rs`.

use piperine_ir::{AnalysisKind, Branch, BranchKind, Env, Expr, ExprId as LoweredExpr, Origin, Site, StmtId as LoweredStmt, Terminal, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use super::unlowered::Unlowered;
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
            let (mut lowered, value) = self.lower_statements(scope, &behavior.body).unwrap_or_default();
            scope.analog = false;
            // At the top of the block a trailing expression without `;` is a statement like any other.
            if let Some(value) = value {
                lowered.push(self.stmt(piperine_ir::Stmt::Expr(value)));
            }
            stmts.get_or_insert_with(Vec::new).extend(lowered);
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
    fn potential_branch(&mut self, scope: &mut Scope, plus: Terminal, minus: Terminal) -> Option<(u32, bool)> {
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
                let potential = match &self.design.expr_node(callee).kind {
                    ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                        Some(Resolution::Decl(q)) => matches!(&self.design.decl_data(q).kind, DeclKind::Quantity(data) if data.role == crate::pom::QuantityRole::Potential),
                        Some(Resolution::Builtin(id)) => id.info().name == "potential",
                        _ => false,
                    },
                    _ => false,
                };
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
            "limit" if scope.nesting == 0 => {
                let site = scope.limits.len() as u32;
                let name = match scope.limit_name.take() {
                    Some((at, name)) if at == expr => name,
                    _ => format!("limit{site}"),
                };
                scope.limits.push(Site { name, origin: origin.clone(), enabled: None });
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.lower_expr(scope, *arg).and_then(|v| self.known_in_analog(scope.file, *arg, v, "an argument of `limit`")));
                }
                let mut lowered = lowered.into_iter();
                let (x, f, seed) = (lowered.next()??, lowered.next()??, lowered.next().flatten());
                Some(self.emit(Expr::Limit { site, x, f, seed }, ty, origin))
            }
            "ac_stim" if scope.nesting == 0 => {
                let site = scope.stims.len() as u32;
                scope.stims.push(Site { name: "ac".to_string(), origin: origin.clone(), enabled: None });
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.lower_expr(scope, *arg).and_then(|v| self.known_in_analog(scope.file, *arg, v, "an argument of `ac_stim`")));
                }
                let mut lowered = lowered.into_iter();
                let (mag, phase) = (lowered.next()??, lowered.next().flatten());
                Some(self.emit(Expr::Stim { site, mag, phase }, ty, origin))
            }
            "limit" => {
                self.refuse(scope.file, span, Unlowered::ConditionalLimit);
                None
            }
            "ac_stim" => {
                self.refuse(scope.file, span, Unlowered::ConditionalStim);
                None
            }
            "ddt" => {
                self.refuse(scope.file, span, Unlowered::DdtOffTop);
                None
            }
            name => {
                self.unsupported(scope.file, span, &format!("the core primitive `{name}`"));
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
        if scope.nesting > 0 || scope.block_depth > 0 {
            self.refuse(scope.file, node.span, Unlowered::ConditionalBoundStep);
            return Some(false);
        }
        let [arg] = &args[..] else { return Some(false) };
        let value = self.lower_expr(scope, arg.value)?;
        let site = scope.bounds;
        scope.bounds += 1;
        out.push(self.stmt(piperine_ir::Stmt::BoundStep { site, value, when: None }));
        Some(true)
    }

    /// A node argument: a port or signal of the body, or ground.
    pub(super) fn lower_terminal(&mut self, scope: &Scope, arg: ExprId) -> Option<Terminal> {
        let node = self.design.expr_node(arg).clone();
        let file = self.design.decl_data(node.owner).file;
        let ExprKind::Path(path) = &node.kind else {
            self.unsupported(file, node.span, "a node argument that is not a name");
            return None;
        };
        match self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            Resolution::Decl(decl) => match scope.nodes.get(&decl) {
                Some(index) => Some(Terminal::Node(*index)),
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
