//! A `fn` called where an expression is differentiated (Part IV §14.2.3, analog-operators AO-01): the call is
//! expanded where it stands, so the derivative of `ddx(diode_current(v(a, k)), v(a, k))` is the derivative of the
//! body of `diode_current` with its parameter read as the argument.
//!
//! A `ddx` value is lowered standalone (`derivatives.rs`): it holds no local, and a name that stands for an
//! expression is read as that expression (`Scope::let_sources`). A `fn` called there is no IR function (its id would
//! be handed out after the derivative is built, `variants.rs`); its parameters and `let`s are bound the same way, to
//! the argument or the initializer they stand for, and the tail expression is lowered in place. A `fn` whose body is
//! more than `let`s and a value (a statement that assigns, a loop, an early `return`) is `P0001` naming the form: the
//! derivative of a branch of control flow is not an expression.

use piperine_ir::ExprId as LoweredExpr;

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, PatKind, StmtKind};
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// The call `call` of the plain `function`, expanded: its parameters and `let`s read as the expressions they are
    /// bound to, its tail expression lowered where the call stands. `None` after a diagnostic.
    pub(super) fn expand_function_call(&mut self, scope: &mut Scope, call: ExprId, function: DeclId) -> Option<LoweredExpr> {
        let span = self.design.expr_node(call).span;
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let Some(body) = f.body.clone() else {
            self.unsupported(scope.file, span, "a call of a function with no body inside a `ddx`");
            return None;
        };
        if self.takes_handle(function) || self.reaches_recursion(function) {
            self.unsupported(scope.file, span, "a call of a recursive `fn`, or one that takes a handle, inside a `ddx`");
            return None;
        }
        let Some(binding) = self.types.binding(call).map(<[_]>::to_vec) else {
            self.unsupported(scope.file, span, "a call whose arguments the type checker did not bind, inside a `ddx`");
            return None;
        };
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut bound = Vec::new();
        for (param, slot) in params.iter().zip(binding) {
            let Some(argument) = slot.filter(|_| !matches!(self.typed_local(*param), Some(Ty::Option(_) | Ty::Str))) else {
                self.unsupported(scope.file, span, "a call that leaves out an argument, or passes an `Option` or a `str`, inside a `ddx`");
                self.rebind(scope, bound);
                return None;
            };
            bound.push((*param, scope.let_sources.insert(*param, argument)));
        }
        let value = self.expand_body(scope, &body.stmts, &mut bound, span);
        self.rebind(scope, bound);
        value
    }

    /// The tail expression of the statements `stmts` of an expanded `fn`, after each `let` of them is bound to its
    /// initializer (pushed on `bound` with what it replaced). `None` after a diagnostic.
    fn expand_body(&mut self, scope: &mut Scope, stmts: &[crate::pom::StmtId], bound: &mut Vec<(DeclId, Option<ExprId>)>, call_span: Span) -> Option<LoweredExpr> {
        let mut value = None;
        for (index, stmt) in stmts.iter().enumerate() {
            let node = self.design.stmt_node(*stmt).clone();
            match &node.kind {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } if let PatKind::Ident { binding, sub: None, .. } = self.design.pat_node(*pat).kind && !matches!(self.typed_local(binding), Some(Ty::Option(_) | Ty::Str)) => {
                    bound.push((binding, scope.let_sources.insert(binding, *init)));
                }
                StmtKind::Expr { expr, semi: false } if index + 1 == stmts.len() => value = Some(self.lower_expr(scope, *expr)?),
                StmtKind::Empty => {}
                _ => {
                    let file = self.design.decl_data(node.owner).file;
                    self.unsupported(file, node.span, "a statement that is no plain `let` in a `fn` called inside a `ddx`");
                    return None;
                }
            }
        }
        match value {
            Some(value) => Some(value),
            None => {
                self.unsupported(scope.file, call_span, "a `fn` with no value inside a `ddx`");
                None
            }
        }
    }

    /// Puts back what each of `bound` stood for before the expansion, last binding first.
    fn rebind(&mut self, scope: &mut Scope, bound: Vec<(DeclId, Option<ExprId>)>) {
        for (decl, before) in bound.into_iter().rev() {
            match before {
                Some(source) => scope.let_sources.insert(decl, source),
                None => scope.let_sources.remove(&decl),
            };
        }
    }
}
