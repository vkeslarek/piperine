//! The reflection face: `Copy` handles that borrow a [`Design`], and the
//! [`Reflect`] trait every declaration handle shares.

mod body;
mod hardware;
mod index;
mod items;

use super::body::Lit;
use super::decl::{DeclData, DeclKind};
use super::design::Design;
use super::ids::DeclId;
use super::index::ReferenceId;
use super::meta::{AttrArgNode, AttrInputNode, AttrNode, Doc};
use crate::msl::Span;

pub use body::{Commit, Contribution, Expr, Path, Pat, SourceFile, Stmt, Type};
pub use hardware::{Behavior, Binding, Definition, GenArm, Generation, Instance, Let, Module, Nature, Param, Port, Quantity, Reaction, Signal};
pub use index::{Reference, Writer};
pub use items::{Const, Enum, Field, FnParam, Function, GenericParam, Impl, ImportRef, Namespace, Struct, Trait, Use, Variant};

/// The generic declaration handle: any kind, read through [`Decl::kind`].
#[derive(Clone, Copy)]
pub struct Decl<'d> {
    design: &'d Design,
    id: DeclId,
}

impl<'d> Decl<'d> {
    pub(crate) fn new(design: &'d Design, id: DeclId) -> Self {
        Decl { design, id }
    }

    pub fn id(self) -> DeclId {
        self.id
    }

    pub fn design(self) -> &'d Design {
        self.design
    }

    /// The raw storage of this declaration.
    pub fn data(self) -> &'d DeclData {
        self.design.decl_data(self.id)
    }

    pub fn kind(self) -> &'d DeclKind {
        &self.data().kind
    }

    /// The declarations lexically inside this one, at the first level, in
    /// source order.
    pub fn children(self) -> Vec<Decl<'d>> {
        self.data().children.iter().map(|c| Decl::new(self.design, *c)).collect()
    }
}

impl PartialEq for Decl<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Decl<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Decl({:?}, {})", self.id, self.kind().label())
    }
}

/// What every declaration handle answers. Implementors give [`Reflect::decl`];
/// the rest are plain provided methods.
pub trait Reflect<'d>: Copy {
    fn decl(self) -> Decl<'d>;

    /// The declared name; none for an `impl`, a `use`, a behavior block, an
    /// error item or a file's root namespace.
    fn name(self) -> Option<&'d str> {
        self.decl().data().name.as_ref().map(|n| n.text.as_str())
    }

    /// The range of the whole declaration (an item's span includes its
    /// attributes and docs).
    fn span(self) -> Span {
        self.decl().data().span
    }

    fn file(self) -> SourceFile<'d> {
        let d = self.decl();
        SourceFile::new(d.design, d.data().file)
    }

    /// The authored text of the declaration.
    fn text(self) -> &'d str {
        self.file().text().get(self.span().range()).unwrap_or("")
    }

    /// The doc comments in order, one per comment. On a file's root
    /// namespace these are the file's `//!` comments.
    fn docs(self) -> &'d [Doc] {
        &self.decl().data().meta.docs
    }

    /// The attributes in order.
    fn attrs(self) -> Vec<Attr<'d>> {
        let d = self.decl();
        d.data().meta.attrs.iter().map(|node| Attr { owner: d, node }).collect()
    }

    /// The first attribute whose path is written `name`.
    fn attr(self, name: &str) -> Option<Attr<'d>> {
        self.attrs().into_iter().find(|a| a.name() == name)
    }

    fn is_pub(self) -> bool {
        self.decl().data().public
    }

    /// The module this declaration belongs to: itself for a module, else the
    /// nearest enclosing module (through generation). None outside any
    /// module.
    fn module(self) -> Option<Module<'d>> {
        let mut at = Some(self.decl());
        while let Some(d) = at {
            if let Some(m) = d.as_module() {
                return Some(m);
            }
            // A behavior block is a file item: its module is the one it names.
            if let Some(b) = d.as_behavior() {
                return b.module();
            }
            at = d.declared_in();
        }
        None
    }

    /// The declaration that lexically contains this one; none for a file's
    /// root namespace.
    fn declared_in(self) -> Option<Decl<'d>> {
        let d = self.decl();
        d.data().parent.map(|p| Decl::new(d.design, p))
    }

    /// Every path with a segment, and every member expression, that resolved
    /// to this declaration, in source order.
    fn references(self) -> Vec<Reference<'d>> {
        let d = self.decl();
        d.design()
            .index
            .references(d.id())
            .iter()
            .map(|r| match *r {
                ReferenceId::Path(p) => Reference::Path(d.design().path(p)),
                ReferenceId::Member(e) => Reference::Member(d.design().expr(e)),
            })
            .collect()
    }

    /// The namespace path joined by `::`, then the member chain joined by
    /// `.`: signal `w` of module `M` in `mod a` is `a::M.w`. A generation, an
    /// unnamed declaration (an `impl`, a `use`, a behavior block) and a
    /// file's root namespace add no segment.
    fn qualified_name(self) -> String {
        let mut chain = Vec::new();
        let mut at = Some(self.decl());
        while let Some(d) = at {
            chain.push(d);
            at = d.declared_in();
        }
        chain.reverse();
        let mut out = String::new();
        let mut in_members = false;
        for d in chain {
            let Some(name) = d.name() else { continue };
            if matches!(d.kind(), DeclKind::Generation(_)) {
                continue;
            }
            if !out.is_empty() {
                out.push_str(if in_members { "." } else { "::" });
            }
            out.push_str(name);
            if !matches!(d.kind(), DeclKind::Namespace(_)) {
                in_members = true;
            }
        }
        out
    }
}

/// An attribute: `#[path]`, `#[path(args)]` or `#[path = expr]`.
#[derive(Clone, Copy)]
pub struct Attr<'d> {
    owner: Decl<'d>,
    node: &'d AttrNode,
}

impl<'d> Attr<'d> {
    pub fn node(self) -> &'d AttrNode {
        self.node
    }

    /// The path as written, segments joined by `::`.
    pub fn name(self) -> String {
        let path = self.path();
        path.segments().iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::")
    }

    /// The attribute's path node; its resolution stays `Unresolved`.
    pub fn path(self) -> Path<'d> {
        self.owner.design().path(self.node.path)
    }

    pub fn span(self) -> Span {
        self.node.span
    }

    /// The authored text, `#[...]` included.
    pub fn text(self) -> &'d str {
        self.owner.file().text().get(self.node.span.range()).unwrap_or("")
    }

    pub fn input(self) -> &'d AttrInputNode {
        &self.node.input
    }

    /// The `= expr` value of `#[path = expr]`.
    pub fn value(self) -> Option<Expr<'d>> {
        match &self.node.input {
            AttrInputNode::Eq(e) => Some(self.owner.design().expr(*e)),
            AttrInputNode::None | AttrInputNode::List(_) => None,
        }
    }

    /// The literal arguments written without a name, in order.
    pub fn positional(self) -> Vec<&'d Lit> {
        match &self.node.input {
            AttrInputNode::List(args) => args
                .iter()
                .filter_map(|a| match a {
                    AttrArgNode::Lit(l) => Some(l),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The path arguments, as in `#[derive(Repr, Clone)]`, in order.
    pub fn paths(self) -> Vec<Path<'d>> {
        match &self.node.input {
            AttrInputNode::List(args) => args
                .iter()
                .filter_map(|a| match a {
                    AttrArgNode::Path(p) => Some(self.owner.design().path(*p)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The value of the argument `name = lit`.
    pub fn named(self, name: &str) -> Option<&'d Lit> {
        match &self.node.input {
            AttrInputNode::List(args) => args.iter().find_map(|a| match a {
                AttrArgNode::NameValue(n, l) if n.text == name => Some(l),
                _ => None,
            }),
            _ => None,
        }
    }
}

impl std::fmt::Debug for Attr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attr({})", self.name())
    }
}

impl<'d> Reflect<'d> for Decl<'d> {
    fn decl(self) -> Decl<'d> {
        self
    }
}

impl Design {
    /// The root file's namespace.
    pub fn root(&self) -> Decl<'_> {
        Decl::new(self, self.root_id())
    }

    /// The declaration with this id, as a handle: the way from a
    /// `Resolution::Decl(id)` to the declaration. Ids come from this design.
    pub fn decl(&self, id: DeclId) -> Decl<'_> {
        Decl::new(self, id)
    }

    /// Every declaration of the design in arena (source) order, prelude
    /// files first.
    pub fn decls(&self) -> Vec<Decl<'_>> {
        (0..self.decl_count()).map(|i| Decl::new(self, DeclId(i as u32))).collect()
    }

    /// The declarations of the files that are not prelude sources.
    fn authored(&self) -> impl Iterator<Item = Decl<'_>> {
        self.decls().into_iter().filter(|d| !d.file().is_prelude())
    }

    /// Every module of the design, prelude files first, in source order.
    pub fn modules(&self) -> Vec<Module<'_>> {
        self.decls().into_iter().filter_map(Decl::as_module).collect()
    }

    /// The first module named `name`: the first match in a non-prelude file
    /// in file order, else the first match in a prelude source.
    pub fn module(&self, name: &str) -> Option<Module<'_>> {
        let named = |m: &Module<'_>| m.name() == Some(name);
        self.authored().filter_map(Decl::as_module).find(named).or_else(|| self.modules().into_iter().find(named))
    }

    /// Every free function (a `fn` directly in a namespace) of the non-prelude
    /// files, in source order.
    pub fn functions(&self) -> Vec<Function<'_>> {
        self.authored()
            .filter(|d| d.declared_in().is_some_and(|p| p.as_namespace().is_some()))
            .filter_map(Decl::as_function)
            .collect()
    }

    /// Every struct of the non-prelude files, in source order.
    pub fn structs(&self) -> Vec<Struct<'_>> {
        self.authored().filter_map(Decl::as_struct).collect()
    }

    /// Every named namespace (`mod a { .. }` or `mod a;`) of the non-prelude
    /// files; a file's unnamed root is not listed.
    pub fn namespaces(&self) -> Vec<Namespace<'_>> {
        self.authored().filter_map(Decl::as_namespace).filter(|n| n.name().is_some()).collect()
    }

    /// Every nature of the design, in source order.
    pub fn natures(&self) -> Vec<Nature<'_>> {
        (0..self.decl_count()).filter_map(|i| Decl::new(self, DeclId(i as u32)).as_nature()).collect()
    }

    /// The first nature named `name`.
    pub fn nature(&self, name: &str) -> Option<Nature<'_>> {
        self.natures().into_iter().find(|n| n.name() == Some(name))
    }

    /// The declaration at a `::` path from the root, each segment a child
    /// name: `a::M` is `M` inside namespace `a`.
    pub fn find(&self, path: &str) -> Option<Decl<'_>> {
        let mut at = self.root();
        for segment in path.split("::") {
            at = at.children().into_iter().find(|c| c.name() == Some(segment))?;
        }
        Some(at)
    }
}
