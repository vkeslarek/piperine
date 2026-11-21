//! Paths, types, patterns, expressions and statements into the arenas. One
//! node per syntax node; nothing desugared, nothing resolved.

use super::Lowerer;
use crate::msl::ast;
use crate::pom::body::{
    Arg, Arm, BlockNode, ClosureParam, ExprKind, ExprNode, FieldInit, FieldPatNode, GenericArgNode,
    GenericArgsNode, PatKind, PatNode, PathNode, PathSegment, StmtKind, StmtNode, TypeKind, TypeNode,
};
use crate::pom::decl::{BindingData, BindingOrigin, DeclKind, Name, ReactionData, SignalData};
use crate::pom::ids::{DeclId, ExprId, PatId, PathId, StmtId, TypeId};
use crate::pom::resolution::{CallClass, Resolution, StructClass};

pub(super) fn name(i: &ast::Ident) -> Name {
    Name { text: i.name.clone(), raw: i.raw, span: i.span.0 }
}

impl Lowerer<'_> {
    // ---- paths, generic arguments, types --------------------------------------------------

    pub(crate) fn path(&mut self, p: &ast::Path) -> PathId {
        let segments = p
            .segments
            .iter()
            .map(|s| PathSegment {
                name: name(&s.ident),
                args: s.args.as_ref().map(|a| self.generic_args(a)),
                res: Resolution::Unresolved,
            })
            .collect();
        self.design.push_path(PathNode { segments, span: p.span.0, owner: self.owner })
    }

    pub(crate) fn generic_args(&mut self, g: &ast::GenericArgs) -> GenericArgsNode {
        match g {
            ast::GenericArgs::Angle { args, turbofish, span } => GenericArgsNode::Angle {
                args: args
                    .iter()
                    .map(|a| match a {
                        ast::GenericArg::Type(t) => GenericArgNode::Type(self.ty(t)),
                        ast::GenericArg::Const(e) => GenericArgNode::Const(self.expr(e)),
                    })
                    .collect(),
                turbofish: *turbofish,
                span: span.0,
            },
            ast::GenericArgs::Paren { inputs, output, span } => GenericArgsNode::Paren {
                inputs: inputs.iter().map(|t| self.ty(t)).collect(),
                output: output.as_ref().map(|t| self.ty(t)),
                span: span.0,
            },
        }
    }

    pub(crate) fn ty(&mut self, t: &ast::Type) -> TypeId {
        let kind = match &t.kind {
            ast::TypeKind::Path(p) => TypeKind::Path(self.path(p)),
            ast::TypeKind::Array { elem, len } => TypeKind::Array { elem: self.ty(elem), len: self.expr(len) },
            ast::TypeKind::Tuple(items) => TypeKind::Tuple(items.iter().map(|i| self.ty(i)).collect()),
            ast::TypeKind::FnPtr { params, ret } => TypeKind::FnPtr {
                params: params.iter().map(|i| self.ty(i)).collect(),
                ret: ret.as_ref().map(|r| self.ty(r)),
            },
            ast::TypeKind::Flip(inner) => TypeKind::Flip(self.ty(inner)),
            ast::TypeKind::Infer => TypeKind::Infer,
        };
        self.design.push_type(TypeNode { kind, span: t.span.0, owner: self.owner })
    }

    // ---- patterns -------------------------------------------------------------------------

    /// A `Binding` declaration for one identifier, parented to the owner. Its
    /// `pat` is patched once the pattern node exists.
    fn binding(&mut self, ident: &ast::Ident, span: crate::msl::Span, origin: BindingOrigin) -> DeclId {
        let data = BindingData { pat: PatId(0), origin };
        self.new_decl(DeclKind::Binding(data), Some(ident), span, false)
    }

    fn set_binding_pat(&mut self, binding: DeclId, pat: PatId) {
        if let DeclKind::Binding(b) = &mut self.design.decl_mut(binding).kind {
            b.pat = pat;
        }
    }

    pub(crate) fn pat(&mut self, p: &ast::Pat, origin: BindingOrigin) -> PatId {
        let mut introduced: Vec<DeclId> = Vec::new();
        let kind = match &p.kind {
            ast::PatKind::Wild => PatKind::Wild,
            ast::PatKind::Rest => PatKind::Rest,
            ast::PatKind::Ident { name: ident, sub } => {
                let binding = self.binding(ident, p.span.0, origin);
                introduced.push(binding);
                PatKind::Ident {
                    name: name(ident),
                    sub: sub.as_ref().map(|s| self.pat(s, origin)),
                    binding,
                    res: Resolution::Unresolved,
                }
            }
            ast::PatKind::Lit(e) => PatKind::Lit(self.expr(e)),
            ast::PatKind::Path(path) => PatKind::Path(self.path(path)),
            ast::PatKind::Tuple(items) => PatKind::Tuple(items.iter().map(|i| self.pat(i, origin)).collect()),
            ast::PatKind::TupleStruct { path, pats } => PatKind::TupleStruct {
                path: self.path(path),
                pats: pats.iter().map(|i| self.pat(i, origin)).collect(),
            },
            ast::PatKind::Struct { path, fields, rest } => {
                let path = self.path(path);
                let fields = fields
                    .iter()
                    .map(|f| match &f.pat {
                        Some(sub) => FieldPatNode {
                            name: name(&f.name),
                            pat: Some(self.pat(sub, origin)),
                            binding: None,
                            res: Resolution::Unresolved,
                            span: f.span.0,
                        },
                        None => {
                            let binding = self.binding(&f.name, f.span.0, origin);
                            introduced.push(binding);
                            FieldPatNode { name: name(&f.name), pat: None, binding: Some(binding), res: Resolution::Unresolved, span: f.span.0 }
                        }
                    })
                    .collect();
                PatKind::Struct { path, fields, rest: *rest }
            }
            ast::PatKind::Or(items) => PatKind::Or(items.iter().map(|i| self.pat(i, origin)).collect()),
            ast::PatKind::Range { lo, hi, inclusive } => PatKind::Range {
                lo: lo.as_ref().map(|e| self.expr(e)),
                hi: hi.as_ref().map(|e| self.expr(e)),
                inclusive: *inclusive,
            },
            ast::PatKind::Array(items) => PatKind::Array(items.iter().map(|i| self.pat(i, origin)).collect()),
        };
        let id = self.design.push_pat(PatNode { kind, span: p.span.0, owner: self.owner });
        for b in introduced {
            self.set_binding_pat(b, id);
        }
        id
    }

    // ---- expressions ----------------------------------------------------------------------

    fn arg(&mut self, a: &ast::Arg) -> Arg {
        Arg { name: a.name.as_ref().map(name), value: self.expr(&a.value), res: Resolution::Unresolved, span: a.span.0 }
    }

    pub(crate) fn field_init(&mut self, f: &ast::FieldInit) -> FieldInit {
        FieldInit { name: name(&f.name), value: self.expr(&f.value), shorthand: f.shorthand, res: Resolution::Unresolved, span: f.span.0 }
    }

    fn opt_expr(&mut self, e: &Option<Box<ast::Expr>>) -> Option<ExprId> {
        e.as_ref().map(|e| self.expr(e))
    }

    pub(crate) fn expr(&mut self, e: &ast::Expr) -> ExprId {
        crate::stack::deep(|| self.expr_inner(e))
    }

    fn expr_inner(&mut self, e: &ast::Expr) -> ExprId {
        let kind = match &e.kind {
            ast::ExprKind::Lit(l) => ExprKind::Lit(l.clone()),
            ast::ExprKind::Path(p) => ExprKind::Path(self.path(p)),
            ast::ExprKind::Unary { op, expr } => ExprKind::Unary { op: *op, expr: self.expr(expr) },
            ast::ExprKind::Binary { op, lhs, rhs } => {
                ExprKind::Binary { op: *op, lhs: self.expr(lhs), rhs: self.expr(rhs) }
            }
            ast::ExprKind::Assign { op, lhs, rhs } => {
                ExprKind::Assign { op: *op, lhs: self.expr(lhs), rhs: self.expr(rhs) }
            }
            ast::ExprKind::Cast { expr, ty } => ExprKind::Cast { expr: self.expr(expr), ty: self.ty(ty) },
            ast::ExprKind::Call { callee, args, params } => ExprKind::Call {
                callee: self.expr(callee),
                args: args.iter().map(|a| self.arg(a)).collect(),
                params: params.as_ref().map(|ps| ps.iter().map(|f| self.field_init(f)).collect()),
                class: CallClass::Unclassified,
            },
            ast::ExprKind::MethodCall { recv, method, generics, args } => ExprKind::MethodCall {
                recv: self.expr(recv),
                method: name(method),
                generics: generics.as_ref().map(|g| self.generic_args(g)),
                args: args.iter().map(|a| self.arg(a)).collect(),
                res: Resolution::Unresolved,
            },
            ast::ExprKind::Field { expr, name: n } => {
                ExprKind::Field { expr: self.expr(expr), name: name(n), res: Resolution::Unresolved }
            }
            ast::ExprKind::Index { expr, index } => ExprKind::Index { expr: self.expr(expr), index: self.expr(index) },
            ast::ExprKind::Range { lo, hi, inclusive } => {
                ExprKind::Range { lo: self.opt_expr(lo), hi: self.opt_expr(hi), inclusive: *inclusive }
            }
            ast::ExprKind::Paren(inner) => ExprKind::Paren(self.expr(inner)),
            ast::ExprKind::Tuple(items) => ExprKind::Tuple(items.iter().map(|i| self.expr(i)).collect()),
            ast::ExprKind::Array(items) => ExprKind::Array(items.iter().map(|i| self.expr(i)).collect()),
            ast::ExprKind::ArrayRepeat { elem, len } => {
                ExprKind::ArrayRepeat { elem: self.expr(elem), len: self.expr(len) }
            }
            ast::ExprKind::Struct { path, fields } => ExprKind::Struct {
                path: self.path(path),
                fields: fields.iter().map(|f| self.field_init(f)).collect(),
                class: StructClass::Unclassified,
            },
            ast::ExprKind::Block(b) => ExprKind::Block(self.block(b)),
            ast::ExprKind::If { cond, then, else_ } => {
                ExprKind::If { cond: self.expr(cond), then: self.block(then), else_: self.opt_expr(else_) }
            }
            ast::ExprKind::Let { pat, expr } => {
                ExprKind::Let { pat: self.pat(pat, BindingOrigin::IfLet), expr: self.expr(expr) }
            }
            ast::ExprKind::Match { scrutinee, arms } => ExprKind::Match {
                scrutinee: self.expr(scrutinee),
                arms: arms
                    .iter()
                    .map(|a| Arm {
                        meta: self.meta(&a.meta),
                        pat: self.pat(&a.pat, BindingOrigin::MatchArm),
                        guard: a.guard.as_ref().map(|g| self.expr(g)),
                        body: self.expr(&a.body),
                        span: a.span.0,
                    })
                    .collect(),
            },
            ast::ExprKind::While { cond, body } => ExprKind::While { cond: self.expr(cond), body: self.block(body) },
            ast::ExprKind::Loop { body } => ExprKind::Loop { body: self.block(body) },
            ast::ExprKind::For { pat, iter, body } => ExprKind::For {
                pat: self.pat(pat, BindingOrigin::For),
                iter: self.expr(iter),
                body: self.block(body),
            },
            ast::ExprKind::Break(v) => ExprKind::Break(self.opt_expr(v)),
            ast::ExprKind::Continue => ExprKind::Continue,
            ast::ExprKind::Return(v) => ExprKind::Return(self.opt_expr(v)),
            ast::ExprKind::Closure { params, ret, body } => ExprKind::Closure {
                params: params
                    .iter()
                    .map(|p| ClosureParam {
                        pat: self.pat(&p.pat, BindingOrigin::ClosureParam),
                        ty: p.ty.as_ref().map(|t| self.ty(t)),
                        span: p.span.0,
                    })
                    .collect(),
                ret: ret.as_ref().map(|t| self.ty(t)),
                body: self.expr(body),
            },
            ast::ExprKind::Try(inner) => ExprKind::Try(self.expr(inner)),
            ast::ExprKind::Emit { stream, fields } => {
                ExprKind::Emit { stream: stream.clone(), fields: fields.iter().map(|f| self.field_init(f)).collect() }
            }
            ast::ExprKind::IncludeStr(l) => ExprKind::IncludeStr(l.clone()),
            ast::ExprKind::Error => ExprKind::Error,
        };
        self.design.push_expr(ExprNode { kind, span: e.span.0, owner: self.owner })
    }

    // ---- statements -----------------------------------------------------------------------

    pub(crate) fn block(&mut self, b: &ast::Block) -> BlockNode {
        BlockNode { stmts: b.stmts.iter().map(|s| self.stmt(s)).collect(), span: b.span.0 }
    }

    /// A `Reaction` declaration as the last child of the owner.
    pub(crate) fn reaction(&mut self, r: &ast::Reaction, span: crate::msl::Span) -> DeclId {
        let id = self.new_decl(DeclKind::Error, None, span, false);
        let data = self.within(id, |lw| ReactionData { event: lw.expr(&r.event), body: lw.block(&r.body) });
        self.design.decl_mut(id).kind = DeclKind::Reaction(data);
        id
    }

    pub(crate) fn stmt(&mut self, s: &ast::Stmt) -> StmtId {
        let meta = self.meta(&s.meta);
        let kind = match &s.kind {
            ast::StmtKind::Let { pat, ty, init, else_ } => StmtKind::Let {
                pat: self.pat(pat, BindingOrigin::Local),
                ty: ty.as_ref().map(|t| self.ty(t)),
                init: init.as_ref().map(|e| self.expr(e)),
                else_: else_.as_ref().map(|b| self.block(b)),
            },
            ast::StmtKind::Expr { expr, semi } => StmtKind::Expr { expr: self.expr(expr), semi: *semi },
            ast::StmtKind::Item(item) => StmtKind::Item(self.item(item)),
            ast::StmtKind::Contribution { lhs, rhs } => {
                StmtKind::Contribution { lhs: self.expr(lhs), rhs: self.expr(rhs) }
            }
            ast::StmtKind::Commit { place, value, after } => StmtKind::Commit {
                place: self.expr(place),
                value: self.expr(value),
                after: after.as_ref().map(|e| self.expr(e)),
            },
            ast::StmtKind::Empty => StmtKind::Empty,
            ast::StmtKind::Signal { name: ident, ty, init } => {
                // The declaration is pushed first so its type and initial
                // value are owned by it.
                let id = self.new_decl(DeclKind::Error, Some(ident), s.span.0, false);
                let data = self.within(id, |lw| SignalData {
                    ty: lw.ty(ty),
                    init: init.as_ref().map(|e| lw.expr(e)),
                });
                self.design.decl_mut(id).kind = DeclKind::Signal(data);
                StmtKind::Signal(id)
            }
            ast::StmtKind::Reaction(r) => StmtKind::Reaction(self.reaction(r, s.span.0)),
        };
        self.design.push_stmt(StmtNode { meta, kind, span: s.span.0, owner: self.owner })
    }
}
