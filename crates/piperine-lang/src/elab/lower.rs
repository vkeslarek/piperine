//! Value code: POM expressions lowered to the IR node of the same form.
//!
//! One `Scope` per owner (a module body, later a function) names the params, nodes and locals
//! an expression may read. A `const` read becomes its folded value; a param read stays a
//! `Param` read, never a number (a host `set` must reach the solver without re-elaboration).

use std::collections::BTreeMap;

use piperine_ir::{BinaryOp, Branch, Constant, Expr, ExprId as LoweredExpr, Local, MathFn, Origin, Site, Stmt, StmtId as LoweredStmt, UnaryOp, ValueType};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::msl::Span;
use crate::pom::{BinOp, CallClass, DeclId, DeclKind, ExprId, ExprKind, FileId, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::{Callee, Method, Ty};

/// What names the code being lowered: the owner's params, nodes and locals.
pub(super) struct Scope {
    pub(super) file: FileId,
    /// Param declaration to its index in the body's params.
    pub(super) params: BTreeMap<DeclId, u32>,
    /// Port or signal declaration to its index in the body's nodes.
    pub(super) nodes: BTreeMap<DeclId, u32>,
    pub(super) locals: Vec<Local>,
    pub(super) local_index: BTreeMap<DeclId, u32>,
    pub(super) limits: Vec<Site>,
    pub(super) stims: Vec<Site>,
    /// The branches the contributions name, in order of first contribution.
    pub(super) branches: Vec<Branch>,
    /// How many `if` branches and closure bodies the code being lowered sits in: a site there
    /// is not unconditional, which `Ir::check` and the model both forbid.
    pub(super) nesting: u32,
    /// Plain blocks entered below the analog block's own statement list: a contribution there
    /// is not a top-level statement (Part IV §10 rule 8 makes it legal; M1 refuses it).
    pub(super) block_depth: u32,
    /// The name the `let` being lowered gives the `limit` call that is its whole initializer.
    pub(super) limit_name: Option<(ExprId, String)>,
}

impl Scope {
    pub(super) fn new(file: FileId) -> Self {
        Scope { file, params: BTreeMap::new(), nodes: BTreeMap::new(), locals: Vec::new(), local_index: BTreeMap::new(), limits: Vec::new(), stims: Vec::new(), branches: Vec::new(), nesting: 0, block_depth: 0, limit_name: None }
    }
}

/// The IR type of a type of the subset.
pub(super) struct ValueTypes;

impl ValueTypes {
    /// `None` for a type no value has in the IR: a node, a module; and for an integer type,
    /// which is lowered from msl-digital T23 on (until then elaboration refuses it by name).
    pub(super) fn of(ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::F64 => Some(ValueType::F64),
            Ty::Bool => Some(ValueType::Bool),
            Ty::Unit => Some(ValueType::Unit),
            Ty::Fn(params, ret) => {
                let params: Option<Vec<ValueType>> = params.iter().map(ValueTypes::of).collect();
                Some(ValueType::Fn(params?, Box::new(ValueTypes::of(ret)?)))
            }
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::UInt(_) | Ty::Int(_) => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// Add an expression to the IR, remembering its type (the builder does not hand it back).
    pub(super) fn emit(&mut self, kind: Expr, ty: ValueType, origin: Origin) -> LoweredExpr {
        self.expr_types.push(ty.clone());
        self.ir.expr(kind, ty, origin)
    }

    /// The type of an expression already added.
    pub(super) fn type_of(&self, expr: LoweredExpr) -> ValueType {
        self.expr_types[expr.index()].clone()
    }

    /// The origin of an expression: where its construct starts.
    pub(super) fn expr_origin(&self, expr: ExprId) -> Origin {
        let node = self.design.expr_node(expr);
        self.locator.origin(self.design.decl_data(node.owner).file, node.span)
    }

    /// `expr` lowered, or `None` after the diagnostics of what could not be.
    pub(super) fn lower_expr(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let file = self.design.decl_data(node.owner).file;
        let origin = self.expr_origin(expr);
        let ty = self.types.ty(expr).and_then(ValueTypes::of);
        let Some(ty) = ty else {
            self.unsupported(file, node.span, "an expression the type checker gave no value type");
            return None;
        };
        match &node.kind {
            ExprKind::Lit(_) => match self.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a literal");
                    None
                }
            },
            ExprKind::Paren(inner) => self.lower_expr(scope, *inner),
            ExprKind::Path(_) => self.lower_path(scope, expr, ty, origin),
            ExprKind::Unary { op, expr: operand } => {
                let op = match op {
                    UnOp::Neg => UnaryOp::Neg,
                    UnOp::Not => UnaryOp::Not,
                    UnOp::Deref | UnOp::Ref => {
                        self.unsupported(file, node.span, "a reference or a dereference");
                        return None;
                    }
                };
                let operand = self.lower_expr(scope, *operand)?;
                Some(self.emit(Expr::Unary { op, expr: operand }, ty, origin))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                // The right operand of `&&`/`||` runs only when the left does not decide: it is
                // conditional exactly like an `if` arm.
                let short_circuit = matches!(op, BinOp::And | BinOp::Or);
                let lhs = self.lower_expr(scope, *lhs);
                scope.nesting += u32::from(short_circuit);
                let rhs = self.lower_expr(scope, *rhs);
                scope.nesting -= u32::from(short_circuit);
                let (lhs, rhs) = (lhs?, rhs?);
                let Some(op) = Self::binary_op(*op) else {
                    self.unsupported(file, node.span, "this operator");
                    return None;
                };
                Some(self.emit(Expr::Binary { op, lhs, rhs }, ty, origin))
            }
            ExprKind::If { cond, then, else_ } => {
                let cond = self.lower_expr(scope, *cond);
                scope.nesting += 1;
                let then = self.lower_block(scope, file, &then.stmts, node.span);
                let otherwise = match else_ {
                    Some(else_) => self.lower_expr(scope, *else_),
                    None => Some(self.emit(Expr::Unit, ValueType::Unit, origin.clone())),
                };
                scope.nesting -= 1;
                Some(self.emit(Expr::If { cond: cond?, then: then?, otherwise: otherwise? }, ty, origin))
            }
            ExprKind::Block(block) => {
                scope.block_depth += 1;
                let lowered = self.lower_block(scope, file, &block.stmts, node.span);
                scope.block_depth -= 1;
                lowered
            }
            ExprKind::Call { args, class, .. } => self.lower_call(scope, expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), *class, ty, origin),
            ExprKind::MethodCall { recv, .. } => {
                let Some(method) = self.types.method(expr) else {
                    self.unsupported(file, node.span, "a method outside the catalogue");
                    return None;
                };
                let recv = self.lower_expr(scope, *recv)?;
                let f = match method {
                    Method::Exp => MathFn::Exp,
                    Method::Ln => MathFn::Ln,
                    Method::Sqrt => MathFn::Sqrt,
                    Method::Abs => MathFn::Abs,
                };
                Some(self.emit(Expr::Math { f, args: vec![recv] }, ty, origin))
            }
            ExprKind::Closure { params, body, .. } => {
                let mut locals = Vec::new();
                for param in params {
                    match self.design.pat_node(param.pat).kind.clone() {
                        PatKind::Ident { binding, sub: None, .. } => locals.push(self.bind_local(scope, binding)?),
                        _ => {
                            self.unsupported(file, node.span, "a closure parameter that is not a name");
                            return None;
                        }
                    }
                }
                scope.nesting += 1;
                let body = self.lower_expr(scope, *body);
                scope.nesting -= 1;
                Some(self.emit(Expr::Closure { params: locals, body: body? }, ty, origin))
            }
            _ => {
                self.unsupported(file, node.span, "this expression form");
                None
            }
        }
    }

    /// A call: of a function of the reachable set (one `Call` node, the function lowered once
    /// however many bodies call it), or of a core primitive or a probe (see `analog.rs`).
    fn lower_call(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], class: CallClass, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match (class, self.types.callee(expr)) {
            (CallClass::Probe, _) => self.lower_probe(scope, expr, ty, origin),
            (_, Some(Callee::Function(function))) => {
                let id = self.function_ids.get(&function).copied()?;
                let mut lowered = Vec::new();
                let mut ok = true;
                for arg in args {
                    match self.lower_expr(scope, *arg) {
                        Some(arg) => lowered.push(arg),
                        None => ok = false,
                    }
                }
                ok.then(|| self.emit(Expr::Call { function: id, args: lowered }, ty, origin))
            }
            (_, Some(Callee::Core(id))) => self.lower_core(scope, expr, id, args, ty, origin),
            (_, Some(Callee::Module(_))) => {
                self.unsupported(scope.file, span, "a placement used as a value");
                None
            }
            (_, None) => {
                self.unsupported(scope.file, span, "a call the type checker did not resolve");
                None
            }
        }
    }

    /// `expr` without the parentheses around it.
    pub(super) fn peel_parens(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    /// Allocate the local a `Binding` declaration names: its name and its type, in declaration
    /// order. `None` after a diagnostic when the checker typed it with a non-value type.
    pub(super) fn bind_local(&mut self, scope: &mut Scope, binding: DeclId) -> Option<u32> {
        let data = self.design.decl_data(binding);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (file, span) = (data.file, data.span);
        let Some(ty) = self.types.local(binding).and_then(ValueTypes::of) else {
            self.unsupported(file, span, "a name whose type is not a value type");
            return None;
        };
        let index = scope.locals.len() as u32;
        scope.locals.push(Local { name, ty });
        scope.local_index.insert(binding, index);
        Some(index)
    }

    /// The statements of a block as one expression: a block with statements is a `Block`, one
    /// that is only its value is that value (`{ x }` is `x`).
    pub(super) fn lower_block(&mut self, scope: &mut Scope, file: FileId, stmts: &[StmtId], span: Span) -> Option<LoweredExpr> {
        let origin = self.locator.origin(file, span);
        let (stmts, value) = self.lower_statements(scope, stmts)?;
        match (stmts.is_empty(), value) {
            (true, Some(value)) => Some(value),
            (_, value) => {
                let ty = value.map_or(ValueType::Unit, |v| self.type_of(v));
                Some(self.emit(Expr::Block { stmts, value }, ty, origin))
            }
        }
    }

    /// The statements of a block in order and its value (the last expression without `;`).
    /// `None` after the diagnostics of every statement that could not be lowered.
    pub(super) fn lower_statements(&mut self, scope: &mut Scope, stmts: &[StmtId]) -> Option<(Vec<LoweredStmt>, Option<LoweredExpr>)> {
        let mut out = Vec::new();
        let mut value = None;
        let mut ok = true;
        for (i, stmt) in stmts.iter().enumerate() {
            let node = self.design.stmt_node(*stmt).clone();
            let file = self.design.decl_data(node.owner).file;
            match &node.kind {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => match self.design.pat_node(*pat).kind.clone() {
                    PatKind::Ident { binding, sub: None, .. } => {
                        let local = self.bind_local(scope, binding);
                        let name = self.design.decl_data(binding).name.as_ref().map(|n| n.text.clone());
                        scope.limit_name = name.map(|n| (self.peel_parens(*init), n));
                        let init = self.lower_expr(scope, *init);
                        scope.limit_name = None;
                        match (local, init) {
                            (Some(local), Some(init)) => out.push(self.ir.stmt(Stmt::Let { local, init })),
                            _ => ok = false,
                        }
                    }
                    _ => {
                        self.unsupported(file, node.span, "a `let` pattern that is not a name");
                        ok = false;
                    }
                },
                StmtKind::Expr { expr, semi } => match self.lower_expr(scope, *expr) {
                    Some(lowered) if i + 1 == stmts.len() && !semi => value = Some(lowered),
                    Some(lowered) => out.push(self.ir.stmt(Stmt::Expr(lowered))),
                    None => ok = false,
                },
                StmtKind::Empty => {}
                StmtKind::Contribution { lhs, rhs } => ok &= self.lower_contribution(scope, &mut out, *lhs, *rhs, node.span),
                _ => {
                    self.unsupported(file, node.span, "this statement");
                    ok = false;
                }
            }
        }
        ok.then_some((out, value))
    }

    fn binary_op(op: BinOp) -> Option<BinaryOp> {
        Some(match op {
            BinOp::Add => BinaryOp::Add,
            BinOp::Sub => BinaryOp::Sub,
            BinOp::Mul => BinaryOp::Mul,
            BinOp::Div => BinaryOp::Div,
            BinOp::Lt => BinaryOp::Lt,
            BinOp::Le => BinaryOp::Le,
            BinOp::Gt => BinaryOp::Gt,
            BinOp::Ge => BinaryOp::Ge,
            BinOp::Eq => BinaryOp::Eq,
            BinOp::Ne => BinaryOp::Ne,
            BinOp::And => BinaryOp::And,
            BinOp::Or => BinaryOp::Or,
            BinOp::Rem | BinOp::BitXor | BinOp::BitAnd | BinOp::BitOr | BinOp::Shl | BinOp::Shr => return None,
        })
    }

    /// A folded value as the IR expression of its type; an integer takes `ty`, its expression's
    /// value type.
    pub(super) fn constant_expr(&mut self, value: Constant, ty: ValueType, origin: Origin) -> LoweredExpr {
        match value {
            Constant::Real(r) => self.emit(Expr::Real(r), ValueType::F64, origin),
            Constant::Bool(b) => self.emit(Expr::Bool(b), ValueType::Bool, origin),
            Constant::Int(v) => self.emit(Expr::Int(v), ty, origin),
        }
    }

    /// A name: a `const` (folded), a param of the body (a read), a local.
    fn lower_path(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            self.unsupported(scope.file, node.span, "a built-in name used as a value");
            return None;
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) => match self.consts.constant(decl) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const`");
                    None
                }
            },
            DeclKind::Param(_) => match scope.params.get(&decl) {
                Some(index) => Some(self.emit(Expr::Param(*index), ty, origin)),
                None => {
                    self.unsupported(scope.file, node.span, "a param of another module");
                    None
                }
            },
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match scope.local_index.get(&decl) {
                Some(index) => Some(self.emit(Expr::Local(*index), ty, origin)),
                None => {
                    self.unsupported(scope.file, node.span, "a name with no value here");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, node.span, "this name as a value");
                None
            }
        }
    }
}
