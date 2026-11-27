//! Closures and function values (part I §1.5.14, §3 rules 1 and 3).
//!
//! A closure lowers to `Expr::Closure`; the mutable locals it reads are copied where it is written
//! (`let name$copy = name;` before it), so a later assignment does not reach it: a closure captures
//! by value. A function's name used as a value is `Expr::FunctionRef`; a call of a local of
//! function type is `Expr::CallValue`. The IR types a call through a value by its function type.

use piperine_ir::{Expr, ExprId as LoweredExpr, Local, Origin, Stmt, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use super::variants::Half;
use crate::refusal::forms::ValueForm;
use crate::msl::Span;
use crate::pom::{ClosureParam, DeclId, DeclKind, ExprId, ExprKind, PatKind, Resolution};

impl Elaboration<'_, '_> {
    /// `|params| body`, its captured mutable locals copied first.
    pub(super) fn lower_closure(&mut self, scope: &mut Scope, closure: ExprId, params: &[ClosureParam], body: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(closure).span;
        let (copies, lets) = self.copy_captured(scope, body);
        let mut locals = Vec::new();
        for param in params {
            match self.design.pat_node(param.pat).kind.clone() {
                PatKind::Ident { binding, sub: None, .. } => locals.push(self.bind_local(scope, binding)?),
                _ => {
                    self.refuse(scope.file, span, ValueForm::ClosureParameterThatIsNotName);
                    return None;
                }
            }
        }
        scope.nesting += 1;
        let lowered = self.lower_expr(scope, body);
        scope.nesting -= 1;
        for (decl, original) in copies {
            scope.local_index.insert(decl, original);
        }
        let lowered = lowered?;
        let parameters: Vec<ValueType> = locals.iter().map(|l| scope.locals[*l as usize].ty.clone()).collect();
        let ty = ValueType::Fn(parameters, Box::new(self.type_of(lowered)));
        let made = self.emit(Expr::Closure { params: locals, body: lowered }, ty.clone(), origin.clone());
        if lets.is_empty() {
            return Some(made);
        }
        Some(self.emit(Expr::Block { stmts: lets, value: Some(made) }, ty, origin))
    }

    /// The mutable locals `body` reads that were declared outside it: each gets a copy, the name
    /// now stands for the copy while the body is lowered. Gives the (name, original local) pairs to
    /// restore and the statements that make the copies.
    fn copy_captured(&mut self, scope: &mut Scope, body: ExprId) -> (Vec<(DeclId, u32)>, Vec<LoweredStmt>) {
        let mut inside = Vec::new();
        crate::pom::BodyWalk::new(self.design).descendants_depth_first(body, &mut inside);
        let (mut copies, mut lets) = (Vec::new(), Vec::new());
        for expr in inside {
            let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { continue };
            let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { continue };
            let Some(original) = scope.local_index.get(&decl).copied() else { continue };
            if !scope.locals[original as usize].mutable || copies.iter().any(|(d, _)| *d == decl) {
                continue;
            }
            let held = scope.locals[original as usize].clone();
            let copy = scope.locals.len() as u32;
            scope.locals.push(Local { name: format!("{}$copy{copy}", held.name), ty: held.ty.clone(), mutable: false });
            let origin = self.expr_origin(expr);
            let read = self.emit(Expr::Local(original), held.ty, origin);
            lets.push(self.stmt(Stmt::Let { local: copy, init: read }));
            scope.local_index.insert(decl, copy);
            copies.push((decl, original));
        }
        (copies, lets)
    }

    /// A function's name used as a value: `Expr::FunctionRef` of its function.
    pub(super) fn lower_function_value(&mut self, scope: &mut Scope, function: DeclId, ty: ValueType, span: Span, origin: Origin) -> Option<LoweredExpr> {
        let parameters = self.design.decl_data(function).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::FnParam(_))).count();
        let ValueType::Fn(types, _) = &ty else { return None };
        if types.len() != parameters || self.calls_option_function_decl(function) {
            self.refuse(scope.file, span, ValueForm::FunctionWithOptionParameterOrResultUsed);
            return None;
        }
        let (id, ret) = self.request(function, Half::Whole)?;
        let ValueType::Fn(types, _) = ty else { return None };
        Some(self.emit(Expr::FunctionRef(id), ValueType::Fn(types, Box::new(ret)), origin))
    }

    /// The local a call `callee(args)` names when it is a local of function type.
    pub(super) fn called_local(&self, scope: &Scope, expr: ExprId) -> Option<DeclId> {
        let ExprKind::Call { callee, .. } = &self.design.expr_node(expr).kind else { return None };
        let ExprKind::Path(path) = &self.design.expr_node(self.peel_parens(*callee)).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(decl)) if scope.local_index.contains_key(&decl) => Some(decl),
            _ => None,
        }
    }

    /// `f(args)` where the local `decl` holds a function value.
    pub(super) fn lower_value_call(&mut self, scope: &mut Scope, expr: ExprId, decl: DeclId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        let mut lowered = Vec::new();
        for arg in args {
            lowered.push(self.lower_expr(scope, *arg)?);
        }
        let index = scope.local_index[&decl];
        let callee_ty = scope.locals[index as usize].ty.clone();
        let ValueType::Fn(_, ret) = &callee_ty else {
            self.refuse(scope.file, span, ValueForm::CallOfValueThatIsNotFunction);
            return None;
        };
        let ret = (**ret).clone();
        let callee = self.emit(Expr::Local(index), callee_ty, origin.clone());
        Some(self.emit(Expr::CallValue { callee, args: lowered }, ret, origin))
    }

    /// Whether the callee `callee` has a function type: typed so, or a name of a local whose type is one (a module-body `let` of a
    /// closure, `let q = |y: f64| y * 2.0;`).
    pub(super) fn is_function_valued(&self, callee: ExprId) -> bool {
        if matches!(self.typed(callee), Some(crate::types::Ty::Fn(..))) {
            return true;
        }
        let ExprKind::Path(path) = &self.design.expr_node(self.peel_parens(callee)).kind else { return false };
        matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(decl)) if matches!(self.typed_local(decl), Some(crate::types::Ty::Fn(..))))
    }

    /// `(callee)(args)` where `callee` is an expression whose value is a function: the value lowered, then called.
    pub(super) fn lower_expression_call(&mut self, scope: &mut Scope, callee: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(callee).span;
        let function = self.lower_expr(scope, callee)?;
        let callee_ty = self.expr_types[function.index()].clone();
        let ValueType::Fn(_, ret) = &callee_ty else {
            self.refuse(scope.file, span, ValueForm::CalleeWhoseFunctionValueHasNoFunction);
            return None;
        };
        let ret = (**ret).clone();
        let mut lowered = Vec::new();
        for arg in args {
            lowered.push(self.lower_expr(scope, *arg)?);
        }
        Some(self.emit(Expr::CallValue { callee: function, args: lowered }, ret, origin))
    }

    /// Whether the function `decl` returns an `Option`.
    fn calls_option_function_decl(&self, decl: DeclId) -> bool {
        let DeclKind::Fn(f) = &self.design.decl_data(decl).kind else { return false };
        f.ret.and_then(|t| self.typed_written(t)).is_some_and(|t| matches!(t, crate::types::Ty::Option(_)))
    }
}
