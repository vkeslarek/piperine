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

use super::AnalogLowering;
use piperine_ir::{BodyId, Differentiation, Expr, ExprId as LoweredExpr, Origin, PartialDerivative, Terminal, ValueType, Variable};

use crate::elab::elaborator::Scope;
use crate::refusal::forms::AnalogForm;
use crate::pom::{ExprId, ExprKind, QuantityRole};

impl AnalogLowering<'_, '_, '_> {
    /// A `ddx(e, probe)` read as a value: the derivative expression; `None` after a diagnostic.
    pub(super) fn lower_ddx(&mut self, scope: &mut Scope, call: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(call).span;
        let [value, probe] = args else { return None };
        let (unknown, reversed) = self.ddx_variable(scope, *probe)?;
        let mut standalone = Self::standalone_scope(scope);
        standalone.analog.differentiated = true;
        standalone.analog.value_input = true;
        // a local the derivative would read is refused by the pass, naming it; the locals of the
        // closure a `limit` takes are never read, `limit` being transparent
        let expr = self.values().lower_expr(&mut standalone, *value)?;
        scope.adopt_standalone(&mut standalone);
        if self.reads_a_function(expr) {
            self.refuse(scope.values.file, span, AnalogForm::CallInsideDdxThatExpandsToNo);
            return None;
        }
        // the body of the derivative is the one being lowered: it has no id yet and the pass reads none of it
        let derivative = match Differentiation::new(&mut self.state.builder, BodyId(0), unknown).of(expr) {
            Ok(derivative) => derivative,
            Err(refused) => {
                self.refuse_with(scope.values.file, span, AnalogForm::FormWithNoDerivativeRule, [refused.form.to_string()]);
                return None;
            }
        };
        self.adopt_built_expressions();
        if self.reads_a_flow_leaf(derivative, &scope.analog.flow_leaves.clone()) {
            self.refuse(scope.values.file, span, AnalogForm::DdxWhoseDerivativeReadsValueOfFlow);
            return None;
        }
        let derivative = if reversed { self.emit(Expr::Unary { op: piperine_ir::UnaryOp::Neg, expr: derivative }, ValueType::F64, origin.clone()) } else { derivative };
        let probe_span = self.state.design.expr_node(*probe).span;
        let name = self.state.design.file_data(scope.values.file).text.get(probe_span.range()).unwrap_or("probe").to_string();
        self.state.digital.structure.partial_derivatives.push(PartialDerivative { name, origin });
        Some(derivative)
    }

    /// The variable of `ddx` and whether the probe names its branch the other way round: the potential of the
    /// probe's nodes, or the flow of the branch between them (the type checker accepted a probe call only). A
    /// branch the module gives no potential contribution is a leaf like any other (`flow_leaf_branch`), so a probe
    /// of the same flow contributes 1 and any other quantity is held constant (§14.2.3). `None` is a diagnostic.
    fn ddx_variable(&mut self, scope: &mut Scope, probe: ExprId) -> Option<(Variable, bool)> {
        let node = self.state.design.expr_node(probe).clone();
        let ExprKind::Call { callee, args, .. } = &node.kind else { return None };
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
        match self.quantity_role(*callee) {
            Some(QuantityRole::Flow) => {
                let (branch, reversed) = match self.potential_branch(scope, plus, minus) {
                    Some(found) => found,
                    None => self.flow_leaf_branch(scope, plus, minus),
                };
                Some((Variable::Flow { branch }, reversed))
            }
            _ => Some((Variable::Potential { plus, minus }, false)),
        }
    }

    /// Whether the lowered expression `expr` reads the flow of one of `branches`.
    fn reads_a_flow_leaf(&self, expr: LoweredExpr, branches: &[u32]) -> bool {
        let mut open = vec![expr];
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = open.pop() {
            if !seen.insert(e) {
                continue;
            }
            let Some(kind) = self.state.builder.expr_kind(e) else { continue };
            if matches!(kind, Expr::Flow { branch } if branches.contains(branch)) {
                return true;
            }
            open.extend(kind.operands());
        }
        false
    }

    /// Whether the lowered expression `expr` calls an IR function.
    fn reads_a_function(&self, expr: LoweredExpr) -> bool {
        let mut open = vec![expr];
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = open.pop() {
            if !seen.insert(e) {
                continue;
            }
            let Some(kind) = self.state.builder.expr_kind(e) else { continue };
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
        for index in self.state.values.expr_types.len()..self.state.builder.expr_count() {
            let id = piperine_ir::ExprId(index as u32);
            let (Some(kind), Some(ty), Some(origin)) = (self.state.builder.expr_kind(id).cloned(), self.state.builder.expr_type(id).cloned(), self.state.builder.expr_origin(id).cloned()) else { continue };
            let continuous = self.continuous_kind(&kind);
            self.state.values.expr_types.push(ty);
            self.state.values.expr_continuous.push(continuous);
            self.state.values.expr_origins.push(origin);
        }
    }
}
