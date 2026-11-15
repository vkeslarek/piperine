//! Value expressions: `check` against an expected type where the position gives one, `infer`
//! otherwise. A mismatch is one `P0002` at the expression and the expression then takes the
//! expected type, so one mistake is one diagnostic.

use crate::pom::{BinOp, BlockNode, BuiltinKind, DeclKind, ExprId, ExprKind, LitKind, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::stack;

use super::check::Checker;
use super::unsupported::Unsupported;
use super::Ty;

impl Checker<'_> {
    /// Check `expr` against `expected`. A mismatch is one diagnostic; the expression then takes
    /// the expected type.
    pub(super) fn check(&mut self, expr: ExprId, expected: &Ty) {
        self.check_as(expr, expected, "P0002");
    }

    /// `check`, where a mismatch of `expr` itself carries `code` (a rule of the specification
    /// with a code of its own, `E0318` for a `where`) instead of the plain `P0002`.
    pub(super) fn check_as(&mut self, expr: ExprId, expected: &Ty, code: &'static str) {
        stack::deep(|| {
            let node = self.design.expr_node(expr).clone();
            let file = self.file_of_expr(expr);
            // Where the expected type flows into the parts of the expression: the value of a
            // block or of the arms of an `if` is where a mismatch is reported.
            let found = match &node.kind {
                ExprKind::Paren(inner) => {
                    self.check(*inner, expected);
                    Some(expected.clone())
                }
                ExprKind::Block(block) => self.statements(&block.stmts, Some(expected)),
                ExprKind::Closure { .. } => self.closure(expr, expected),
                ExprKind::If { cond, then, else_: Some(else_) } => {
                    self.check(*cond, &Ty::Bool);
                    self.statements(&then.stmts, Some(expected));
                    self.check(*else_, expected);
                    Some(expected.clone())
                }
                // `100` where an `f64` is required: the one integer literal the subset names.
                ExprKind::Lit(lit) if matches!(&lit.kind, LitKind::Int(text) if !text.ends_with("f64")) && *expected == Ty::F64 => {
                    self.mismatch_coded(file, node.span, code, "f64", "an integer literal");
                    Some(expected.clone())
                }
                _ => self.infer(expr),
            };
            if let Some(found) = found
                && found != *expected
            {
                let (expected_name, found_name) = (expected.name(self.design), found.name(self.design));
                self.mismatch_coded(file, node.span, code, &expected_name, &found_name);
            }
            self.exprs[expr.index()] = Some(expected.clone());
        })
    }

    /// The type of `expr` from the expression alone; `None` after a diagnostic.
    pub(super) fn infer(&mut self, expr: ExprId) -> Option<Ty> {
        stack::deep(|| {
            let ty = self.infer_kind(expr);
            self.exprs[expr.index()] = ty.clone();
            ty
        })
    }

    fn infer_kind(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        match &node.kind {
            ExprKind::Lit(lit) => self.literal(&lit.kind, file, node.span),
            ExprKind::Path(_) => self.path(expr),
            ExprKind::Paren(inner) => self.infer(*inner),
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => {
                self.check(*operand, &Ty::F64);
                Some(Ty::F64)
            }
            ExprKind::Unary { op: UnOp::Not, expr: operand } => {
                self.check(*operand, &Ty::Bool);
                Some(Ty::Bool)
            }
            ExprKind::Binary { op, lhs, rhs } => self.binary(*op, *lhs, *rhs),
            ExprKind::Block(block) => self.statements(&block.stmts, None),
            ExprKind::If { cond, then, else_ } => self.conditional(*cond, then, *else_),
            ExprKind::Call { .. } => self.call(expr),
            ExprKind::MethodCall { .. } => self.method_call(expr),
            ExprKind::Closure { .. } => {
                self.unsupported(file, node.span, "closure outside an argument whose parameter has a function type");
                None
            }
            other => {
                self.unsupported(file, node.span, Unsupported::expr(other));
                None
            }
        }
    }

    fn binary(&mut self, op: BinOp, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                self.check(lhs, &Ty::F64);
                self.check(rhs, &Ty::F64);
                Some(Ty::F64)
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.check(lhs, &Ty::F64);
                self.check(rhs, &Ty::F64);
                Some(Ty::Bool)
            }
            BinOp::And | BinOp::Or => {
                self.check(lhs, &Ty::Bool);
                self.check(rhs, &Ty::Bool);
                Some(Ty::Bool)
            }
            BinOp::Eq | BinOp::Ne => {
                match self.infer(lhs) {
                    Some(t @ (Ty::F64 | Ty::Bool)) => self.check(rhs, &t),
                    Some(_) => {
                        let file = self.file_of_expr(lhs);
                        let span = self.design.expr_node(lhs).span;
                        self.unsupported(file, span, "comparison of values that are neither `f64` nor `bool`");
                    }
                    None => {
                        self.infer(rhs);
                    }
                }
                Some(Ty::Bool)
            }
            BinOp::Rem | BinOp::BitXor | BinOp::BitAnd | BinOp::BitOr | BinOp::Shl | BinOp::Shr => {
                let file = self.file_of_expr(lhs);
                let span = self.design.expr_node(lhs).span;
                self.unsupported(file, span, Unsupported::binary(op));
                None
            }
        }
    }

    /// `if cond { then } else { else_ }`: with an `else` the common type of the two blocks;
    /// without one, `()`.
    fn conditional(&mut self, cond: ExprId, then: &BlockNode, else_: Option<ExprId>) -> Option<Ty> {
        self.check(cond, &Ty::Bool);
        let then_ty = self.statements(&then.stmts, None);
        match (else_, then_ty) {
            (Some(else_), Some(t)) => {
                self.check(else_, &t);
                Some(t)
            }
            (Some(else_), None) => {
                self.infer(else_);
                None
            }
            (None, Some(t)) if t != Ty::Unit => {
                let file = self.file_of_expr(cond);
                self.mismatch(file, then.span, &Ty::Unit, &t);
                Some(Ty::Unit)
            }
            (None, t) => t,
        }
    }

    fn literal(&mut self, kind: &LitKind, file: crate::pom::FileId, span: crate::msl::Span) -> Option<Ty> {
        match kind {
            LitKind::Float { raw, .. } => match raw.rsplit_once('_') {
                Some((_, suffix)) if suffix.starts_with('f') && suffix != "f64" => {
                    self.unsupported(file, span, &format!("`{suffix}` literal"));
                    None
                }
                _ => Some(Ty::F64),
            },
            LitKind::Bool(_) => Some(Ty::Bool),
            LitKind::Int(text) if text.ends_with("f64") => Some(Ty::F64),
            LitKind::Int(_) => {
                self.unsupported(file, span, "integer literal");
                None
            }
            LitKind::Str { .. } => {
                self.unsupported(file, span, "string literal");
                None
            }
            LitKind::Unknown => {
                self.unsupported(file, span, "`Unknown` literal");
                None
            }
        }
    }

    /// A name: a param, a constant, a parameter or local binding, or a function as a value.
    fn path(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let file = self.file_of_expr(expr);
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        match res {
            Resolution::Decl(decl) => {
                let kind = self.design.decl_data(decl).kind.clone();
                match kind {
                    DeclKind::Const(c) => {
                        self.reach(decl);
                        self.lower_type(c.ty)
                    }
                    DeclKind::Param(p) => self.lower_type(p.ty),
                    DeclKind::Port(p) => self.node_type(p.ty),
                    DeclKind::Signal(s) => self.node_type(s.ty),
                    DeclKind::FnParam(_) | DeclKind::Binding(_) => {
                        let local = self.locals.get(&decl).cloned();
                        if local.is_none() {
                            self.unsupported(file, node.span, "a name whose type is unknown");
                        }
                        local
                    }
                    DeclKind::Fn(_) => {
                        self.reach(decl);
                        let (params, ret) = self.signature(decl)?;
                        Some(Ty::Fn(params, Box::new(ret)))
                    }
                    other => {
                        self.unsupported(file, node.span, &format!("a name of a {}", other.label().to_lowercase()));
                        None
                    }
                }
            }
            Resolution::Builtin(id) if id.info().kind == BuiltinKind::Node => Some(Ty::AnyNode),
            Resolution::Builtin(id) => {
                self.unsupported(file, node.span, &format!("built-in `{}`", id.name()));
                None
            }
            Resolution::Unresolved | Resolution::Deferred(_) => {
                self.unsupported(file, node.span, "an unresolved name");
                None
            }
        }
    }

    /// The statements of a block, a function body or an analog block; the type of the block is
    /// that of its last expression without a `;`, else `()`. With an `expected` type the last
    /// expression is checked against it.
    pub(super) fn statements(&mut self, stmts: &[StmtId], expected: Option<&Ty>) -> Option<Ty> {
        let mut value = Some(Ty::Unit);
        for (i, stmt) in stmts.iter().enumerate() {
            let last = i + 1 == stmts.len();
            value = Some(Ty::Unit);
            let node = self.design.stmt_node(*stmt).clone();
            let file = self.design.decl_data(node.owner).file;
            match &node.kind {
                StmtKind::Let { pat, ty, init, else_ } => {
                    if else_.is_some() {
                        self.unsupported(file, node.span, Unsupported::stmt(&node.kind));
                    } else {
                        self.let_statement(file, node.span, *pat, *ty, *init);
                    }
                }
                StmtKind::Expr { expr, semi } => {
                    if last && !semi {
                        match expected {
                            Some(expected) => {
                                self.check(*expr, expected);
                                value = Some(expected.clone());
                            }
                            None => value = self.infer(*expr),
                        }
                    } else {
                        self.infer(*expr);
                    }
                }
                StmtKind::Empty => {}
                StmtKind::Contribution { lhs, rhs } => self.contribution(file, node.span, *lhs, *rhs),
                StmtKind::Item(_) | StmtKind::Commit { .. } | StmtKind::Signal(_) | StmtKind::Reaction(_) => {
                    self.unsupported(file, node.span, Unsupported::stmt(&node.kind));
                }
            }
        }
        value
    }

    /// `let x = init;` or `let x: T = init;`: the binding takes the annotation, else the type of
    /// the initializer.
    fn let_statement(&mut self, file: crate::pom::FileId, span: crate::msl::Span, pat: crate::pom::PatId, ty: Option<crate::pom::TypeId>, init: Option<ExprId>) {
        let pat_node = self.design.pat_node(pat).clone();
        let PatKind::Ident { by_ref: false, mutable: false, sub: None, binding, res: Resolution::Unresolved, .. } = &pat_node.kind else {
            self.unsupported(file, pat_node.span, Unsupported::pat(&pat_node.kind));
            return;
        };
        let annotated = match ty {
            Some(t) => match self.lower_type(t) {
                Some(t) => Some(t),
                None => return,
            },
            None => None,
        };
        let bound = match (annotated, init) {
            (Some(t), Some(init)) => {
                self.check(init, &t);
                Some(t)
            }
            (None, Some(init)) => self.infer(init),
            (Some(_), None) | (None, None) => {
                self.unsupported(file, span, "`let` without a value");
                None
            }
        };
        if let Some(t) = bound {
            self.locals.insert(*binding, t);
        }
    }
}
