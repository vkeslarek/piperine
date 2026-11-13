//! Stage 1: the syntax tree to the arenas. A pure function of the tree: it
//! reads no scope and resolves nothing. Only this module pushes to the arenas.

use super::decl::{
    BehaviorData, ConstData, DeclData, DeclKind, EnumData, FnData, ImplData, ModuleData,
    NamespaceData, NamespaceOrigin, NatureData, StructData, TraitData, UseData,
};
use super::design::Design;
use super::ids::{DeclId, FileId};
use crate::msl::Span;
use crate::msl::ast;

/// The state of one lowering walk: the design under construction, the file
/// being read and the nearest enclosing declaration.
pub(crate) struct Lowerer<'a> {
    pub(crate) design: &'a mut Design,
    pub(crate) file: FileId,
    pub(crate) owner: DeclId,
}

/// Lower every item of `ast` as a child of the namespace `ns`.
pub(crate) fn lower_file(design: &mut Design, file: FileId, ns: DeclId, ast: &ast::File) {
    let mut lw = Lowerer { design, file, owner: ns };
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
            public,
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

    /// One declaration per item, recursing into inline modules.
    pub(crate) fn item(&mut self, item: &ast::Item) -> DeclId {
        let (kind, name) = match &item.kind {
            ast::ItemKind::Fn(f) => (DeclKind::Fn(FnData::default()), Some(&f.name)),
            ast::ItemKind::Struct(s) => (DeclKind::Struct(StructData::default()), Some(&s.name)),
            ast::ItemKind::Enum(e) => (DeclKind::Enum(EnumData::default()), Some(&e.name)),
            ast::ItemKind::Impl(_) => (DeclKind::Impl(ImplData::default()), None),
            ast::ItemKind::Trait(t) => (DeclKind::Trait(TraitData::default()), Some(&t.name)),
            ast::ItemKind::Const(c) => (DeclKind::Const(ConstData::default()), Some(&c.name)),
            ast::ItemKind::Use(_) => (DeclKind::Use(UseData::default()), None),
            ast::ItemKind::Mod(m) => {
                let origin = if m.items.is_some() {
                    NamespaceOrigin::Inline
                } else {
                    NamespaceOrigin::External { file: None }
                };
                (DeclKind::Namespace(NamespaceData { origin }), Some(&m.name))
            }
            ast::ItemKind::Nature(n) => (DeclKind::Nature(NatureData::default()), Some(&n.name)),
            ast::ItemKind::Module(m) => (DeclKind::Module(ModuleData::default()), Some(&m.name)),
            ast::ItemKind::Behavior(_) => (DeclKind::Behavior(BehaviorData::default()), None),
            ast::ItemKind::Error => (DeclKind::Error, None),
        };
        let id = self.new_decl(kind, name, item.span.0, item.public);
        if let ast::ItemKind::Mod(ast::ModItem { items: Some(items), .. }) = &item.kind {
            self.within(id, |lw| {
                for inner in items {
                    lw.item(inner);
                }
            });
        }
        id
    }
}
