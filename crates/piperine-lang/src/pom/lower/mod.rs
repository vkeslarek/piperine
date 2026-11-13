//! Stage 1: the syntax tree to the arenas. A pure function of the tree: it
//! reads no scope and resolves nothing. Only this module pushes to the arenas.
//!
//! `mod.rs` holds the walk state and the declaration spine; `body.rs` lowers
//! paths, types, patterns, expressions and statements; `items.rs` lowers the
//! value items.

mod body;
mod files;
mod hardware;
mod items;
mod meta;

use super::decl::{ConstData, DeclData, DeclKind, NamespaceData, NamespaceOrigin};
use super::design::Design;
use super::meta::Meta;
use super::source::SourceProvider;
use std::path::PathBuf;
use super::ids::{DeclId, FileId};
use crate::msl::Span;
use crate::msl::ast;

/// The state of one lowering walk: the design under construction, the file
/// being read and the nearest enclosing declaration.
pub(crate) struct Lowerer<'a> {
    pub(crate) design: &'a mut Design,
    pub(crate) file: FileId,
    pub(crate) owner: DeclId,
    /// Where `mod b;` looks for its file.
    pub(crate) provider: &'a dyn SourceProvider,
    /// The path of the file being lowered, and whether it is a root file.
    pub(crate) path: PathBuf,
    pub(crate) is_root: bool,
    /// The enclosing inline modules of the current file, outermost first.
    pub(crate) inline: Vec<String>,
    /// The canonical paths of the files being loaded, outermost first.
    pub(crate) chain: Vec<PathBuf>,
}

/// Lower every item of `ast` as a child of the namespace `ns`.
pub(crate) fn lower_file(
    design: &mut Design,
    file: FileId,
    ns: DeclId,
    ast: &ast::File,
    provider: &dyn SourceProvider,
    path: PathBuf,
) {
    let chain = vec![provider.canonical(&path)];
    let mut lw = Lowerer { design, file, owner: ns, provider, path, is_root: true, inline: Vec::new(), chain };
    // The `//!` comments at the top of a file are the file namespace's docs.
    lw.design.decl_mut(ns).meta.docs = ast.inner_docs.iter().map(|d| meta::doc(d, true, file)).collect();
    for item in &ast.items {
        lw.item(item);
    }
}

impl Lowerer<'_> {
    /// Push a declaration as the last child of the current owner.
    pub(crate) fn new_decl(
        &mut self,
        kind: DeclKind,
        name: Option<&ast::Ident>,
        span: Span,
        public: bool,
    ) -> DeclId {
        let data = DeclData {
            kind,
            name: name.map(|n| super::decl::Name { text: n.name.clone(), span: n.span.0 }),
            parent: Some(self.owner),
            children: Vec::new(),
            file: self.file,
            span,
            generics_span: None,
            public,
            meta: Meta::default(),
        };
        let id = self.design.push_decl(data);
        self.design.decl_mut(self.owner).children.push(id);
        id
    }

    /// Run `f` with `owner` as the enclosing declaration, then restore.
    pub(crate) fn within<R>(&mut self, owner: DeclId, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = std::mem::replace(&mut self.owner, owner);
        let out = f(self);
        self.owner = saved;
        out
    }

    /// One declaration per item. The declaration is pushed first (so ids
    /// follow source order and the item's own bodies have an owner), then its
    /// payload is lowered with the declaration as owner and stored.
    pub(crate) fn item(&mut self, item: &ast::Item) -> DeclId {
        let id = self.new_decl(DeclKind::Error, item_name(&item.kind), item.span.0, item.public);
        let kind = self.within(id, |lw| lw.item_kind(&item.kind));
        let meta = self.within(id, |lw| lw.meta(&item.meta));
        let decl = self.design.decl_mut(id);
        decl.kind = kind;
        decl.meta = meta;
        if let ast::ItemKind::Mod(m) = &item.kind {
            if m.items.is_none() {
                self.load_mod(id, item.span.0, &m.name.name);
            }
        }
        id
    }

    fn item_kind(&mut self, kind: &ast::ItemKind) -> DeclKind {
        match kind {
            ast::ItemKind::Fn(f) => self.fn_decl(f),
            ast::ItemKind::Struct(s) => self.struct_decl(s),
            ast::ItemKind::Enum(e) => self.enum_decl(e),
            ast::ItemKind::Impl(i) => self.impl_decl(i),
            ast::ItemKind::Trait(t) => self.trait_decl(t),
            ast::ItemKind::Const(c) => self.const_item(c),
            ast::ItemKind::Use(u) => self.use_decl(u),
            ast::ItemKind::Mod(m) => {
                let origin = match &m.items {
                    Some(items) => {
                        self.inline.push(m.name.name.clone());
                        for inner in items {
                            self.item(inner);
                        }
                        self.inline.pop();
                        NamespaceOrigin::Inline
                    }
                    None => NamespaceOrigin::External { file: None },
                };
                DeclKind::Namespace(NamespaceData { origin })
            }
            ast::ItemKind::Nature(n) => self.nature_decl(n),
            ast::ItemKind::Module(m) => self.module_decl(m),
            ast::ItemKind::Behavior(b) => self.behavior_decl(b),
            ast::ItemKind::Error => DeclKind::Error,
        }
    }

    /// `const NAME: ty = value;` (the owner is the constant itself).
    pub(crate) fn const_item(&mut self, c: &ast::ConstItem) -> DeclKind {
        let ty = self.ty(&c.ty);
        let value = c.value.as_ref().map(|v| self.expr(v));
        DeclKind::Const(ConstData { ty, value })
    }
}

fn item_name(kind: &ast::ItemKind) -> Option<&ast::Ident> {
    match kind {
        ast::ItemKind::Fn(f) => Some(&f.name),
        ast::ItemKind::Struct(s) => Some(&s.name),
        ast::ItemKind::Enum(e) => Some(&e.name),
        ast::ItemKind::Trait(t) => Some(&t.name),
        ast::ItemKind::Const(c) => Some(&c.name),
        ast::ItemKind::Mod(m) => Some(&m.name),
        ast::ItemKind::Nature(n) => Some(&n.name),
        ast::ItemKind::Module(m) => Some(&m.name),
        ast::ItemKind::Impl(_) | ast::ItemKind::Use(_) | ast::ItemKind::Behavior(_) | ast::ItemKind::Error => None,
    }
}
