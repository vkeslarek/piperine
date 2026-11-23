//! `ddx(e, probe)` (Part IV §14.2, analog-operators AO-01): the partial derivative of `e` with
//! respect to one potential or flow probe of a branch of the module.
//!
//! The derivative is symbolic (§14.2.3). `e` is lowered *standalone* (a core primitive's inputs
//! are evaluated at every accepted solution whatever control flow surrounds the call, §14 rule 9:
//! a scope with no locals, a block-local `let` it reads inlined from its source expression), then
//! `piperine_ir::Differentiation` builds `∂e/∂probe` as ordinary expressions next to it, so no
//! backend learns a new form. A form it has no rule for (a loop, an assignment, a closure, a
//! call through a value) is `P0001` naming it. A `ddx` is also recorded on the body
//! (`BodyData::partial_derivatives`), so a reader and the interpreter see where one was.
//! A call of a `fn` inside `e` is expanded where it stands (`expansion.rs`).

use piperine_ir::{BodyId, Differentiation, Expr, ExprId as LoweredExpr, Origin, PartialDerivative, Terminal, ValueType, Variable};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{ExprId, ExprKind, QuantityRole};

impl Elaboration<'_, '_> {
    /// A `ddx(e, probe)` read as a value: the derivative expression; `None` after a diagnostic.
    pub(super) fn lower_ddx(&mut self, scope: &mut Scope, call: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(call).span;
        let [value, probe] = args else { return None };
        let unknown = self.ddx_variable(scope, *probe)?;
        let mut standalone = Self::standalone_scope(scope);
        standalone.expands_functions = true;
        standalone.value_input = true;
        // a local the derivative would read is refused by the pass, naming it; the locals of the
        // closure a `limit` takes are never read, `limit` being transparent
        let expr = self.lower_expr(&mut standalone, *value)?;
        scope.adopt_stimuli(&mut standalone);
        if self.reads_a_function(expr) {
            self.unsupported(scope.file, span, "a call inside a `ddx` that is no `fn` it can expand (a method, a generic `fn`)");
            return None;
        }
        let derivative = match unknown {
            // the body of the derivative is the one being lowered: it has no id yet and the pass reads none of it
            Some(variable) => match Differentiation::new(&mut self.ir, BodyId(0), variable).of(expr) {
                Ok(derivative) => derivative,
                Err(refused) => {
                    self.unsupported(scope.file, span, refused.form);
                    return None;
                }
            },
            // no unknown: `e` is held constant against it (§14.2.3)
            None => self.emit(Expr::Real(piperine_ir::Real::new(0.0)), ValueType::F64, origin.clone()),
        };
        self.adopt_built_expressions();
        let probe_span = self.design.expr_node(*probe).span;
        let name = self.design.file_data(scope.file).text.get(probe_span.range()).unwrap_or("probe").to_string();
        self.structure.partial_derivatives.push(PartialDerivative { name, origin });
        Some(derivative)
    }

    /// The variable of `ddx`: the potential of the probe's nodes, or the flow of the potential branch
    /// between them (the type checker accepted a probe call only). The flow of a branch the module gives
    /// no potential contribution is no unknown, so the inner `None`: the derivative against it is 0, any
    /// other branch being held constant (§14.2.3). The outer `None` is a diagnostic.
    fn ddx_variable(&mut self, scope: &mut Scope, probe: ExprId) -> Option<Option<Variable>> {
        let node = self.design.expr_node(probe).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
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
        match self.quantity_role(*callee) {
            Some(QuantityRole::Flow) => Some(self.potential_branch(scope, plus, minus).map(|(branch, _)| Variable::Flow { branch })),
            _ => Some(Some(Variable::Potential { plus, minus })),
        }
    }

    /// Whether the lowered expression `expr` calls an IR function.
    fn reads_a_function(&self, expr: LoweredExpr) -> bool {
        let mut open = vec![expr];
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = open.pop() {
            if !seen.insert(e) {
                continue;
            }
            let Some(kind) = self.ir.expr_kind(e) else { continue };
            if matches!(kind, Expr::Call { .. } | Expr::FunctionRef(_)) {
                return true;
            }
            open.extend(kind.operands());
        }
        false
    }

    /// Catches the metadata of the expressions a pass built straight in the IR (`Differentiation`)
    /// up with the builder: the type, the continuity and the origin of each, in id order, which
    /// `emit` keeps parallel to the builder's arena.
    pub(super) fn adopt_built_expressions(&mut self) {
        for index in self.expr_types.len()..self.ir.expr_count() {
            let id = piperine_ir::ExprId(index as u32);
            let (Some(kind), Some(ty), Some(origin)) = (self.ir.expr_kind(id).cloned(), self.ir.expr_type(id).cloned(), self.ir.expr_origin(id).cloned()) else { continue };
            let continuous = self.continuous_kind(&kind);
            self.expr_types.push(ty);
            self.expr_continuous.push(continuous);
            self.expr_origins.push(origin);
        }
    }
}
