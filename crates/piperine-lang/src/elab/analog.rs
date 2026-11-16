//! The `analog` block of a body: its statements lowered in source order. Probes, environment
//! reads and sites are lowered here; the contribution to a branch is `contribute.rs`.

use piperine_ir::{Env, Expr, ExprId as LoweredExpr, Origin, Site, StmtId as LoweredStmt, Terminal, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use super::unlowered::Unlowered;
use crate::pom::{BehaviorKind, BuiltinId, BuiltinKind, DeclId, DeclKind, ExprId, ExprKind, Resolution};

impl Elaboration<'_, '_> {
    /// The statements of the `analog` block bound to `module`, in source order; `None` when it
    /// has none. A second `analog` block, or a `digital` one, is `P0001`.
    pub(super) fn lower_analog(&mut self, scope: &mut Scope, module: DeclId) -> Option<Vec<LoweredStmt>> {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        let blocks = data.behaviors.clone();
        let mut stmts = None;
        for block in blocks {
            let decl = self.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            if behavior.kind != BehaviorKind::Analog {
                self.refuse(decl.file, decl.span, Unlowered::DigitalBlock);
                continue;
            }
            if stmts.is_some() {
                self.refuse(decl.file, decl.span, Unlowered::SecondAnalogBlock);
                continue;
            }
            // After a diagnostic the block is empty: no `Ir` is built anyway.
            let (mut lowered, value) = self.lower_statements(scope, &behavior.body).unwrap_or_default();
            // At the top of the block a trailing expression without `;` is a statement like any other.
            if let Some(value) = value {
                lowered.push(self.ir.stmt(piperine_ir::Stmt::Expr(value)));
            }
            stmts = Some(lowered);
        }
        stmts
    }

    /// A potential probe `v(a)` or `v(a, b)`: a `Probe` over the terminals of its nodes (one
    /// node is a probe against ground).
    pub(super) fn lower_probe(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
        // The type checker lets a module read the flow of a branch it drives with a potential;
        // reading it needs the IR's flow read (msl-digital T20 and T35), so until then it is refused.
        let flow = match &self.design.expr_node(*callee).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(q)) => matches!(&self.design.decl_data(q).kind, DeclKind::Quantity(data) if data.role == crate::pom::QuantityRole::Flow),
                Some(Resolution::Builtin(id)) => id.info().name == "flow",
                _ => false,
            },
            _ => false,
        };
        if flow {
            self.unsupported(scope.file, node.span, "a flow probe read as a value");
            return None;
        }
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
        Some(self.emit(Expr::Probe { plus, minus }, ty, origin))
    }

    /// A call of a core primitive of the subset: an environment read, a `limit` or an `ac_stim`
    /// site. A site is numbered when it is met, so numbering follows the source.
    pub(super) fn lower_core(&mut self, scope: &mut Scope, expr: ExprId, id: BuiltinId, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match id.info().name {
            "temperature" => Some(self.emit(Expr::Env(Env::Temperature), ty, origin)),
            "gmin" => Some(self.emit(Expr::Env(Env::Gmin), ty, origin)),
            "source_scale" => Some(self.emit(Expr::Env(Env::SourceScale), ty, origin)),
            // The checker lets `analysis` take the kind `Static` only.
            "now" => Some(self.emit(Expr::Env(Env::Time), ty, origin)),
            "analysis" => Some(self.emit(Expr::Env(Env::Static), ty, origin)),
            "limit" if scope.nesting == 0 => {
                let site = scope.limits.len() as u32;
                let name = match scope.limit_name.take() {
                    Some((at, name)) if at == expr => name,
                    _ => format!("limit{site}"),
                };
                scope.limits.push(Site { name, origin: origin.clone() });
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.lower_expr(scope, *arg));
                }
                let mut lowered = lowered.into_iter();
                let (x, f, seed) = (lowered.next()??, lowered.next()??, lowered.next().flatten());
                Some(self.emit(Expr::Limit { site, x, f, seed }, ty, origin))
            }
            "ac_stim" if scope.nesting == 0 => {
                let site = scope.stims.len() as u32;
                scope.stims.push(Site { name: "ac".to_string(), origin: origin.clone() });
                let mut lowered = Vec::new();
                for arg in args {
                    lowered.push(self.lower_expr(scope, *arg));
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
