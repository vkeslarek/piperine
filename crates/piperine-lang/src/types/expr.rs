//! Value expressions: `check` against an expected type where the position gives one, `infer`
//! otherwise. A mismatch is one `P0002` at the expression and the expression then takes the
//! expected type, so one mistake is one diagnostic.

use crate::pom::{DeclKind, ExprId, ExprKind, LitKind, PatKind, Resolution, StmtId, StmtKind};
use crate::stack;

use super::check::Checker;
use super::unsupported::Unsupported;
use super::Ty;

impl Checker<'_> {
    /// Check `expr` against `expected`. A mismatch is one diagnostic; the expression then takes
    /// the expected type.
    pub(super) fn check(&mut self, expr: ExprId, expected: &Ty) {
        stack::deep(|| {
            let found = self.infer(expr);
            if let Some(found) = found
                && found != *expected
            {
                let node = self.design.expr_node(expr);
                let (file, span) = (self.file_of_expr(expr), node.span);
                self.mismatch(file, span, expected, &found);
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
            other => {
                self.unsupported(file, node.span, Unsupported::expr(other));
                None
            }
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
                StmtKind::Item(_) | StmtKind::Contribution { .. } | StmtKind::Commit { .. } | StmtKind::Signal(_) | StmtKind::Reaction(_) => {
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
