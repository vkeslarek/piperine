//! The reverse conversion: rebuild an `ast::File` from the arenas alone. It
//! never reads the retained tree (`SourceFile::ast`); that it can rebuild the
//! tree the parser returned is the proof that the POM keeps every construct.
//! Resolution results are dropped: they are not syntax.

use std::collections::BTreeSet;

use super::body::{
    Arg, Arm, Block, ClosureParam, ExprKind, FieldInit, FieldPatNode, GenericArgNode, GenericArgsNode, PatKind,
    PathNode, StmtKind, TypeKind,
};
use super::decl::{
    DeclData, DeclKind, FieldsForm, FnParamData, GenElseNode, GenericParamData, GenerationData, Name, NamespaceOrigin,
    NatureFieldNode, UseTreeKind, UseTreeNode,
};
use super::design::Design;
use super::ids::{DeclId, ExprId, PatId, PathId, StmtId, TypeId};
use super::meta::{AttrArgNode, AttrInputNode, Meta};
use super::reflect::SourceFile;
use crate::msl::{Span, ast};

impl SourceFile<'_> {
    /// The syntax tree of this file, rebuilt from the declaration tree and
    /// the body arenas, spans included.
    pub fn to_ast(self) -> ast::File {
        Rebuild::new(self.design()).file(self)
    }
}

/// The rebuilding walk over one design. `embedded` holds the declarations
/// that a statement of a block owns (items, signals, reactions): they are
/// children of the enclosing declaration like its own items, and this set
/// tells the two apart.
struct Rebuild<'d> {
    design: &'d Design,
    embedded: BTreeSet<DeclId>,
}

fn loc(span: Span) -> ast::Loc {
    ast::Loc(span)
}

fn ident(name: &Name) -> ast::Ident {
    ast::Ident { name: name.text.clone(), raw: name.raw, span: loc(name.span) }
}

impl<'d> Rebuild<'d> {
    fn new(design: &'d Design) -> Self {
        let embedded = (0..design.stmt_count())
            .filter_map(|i| match design.stmt_node(StmtId(i as u32)).kind {
                StmtKind::Item(d) | StmtKind::Signal(d) | StmtKind::Reaction(d) => Some(d),
                _ => None,
            })
            .collect();
        Rebuild { design, embedded }
    }

    fn data(&self, id: DeclId) -> &DeclData {
        self.design.decl_data(id)
    }

    // ---- file -----------------------------------------------------------------------------

    fn file(&self, file: SourceFile<'_>) -> ast::File {
        let root = self.data(self.design.file_root(file.id()));
        // A loaded file's namespace is its `mod b;` item, whose span is not
        // the file's: the file spans its whole text. Its `//!` comments are
        // told from the item's own docs by their kind and file.
        let external = !matches!(root.kind, DeclKind::Namespace(ref n) if n.origin == NamespaceOrigin::Root);
        let span = if external { Span::new(0, file.text().len()) } else { root.span };
        let inner_docs = root
            .meta
            .docs
            .iter()
            .filter(|d| d.inner && d.file == file.id())
            .map(|d| self.doc(d))
            .collect();
        let items = root.children.iter().map(|c| self.item(*c)).collect();
        ast::File { inner_docs, items, span: loc(span) }
    }

    fn doc(&self, d: &super::meta::Doc) -> ast::Doc {
        ast::Doc { text: d.text.clone(), block: d.block, span: loc(d.span) }
    }

    fn meta(&self, meta: &Meta) -> ast::Meta {
        ast::Meta {
            docs: meta.docs.iter().map(|d| self.doc(d)).collect(),
            attrs: meta
                .attrs
                .iter()
                .map(|a| ast::Attr {
                    path: self.path(a.path),
                    input: match &a.input {
                        AttrInputNode::None => ast::AttrInput::None,
                        AttrInputNode::Eq(e) => ast::AttrInput::Eq(self.expr(*e)),
                        AttrInputNode::List(args) => ast::AttrInput::List(
                            args.iter()
                                .map(|arg| match arg {
                                    AttrArgNode::Path(p) => ast::AttrArg::Path(self.path(*p)),
                                    AttrArgNode::Lit(l) => ast::AttrArg::Lit(l.clone()),
                                    AttrArgNode::NameValue(n, l) => ast::AttrArg::NameValue(ident(n), l.clone()),
                                })
                                .collect(),
                        ),
                    },
                    span: loc(a.span),
                })
                .collect(),
        }
    }

    // ---- paths, generic arguments, types --------------------------------------------------

    fn path(&self, id: PathId) -> ast::Path {
        let PathNode { segments, span, .. } = self.design.path_node(id);
        ast::Path {
            segments: segments
                .iter()
                .map(|s| ast::PathSeg { ident: ident(&s.name), args: s.args.as_ref().map(|a| self.generic_args(a)) })
                .collect(),
            span: loc(*span),
        }
    }

    fn generic_args(&self, g: &GenericArgsNode) -> ast::GenericArgs {
        match g {
            GenericArgsNode::Angle { args, turbofish, span } => ast::GenericArgs::Angle {
                args: args
                    .iter()
                    .map(|a| match a {
                        GenericArgNode::Type(t) => ast::GenericArg::Type(self.ty(*t)),
                        GenericArgNode::Const(e) => ast::GenericArg::Const(self.expr(*e)),
                    })
                    .collect(),
                turbofish: *turbofish,
                span: loc(*span),
            },
            GenericArgsNode::Paren { inputs, output, span } => ast::GenericArgs::Paren {
                inputs: inputs.iter().map(|t| self.ty(*t)).collect(),
                output: output.map(|t| Box::new(self.ty(t))),
                span: loc(*span),
            },
        }
    }

    fn ty(&self, id: TypeId) -> ast::Type {
        let node = self.design.type_node(id);
        let kind = match &node.kind {
            TypeKind::Path(p) => ast::TypeKind::Path(self.path(*p)),
            TypeKind::Array { elem, len } => {
                ast::TypeKind::Array { elem: Box::new(self.ty(*elem)), len: Box::new(self.expr(*len)) }
            }
            TypeKind::Slice(inner) => ast::TypeKind::Slice(Box::new(self.ty(*inner))),
            TypeKind::Tuple(items) => ast::TypeKind::Tuple(items.iter().map(|t| self.ty(*t)).collect()),
            TypeKind::FnPtr { params, ret } => ast::TypeKind::FnPtr {
                params: params.iter().map(|t| self.ty(*t)).collect(),
                ret: ret.map(|r| Box::new(self.ty(r))),
            },
            TypeKind::Ref { static_lifetime, inner } => {
                ast::TypeKind::Ref { static_lifetime: *static_lifetime, inner: Box::new(self.ty(*inner)) }
            }
            TypeKind::Flip(inner) => ast::TypeKind::Flip(Box::new(self.ty(*inner))),
            TypeKind::Infer => ast::TypeKind::Infer,
        };
        ast::Type { kind, span: loc(node.span) }
    }

    // ---- patterns -------------------------------------------------------------------------

    fn pat(&self, id: PatId) -> ast::Pat {
        let node = self.design.pat_node(id);
        let pats = |items: &[PatId]| items.iter().map(|p| self.pat(*p)).collect::<Vec<_>>();
        let kind = match &node.kind {
            PatKind::Wild => ast::PatKind::Wild,
            PatKind::Rest => ast::PatKind::Rest,
            PatKind::Ident { by_ref, mutable, name, sub, .. } => ast::PatKind::Ident {
                by_ref: *by_ref,
                mutable: *mutable,
                name: ident(name),
                sub: sub.map(|s| Box::new(self.pat(s))),
            },
            PatKind::Lit(e) => ast::PatKind::Lit(Box::new(self.expr(*e))),
            PatKind::Path(p) => ast::PatKind::Path(self.path(*p)),
            PatKind::Tuple(items) => ast::PatKind::Tuple(pats(items)),
            PatKind::TupleStruct { path, pats: items } => {
                ast::PatKind::TupleStruct { path: self.path(*path), pats: pats(items) }
            }
            PatKind::Struct { path, fields, rest } => ast::PatKind::Struct {
                path: self.path(*path),
                fields: fields.iter().map(|f| self.field_pat(f)).collect(),
                rest: *rest,
            },
            PatKind::Or(items) => ast::PatKind::Or(pats(items)),
            PatKind::Range { lo, hi, inclusive } => ast::PatKind::Range {
                lo: lo.map(|e| Box::new(self.expr(e))),
                hi: hi.map(|e| Box::new(self.expr(e))),
                inclusive: *inclusive,
            },
            PatKind::Ref(inner) => ast::PatKind::Ref(Box::new(self.pat(*inner))),
            PatKind::Slice(items) => ast::PatKind::Slice(pats(items)),
        };
        ast::Pat { kind, span: loc(node.span) }
    }

    fn field_pat(&self, f: &FieldPatNode) -> ast::FieldPat {
        ast::FieldPat { name: ident(&f.name), pat: f.pat.map(|p| self.pat(p)), span: loc(f.span) }
    }

    // ---- expressions ----------------------------------------------------------------------

    fn arg(&self, a: &Arg) -> ast::Arg {
        ast::Arg { name: a.name.as_ref().map(ident), value: self.expr(a.value), span: loc(a.span) }
    }

    fn field_init(&self, f: &FieldInit) -> ast::FieldInit {
        ast::FieldInit { name: ident(&f.name), value: self.expr(f.value), shorthand: f.shorthand, span: loc(f.span) }
    }

    fn boxed(&self, id: ExprId) -> Box<ast::Expr> {
        Box::new(self.expr(id))
    }

    fn opt_boxed(&self, id: Option<ExprId>) -> Option<Box<ast::Expr>> {
        id.map(|e| self.boxed(e))
    }

    fn expr(&self, id: ExprId) -> ast::Expr {
        let node = self.design.expr_node(id);
        let exprs = |items: &[ExprId]| items.iter().map(|e| self.expr(*e)).collect::<Vec<_>>();
        let kind = match &node.kind {
            ExprKind::Lit(l) => ast::ExprKind::Lit(l.clone()),
            ExprKind::Path(p) => ast::ExprKind::Path(self.path(*p)),
            ExprKind::Unary { op, expr } => ast::ExprKind::Unary { op: *op, expr: self.boxed(*expr) },
            ExprKind::Binary { op, lhs, rhs } => {
                ast::ExprKind::Binary { op: *op, lhs: self.boxed(*lhs), rhs: self.boxed(*rhs) }
            }
            ExprKind::Assign { op, lhs, rhs } => {
                ast::ExprKind::Assign { op: *op, lhs: self.boxed(*lhs), rhs: self.boxed(*rhs) }
            }
            ExprKind::Cast { expr, ty } => ast::ExprKind::Cast { expr: self.boxed(*expr), ty: self.ty(*ty) },
            ExprKind::Call { callee, args, params, .. } => ast::ExprKind::Call {
                callee: self.boxed(*callee),
                args: args.iter().map(|a| self.arg(a)).collect(),
                params: params.as_ref().map(|ps| ps.iter().map(|f| self.field_init(f)).collect()),
            },
            ExprKind::MethodCall { recv, method, generics, args, .. } => ast::ExprKind::MethodCall {
                recv: self.boxed(*recv),
                method: ident(method),
                generics: generics.as_ref().map(|g| self.generic_args(g)),
                args: args.iter().map(|a| self.arg(a)).collect(),
            },
            ExprKind::Field { expr, name, .. } => ast::ExprKind::Field { expr: self.boxed(*expr), name: ident(name) },
            ExprKind::Index { expr, index } => {
                ast::ExprKind::Index { expr: self.boxed(*expr), index: self.boxed(*index) }
            }
            ExprKind::Range { lo, hi, inclusive } => {
                ast::ExprKind::Range { lo: self.opt_boxed(*lo), hi: self.opt_boxed(*hi), inclusive: *inclusive }
            }
            ExprKind::Paren(inner) => ast::ExprKind::Paren(self.boxed(*inner)),
            ExprKind::Tuple(items) => ast::ExprKind::Tuple(exprs(items)),
            ExprKind::Array(items) => ast::ExprKind::Array(exprs(items)),
            ExprKind::ArrayRepeat { elem, len } => {
                ast::ExprKind::ArrayRepeat { elem: self.boxed(*elem), len: self.boxed(*len) }
            }
            ExprKind::Struct { path, fields, .. } => ast::ExprKind::Struct {
                path: self.path(*path),
                fields: fields.iter().map(|f| self.field_init(f)).collect(),
            },
            ExprKind::Block(b) => ast::ExprKind::Block(self.block(b)),
            ExprKind::If { cond, then, else_ } => ast::ExprKind::If {
                cond: self.boxed(*cond),
                then: self.block(then),
                else_: self.opt_boxed(*else_),
            },
            ExprKind::Let { pat, expr } => ast::ExprKind::Let { pat: self.pat(*pat), expr: self.boxed(*expr) },
            ExprKind::Match { scrutinee, arms } => ast::ExprKind::Match {
                scrutinee: self.boxed(*scrutinee),
                arms: arms.iter().map(|a| self.arm(a)).collect(),
            },
            ExprKind::While { cond, body } => ast::ExprKind::While { cond: self.boxed(*cond), body: self.block(body) },
            ExprKind::Loop { body } => ast::ExprKind::Loop { body: self.block(body) },
            ExprKind::For { pat, iter, body } => {
                ast::ExprKind::For { pat: self.pat(*pat), iter: self.boxed(*iter), body: self.block(body) }
            }
            ExprKind::Break(v) => ast::ExprKind::Break(self.opt_boxed(*v)),
            ExprKind::Continue => ast::ExprKind::Continue,
            ExprKind::Return(v) => ast::ExprKind::Return(self.opt_boxed(*v)),
            ExprKind::Closure { is_move, params, ret, body } => ast::ExprKind::Closure {
                is_move: *is_move,
                params: params.iter().map(|p| self.closure_param(p)).collect(),
                ret: ret.map(|t| self.ty(t)),
                body: self.boxed(*body),
            },
            ExprKind::Try(inner) => ast::ExprKind::Try(self.boxed(*inner)),
            ExprKind::Emit { stream, fields } => ast::ExprKind::Emit {
                stream: stream.clone(),
                fields: fields.iter().map(|f| self.field_init(f)).collect(),
            },
            ExprKind::IncludeStr(l) => ast::ExprKind::IncludeStr(l.clone()),
            ExprKind::Error => ast::ExprKind::Error,
        };
        ast::Expr { kind, span: loc(node.span) }
    }

    fn arm(&self, a: &Arm) -> ast::Arm {
        ast::Arm {
            meta: self.meta(&a.meta),
            pat: self.pat(a.pat),
            guard: a.guard.map(|g| self.expr(g)),
            body: self.expr(a.body),
            span: loc(a.span),
        }
    }

    fn closure_param(&self, p: &ClosureParam) -> ast::ClosureParam {
        ast::ClosureParam { pat: self.pat(p.pat), ty: p.ty.map(|t| self.ty(t)), span: loc(p.span) }
    }

    // ---- statements -----------------------------------------------------------------------

    fn block(&self, b: &Block) -> ast::Block {
        ast::Block { stmts: b.stmts.iter().map(|s| self.stmt(*s)).collect(), span: loc(b.span) }
    }

    fn stmt(&self, id: StmtId) -> ast::Stmt {
        let node = self.design.stmt_node(id);
        let kind = match &node.kind {
            StmtKind::Let { pat, ty, init, else_ } => ast::StmtKind::Let {
                pat: self.pat(*pat),
                ty: ty.map(|t| self.ty(t)),
                init: init.map(|e| self.expr(e)),
                else_: else_.as_ref().map(|b| self.block(b)),
            },
            StmtKind::Expr { expr, semi } => ast::StmtKind::Expr { expr: self.expr(*expr), semi: *semi },
            StmtKind::Item(d) => ast::StmtKind::Item(Box::new(self.item(*d))),
            StmtKind::Contribution { lhs, rhs } => {
                ast::StmtKind::Contribution { lhs: self.expr(*lhs), rhs: self.expr(*rhs) }
            }
            StmtKind::Commit { place, value, after } => ast::StmtKind::Commit {
                place: self.expr(*place),
                value: self.expr(*value),
                after: after.map(|e| self.expr(e)),
            },
            StmtKind::Empty => ast::StmtKind::Empty,
            StmtKind::Signal(d) => {
                let DeclKind::Signal(s) = &self.data(*d).kind else { unreachable!("a Signal statement holds a Signal") };
                ast::StmtKind::Signal { name: ident(self.name(*d)), ty: self.ty(s.ty), init: s.init.map(|e| self.expr(e)) }
            }
            StmtKind::Reaction(d) => ast::StmtKind::Reaction(self.reaction(*d)),
        };
        ast::Stmt { meta: self.meta(&node.meta), kind, span: loc(node.span) }
    }

    fn reaction(&self, d: DeclId) -> ast::Reaction {
        let DeclKind::Reaction(r) = &self.data(d).kind else { unreachable!("a Reaction statement holds a Reaction") };
        ast::Reaction { event: self.expr(r.event), body: self.block(&r.body) }
    }

    // ---- declarations ---------------------------------------------------------------------

    /// The declared name of a declaration that always has one.
    fn name(&self, d: DeclId) -> &Name {
        self.data(d).name.as_ref().expect("this declaration kind always has a name")
    }

    /// The children of `d` of one kind, in order.
    fn children_of<T>(&self, d: DeclId, pick: impl Fn(DeclId, &DeclData) -> Option<T>) -> Vec<T> {
        self.data(d).children.iter().filter(|c| !self.embedded.contains(c)).filter_map(|c| pick(*c, self.data(*c))).collect()
    }

    fn generics(&self, d: DeclId) -> Option<ast::Generics> {
        let span = self.data(d).generics_span?;
        let params = self.children_of(d, |c, data| match &data.kind {
            DeclKind::GenericParam(p) => Some(ast::GenericParam {
                name: ident(self.name(c)),
                kind: match p {
                    GenericParamData::Type { bounds, default } => ast::GenericParamKind::Type {
                        bounds: bounds.iter().map(|b| self.path(*b)).collect(),
                        default: default.map(|t| self.ty(t)),
                    },
                    GenericParamData::Const { ty, default } => {
                        ast::GenericParamKind::Const { ty: self.ty(*ty), default: default.map(|e| self.expr(e)) }
                    }
                },
                span: loc(data.span),
            }),
            _ => None,
        });
        Some(ast::Generics { params, span: loc(span) })
    }

    fn const_item(&self, d: DeclId) -> ast::ConstItem {
        let DeclKind::Const(c) = &self.data(d).kind else { unreachable!("caller matched Const") };
        ast::ConstItem { name: ident(self.name(d)), ty: self.ty(c.ty), value: c.value.map(|e| self.expr(e)) }
    }

    fn fn_decl(&self, d: DeclId) -> ast::FnDecl {
        let DeclKind::Fn(f) = &self.data(d).kind else { unreachable!("caller matched Fn") };
        let params = self.children_of(d, |c, data| match &data.kind {
            DeclKind::FnParam(FnParamData::SelfValue) => Some(ast::FnParam::SelfValue(loc(data.span))),
            DeclKind::FnParam(FnParamData::Named { ty }) => {
                Some(ast::FnParam::Named { name: ident(self.name(c)), ty: self.ty(*ty), span: loc(data.span) })
            }
            DeclKind::FnParam(FnParamData::Error) => Some(ast::FnParam::Error(loc(data.span))),
            _ => None,
        });
        ast::FnDecl {
            is_const: f.is_const,
            abi: f.abi.clone(),
            name: ident(self.name(d)),
            generics: self.generics(d),
            params,
            ret: f.ret.map(|t| self.ty(t)),
            where_: f.where_.map(|e| self.expr(e)),
            body: f.body.as_ref().map(|b| self.block(b)),
        }
    }

    /// The docs and attributes of a declaration. A `mod b;` item that loaded
    /// a file also holds that file's `//!` comments: only the docs the
    /// declaring file wrote (`Doc::inner` is false) belong to the item.
    fn item_meta(&self, data: &DeclData) -> ast::Meta {
        let mut meta = self.meta(&data.meta);
        let outer = data.meta.docs.iter().filter(|d| !d.inner);
        meta.docs = outer.map(|d| self.doc(d)).collect();
        meta
    }

    /// One item of a file, a module or a block.
    fn item(&self, d: DeclId) -> ast::Item {
        let data = self.data(d);
        let kind = match &data.kind {
            DeclKind::Const(_) => ast::ItemKind::Const(self.const_item(d)),
            DeclKind::Fn(_) => ast::ItemKind::Fn(self.fn_decl(d)),
            DeclKind::Struct(s) => ast::ItemKind::Struct(ast::StructDecl {
                name: ident(self.name(d)),
                generics: self.generics(d),
                fields: self.fields(d, s.form),
            }),
            DeclKind::Enum(_) => ast::ItemKind::Enum(ast::EnumDecl {
                name: ident(self.name(d)),
                generics: self.generics(d),
                variants: self.children_of(d, |c, v| match &v.kind {
                    DeclKind::Variant(vd) => Some(ast::Variant {
                        meta: self.meta(&v.meta),
                        name: ident(self.name(c)),
                        fields: self.fields(c, vd.form),
                        discriminant: vd.discriminant.map(|e| self.expr(e)),
                        span: loc(v.span),
                    }),
                    _ => None,
                }),
            }),
            DeclKind::Trait(t) => ast::ItemKind::Trait(ast::TraitDecl {
                name: ident(self.name(d)),
                generics: self.generics(d),
                supertraits: t.supertraits.iter().map(|p| self.path(*p)).collect(),
                items: self.impl_items(d),
            }),
            DeclKind::Impl(i) => ast::ItemKind::Impl(ast::ImplBlock {
                generics: self.generics(d),
                trait_: i.trait_.map(|p| self.path(p)),
                self_ty: self.ty(i.self_ty),
                items: self.impl_items(d),
            }),
            DeclKind::Use(u) => ast::ItemKind::Use(use_tree(&u.tree)),
            DeclKind::Namespace(n) => ast::ItemKind::Mod(ast::ModItem {
                name: ident(self.name(d)),
                items: match n.origin {
                    NamespaceOrigin::Inline => Some(self.children_of(d, |c, _| Some(self.item(c)))),
                    NamespaceOrigin::External { .. } | NamespaceOrigin::Root => None,
                },
            }),
            DeclKind::Nature(n) => ast::ItemKind::Nature(ast::NatureDecl {
                name: ident(self.name(d)),
                generics: self.generics(d),
                fields: n.fields.iter().map(|f| self.nature_field(f)).collect(),
            }),
            DeclKind::Module(m) => ast::ItemKind::Module(self.module_decl(d, m)),
            DeclKind::Behavior(b) => ast::ItemKind::Behavior(self.behavior(d, b)),
            DeclKind::Error => ast::ItemKind::Error,
            other => unreachable!("a {} is not an item", other.label()),
        };
        ast::Item { meta: self.item_meta(data), public: data.public, kind, span: loc(data.span) }
    }

    /// The fields of a struct or variant: its `Field` children, in the form
    /// it was written.
    fn fields(&self, d: DeclId, form: FieldsForm) -> ast::Fields {
        let fields = self.children_of(d, |_, f| match &f.kind {
            DeclKind::Field(fd) => Some(ast::Field {
                meta: self.meta(&f.meta),
                public: f.public,
                dir: fd.dir,
                name: f.name.as_ref().map(ident),
                ty: self.ty(fd.ty),
                span: loc(f.span),
            }),
            _ => None,
        });
        match form {
            FieldsForm::Named => ast::Fields::Named(fields),
            FieldsForm::Tuple => ast::Fields::Tuple(fields),
            FieldsForm::Unit => ast::Fields::Unit,
        }
    }

    /// The functions and constants of an `impl` or a `trait`.
    fn impl_items(&self, d: DeclId) -> Vec<ast::ImplItem> {
        self.children_of(d, |c, data| {
            let kind = match &data.kind {
                DeclKind::Fn(_) => ast::ImplItemKind::Fn(self.fn_decl(c)),
                DeclKind::Const(_) => ast::ImplItemKind::Const(self.const_item(c)),
                _ => return None,
            };
            Some(ast::ImplItem { meta: self.meta(&data.meta), public: data.public, kind, span: loc(data.span) })
        })
    }

    fn nature_field(&self, f: &NatureFieldNode) -> ast::NatureField {
        match f {
            NatureFieldNode::Quantity(q) => {
                let data = self.data(*q);
                let DeclKind::Quantity(qd) = &data.kind else { unreachable!("a nature field names a Quantity") };
                ast::NatureField {
                    meta: self.meta(&data.meta),
                    kind: ast::NatureFieldKind::Quantity {
                        which: qd.role,
                        name: ident(self.name(*q)),
                        attrs: qd.attrs.iter().map(|a| self.field_init(a)).collect(),
                    },
                    span: loc(data.span),
                }
            }
            NatureFieldNode::Value { meta, ty, span } => ast::NatureField {
                meta: self.meta(meta),
                kind: ast::NatureFieldKind::Value(self.ty(*ty)),
                span: loc(*span),
            },
            NatureFieldNode::Resolve { meta, expr, span } => ast::NatureField {
                meta: self.meta(meta),
                kind: ast::NatureFieldKind::Resolve(self.expr(*expr)),
                span: loc(*span),
            },
        }
    }

    // ---- modules and behavior -------------------------------------------------------------

    fn module_decl(&self, d: DeclId, m: &super::decl::ModuleData) -> ast::ModuleDecl {
        let ports = self.children_of(d, |c, p| match &p.kind {
            DeclKind::Port(pd) => Some(ast::Port {
                meta: self.meta(&p.meta),
                dir: pd.dir,
                name: ident(self.name(c)),
                ty: self.ty(pd.ty),
                default: pd.default.map(|e| self.expr(e)),
                span: loc(p.span),
            }),
            _ => None,
        });
        ast::ModuleDecl {
            abi: m.abi.clone(),
            name: ident(self.name(d)),
            generics: self.generics(d),
            ports,
            ret: m.ret.map(|t| self.ty(t)),
            where_: m.where_.map(|e| self.expr(e)),
            body: self.children_of(d, |c, data| self.module_item(c, data)),
            result: m.result.map(|e| self.expr(e)),
        }
    }

    /// A body item of a module or a generation; none for a child of another
    /// kind (a generic parameter, a port, a binding).
    fn module_item(&self, c: DeclId, data: &DeclData) -> Option<ast::ModuleItem> {
        let kind = match &data.kind {
            DeclKind::Param(p) => ast::ModuleItemKind::Param {
                name: ident(self.name(c)),
                ty: self.ty(p.ty),
                default: p.default.map(|e| self.expr(e)),
                where_: p.where_.map(|e| self.expr(e)),
            },
            DeclKind::Signal(s) => ast::ModuleItemKind::Signal {
                name: ident(self.name(c)),
                ty: self.ty(s.ty),
                init: s.init.map(|e| self.expr(e)),
            },
            DeclKind::Let(l) => ast::ModuleItemKind::Let { pat: self.pat(l.pat), value: self.expr(l.value) },
            DeclKind::Definition(def) => ast::ModuleItemKind::Def {
                place: self.expr(def.place),
                value: self.expr(def.value),
                after: def.after.map(|e| self.expr(e)),
            },
            DeclKind::Generation(_) => ast::ModuleItemKind::Gen(self.generation(c, &|x| self.module_item_of(x))),
            DeclKind::Error => ast::ModuleItemKind::Error,
            _ => return None,
        };
        Some(ast::ModuleItem { meta: self.meta(&data.meta), public: data.public, kind, span: loc(data.span) })
    }

    fn module_item_of(&self, c: DeclId) -> ast::ModuleItem {
        self.module_item(c, self.data(c)).expect("a generation holds module items")
    }

    /// `gen<X>`: a generation, its items rebuilt by `item`.
    fn generation<X>(&self, d: DeclId, item: &dyn Fn(DeclId) -> X) -> ast::Gen<X> {
        let data = self.data(d);
        let DeclKind::Generation(g) = &data.kind else { unreachable!("caller matched Generation") };
        let items = |ids: &[DeclId]| ids.iter().map(|i| item(*i)).collect::<Vec<_>>();
        let kind = match g {
            GenerationData::For { pat, iter, body } => {
                ast::GenKind::For { pat: self.pat(*pat), iter: self.expr(*iter), body: items(body) }
            }
            GenerationData::If { cond, then, else_ } => ast::GenKind::If {
                cond: self.expr(*cond),
                then: items(then),
                else_: else_.as_ref().map(|e| {
                    Box::new(match e {
                        GenElseNode::Block(ids) => ast::GenElse::Block(items(ids)),
                        GenElseNode::If(nested) => ast::GenElse::If(self.generation(*nested, item)),
                    })
                }),
            },
        };
        ast::Gen { kind, span: loc(data.span) }
    }

    fn behavior(&self, d: DeclId, b: &super::decl::BehaviorData) -> ast::BehaviorBlock {
        let body = match b.kind {
            super::body::BehaviorKind::Analog => ast::BehaviorBody::Analog(b.body.iter().map(|s| self.stmt(*s)).collect()),
            super::body::BehaviorKind::Digital => {
                ast::BehaviorBody::Digital(self.children_of(d, |c, data| self.digital_item(c, data)))
            }
        };
        ast::BehaviorBlock {
            kind: b.kind,
            names: b.names.iter().map(ident).collect(),
            target: self.path(b.target),
            body,
        }
    }

    fn digital_item(&self, c: DeclId, data: &DeclData) -> Option<ast::DigitalItem> {
        let kind = match &data.kind {
            DeclKind::Reaction(_) => ast::DigitalItemKind::Reaction(self.reaction(c)),
            DeclKind::Generation(_) => ast::DigitalItemKind::Gen(self.generation(c, &|x| self.digital_item_of(x))),
            DeclKind::Error => ast::DigitalItemKind::Error,
            _ => return None,
        };
        Some(ast::DigitalItem { meta: self.meta(&data.meta), kind, span: loc(data.span) })
    }

    fn digital_item_of(&self, c: DeclId) -> ast::DigitalItem {
        self.digital_item(c, self.data(c)).expect("a generation holds digital items")
    }
}

fn use_tree(t: &UseTreeNode) -> ast::UseTree {
    let kind = match &t.kind {
        UseTreeKind::Simple { alias } => ast::UseKind::Simple { alias: alias.as_ref().map(ident) },
        UseTreeKind::Glob => ast::UseKind::Glob,
        UseTreeKind::Group(children) => ast::UseKind::Group(children.iter().map(use_tree).collect()),
    };
    ast::UseTree { path: t.path.iter().map(ident).collect(), kind, span: loc(t.span) }
}
