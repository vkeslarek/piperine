//! The reflection face: `Copy` handles that borrow a [`Design`], and the
//! [`Reflect`] trait every declaration handle shares.

mod body;
mod items;

use super::body::Lit;
use super::decl::{DeclData, DeclKind};
use super::design::Design;
use super::ids::DeclId;
use super::meta::{AttrArgNode, AttrInputNode, AttrNode, Doc};
use crate::msl::Span;

pub use body::{Expr, Path, Pat, SourceFile, Stmt, Type};
pub use items::{Const, FnParam, Function, GenericParam};

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

    /// The declaration that lexically contains this one; none for a file's
    /// root namespace.
    fn declared_in(self) -> Option<Decl<'d>> {
        let d = self.decl();
        d.data().parent.map(|p| Decl::new(d.design, p))
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
