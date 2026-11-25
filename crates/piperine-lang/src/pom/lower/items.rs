//! Value items: functions and the generic parameters every item kind shares.

use super::Lowerer;
use crate::msl::ast;
use crate::pom::decl::{DeclKind, EnumData, FieldData, FieldsForm, FnData, FnParamData, GenericParamData, ImplData, Import, StructData, TraitData, UseData, UseTreeKind, UseTreeNode, VariantData};
use crate::pom::body::{PathNode, PathSegment};
use crate::pom::resolution::Resolution;
use super::body::name;

impl Lowerer<'_> {
    /// One `GenericParam` declaration per parameter, as children of the
    /// current owner, in source order. Shared by every item that has
    /// generics.
    pub(crate) fn generics(&mut self, generics: &Option<ast::Generics>) {
        let Some(g) = generics else { return };
        self.design.decl_mut(self.owner).generics_span = Some(g.span.0);
        for p in &g.params {
            let id = self.new_decl(DeclKind::Error, Some(&p.name), p.span.0, false);
            let data = self.within(id, |lw| match &p.kind {
                ast::GenericParamKind::Type { bounds, default } => GenericParamData::Type {
                    bounds: bounds.iter().map(|b| lw.path(b)).collect(),
                    default: default.as_ref().map(|t| lw.ty(t)),
                },
                ast::GenericParamKind::Const { ty, default } => GenericParamData::Const {
                    ty: lw.ty(ty),
                    default: default.as_ref().map(|e| lw.expr(e)),
                },
            });
            self.design.decl_mut(id).kind = DeclKind::GenericParam(data);
        }
    }

    /// A function: generics, then parameters, then the body, all children of
    /// the function declaration (the current owner).
    pub(crate) fn fn_decl(&mut self, f: &ast::FnDecl) -> DeclKind {
        self.generics(&f.generics);
        for p in &f.params {
            self.fn_param(p);
        }
        let ret = f.ret.as_ref().map(|t| self.ty(t));
        let where_ = f.where_.as_ref().map(|e| self.expr(e));
        let body = f.body.as_ref().map(|b| self.block(b));
        DeclKind::Fn(FnData { is_const: f.is_const, abi: f.abi.clone(), ret, where_, body })
    }

    fn fn_param(&mut self, p: &ast::FnParam) {
        match p {
            ast::FnParam::SelfValue(loc) => {
                let name = ast::Ident { name: "self".to_string(), raw: false, span: *loc };
                self.new_decl(DeclKind::FnParam(FnParamData::SelfValue), Some(&name), loc.0, false);
            }
            ast::FnParam::Named { name, ty, span } => {
                let id = self.new_decl(DeclKind::Error, Some(name), span.0, false);
                let ty = self.within(id, |lw| lw.ty(ty));
                self.design.decl_mut(id).kind = DeclKind::FnParam(FnParamData::Named { ty });
            }
            ast::FnParam::Error(loc) => {
                self.new_decl(DeclKind::FnParam(FnParamData::Error), None, loc.0, false);
            }
        }
    }

    /// A struct: generics, then fields, as children of the struct.
    pub(crate) fn struct_decl(&mut self, s: &ast::StructDecl) -> DeclKind {
        self.generics(&s.generics);
        let form = self.fields(&s.fields);
        DeclKind::Struct(StructData { form, bundle: None })
    }

    /// An enum: generics, then one `Variant` per variant.
    pub(crate) fn enum_decl(&mut self, e: &ast::EnumDecl) -> DeclKind {
        self.generics(&e.generics);
        for v in &e.variants {
            let id = self.new_decl(DeclKind::Error, Some(&v.name), v.span.0, false);
            let (kind, meta) = self.within(id, |lw| {
                let form = lw.fields(&v.fields);
                let discriminant = v.discriminant.as_ref().map(|d| lw.expr(d));
                (DeclKind::Variant(VariantData { form, discriminant }), lw.meta(&v.meta))
            });
            let decl = self.design.decl_mut(id);
            decl.kind = kind;
            decl.meta = meta;
        }
        DeclKind::Enum(EnumData::default())
    }

    /// One `Field` child of the current owner per field; the form as written.
    fn fields(&mut self, fields: &ast::Fields) -> FieldsForm {
        let (list, form) = match fields {
            ast::Fields::Named(f) => (f.as_slice(), FieldsForm::Named),
            ast::Fields::Tuple(f) => (f.as_slice(), FieldsForm::Tuple),
            ast::Fields::Unit => return FieldsForm::Unit,
        };
        for f in list {
            let id = self.new_decl(DeclKind::Error, f.name.as_ref(), f.span.0, f.public);
            let (kind, meta) = self.within(id, |lw| {
                (DeclKind::Field(FieldData { dir: f.dir, ty: lw.ty(&f.ty), init: f.init.as_ref().map(|init| lw.expr(init)) }), lw.meta(&f.meta))
            });
            let decl = self.design.decl_mut(id);
            decl.kind = kind;
            decl.meta = meta;
        }
        form
    }

    /// A trait: generics, then its items, as children of the trait.
    pub(crate) fn trait_decl(&mut self, t: &ast::TraitDecl) -> DeclKind {
        self.generics(&t.generics);
        let supertraits = t.supertraits.iter().map(|p| self.path(p)).collect();
        self.impl_items(&t.items);
        DeclKind::Trait(TraitData { supertraits })
    }

    /// An `impl`: generics, then its items, as children of the block.
    pub(crate) fn impl_decl(&mut self, i: &ast::ImplBlock) -> DeclKind {
        self.generics(&i.generics);
        let trait_ = i.trait_.as_ref().map(|p| self.path(p));
        let self_ty = self.ty(&i.self_ty);
        self.impl_items(&i.items);
        DeclKind::Impl(ImplData { trait_, self_ty })
    }

    /// The functions and constants of an `impl` or a `trait`.
    fn impl_items(&mut self, items: &[ast::ImplItem]) {
        for item in items {
            let name = match &item.kind {
                ast::ImplItemKind::Fn(f) => &f.name,
                ast::ImplItemKind::Const(c) => &c.name,
            };
            let id = self.new_decl(DeclKind::Error, Some(name), item.span.0, item.public);
            let (kind, meta) = self.within(id, |lw| {
                let kind = match &item.kind {
                    ast::ImplItemKind::Fn(f) => lw.fn_decl(f),
                    ast::ImplItemKind::Const(c) => lw.const_item(c),
                };
                (kind, lw.meta(&item.meta))
            });
            let decl = self.design.decl_mut(id);
            decl.kind = kind;
            decl.meta = meta;
        }
    }

    /// A `use`: the tree as written plus the flattened imports.
    pub(crate) fn use_decl(&mut self, u: &ast::UseTree) -> DeclKind {
        let tree = use_tree(u);
        let mut imports = Vec::new();
        self.flatten_use(&tree, &mut Vec::new(), &mut imports);
        DeclKind::Use(UseData { tree, imports })
    }

    /// One import per leaf, each with the prefixes above it.
    fn flatten_use(&mut self, node: &UseTreeNode, prefix: &mut Vec<crate::pom::decl::Name>, out: &mut Vec<Import>) {
        let depth = prefix.len();
        prefix.extend(node.path.iter().cloned());
        match &node.kind {
            UseTreeKind::Group(children) => {
                for c in children {
                    self.flatten_use(c, prefix, out);
                }
            }
            UseTreeKind::Simple { alias } => out.push(self.import(prefix, node, alias.clone(), false)),
            UseTreeKind::Glob => out.push(self.import(prefix, node, None, true)),
        }
        prefix.truncate(depth);
    }

    /// One leaf. Its path holds every segment from the root of the tree, but
    /// its span covers only the segments the leaf itself writes (a leaf of a
    /// group never includes the braces around it). A `self` leaf names the
    /// namespace its prefix names, so it drops the `self` segment.
    fn import(&mut self, path: &[crate::pom::decl::Name], leaf: &UseTreeNode, alias: Option<crate::pom::decl::Name>, glob: bool) -> Import {
        let own = match (leaf.path.first(), leaf.path.last()) {
            (Some(first), Some(last)) => crate::msl::Span::new(first.span.start, last.span.end),
            _ => leaf.span,
        };
        let takes_prefix = !glob && path.len() > 1 && leaf.path.last().is_some_and(|n| n.text == "self");
        let kept = if takes_prefix { &path[..path.len() - 1] } else { path };
        let segments = kept
            .iter()
            .map(|n| PathSegment { name: n.clone(), args: None, res: Resolution::Unresolved })
            .collect();
        let path = self.design.push_path(PathNode { segments, span: own, owner: self.owner });
        Import { path, alias, glob }
    }
}

fn use_tree(u: &ast::UseTree) -> UseTreeNode {
    let kind = match &u.kind {
        ast::UseKind::Simple { alias } => UseTreeKind::Simple { alias: alias.as_ref().map(name) },
        ast::UseKind::Glob => UseTreeKind::Glob,
        ast::UseKind::Group(children) => UseTreeKind::Group(children.iter().map(use_tree).collect()),
    };
    UseTreeNode { path: u.path.iter().map(name).collect(), kind, span: u.span.0 }
}
