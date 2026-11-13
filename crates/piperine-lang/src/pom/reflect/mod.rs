//! The reflection face: `Copy` handles that borrow a [`Design`], and the
//! [`Reflect`] trait every declaration handle shares.

mod body;
mod items;

use super::decl::{DeclData, DeclKind};
use super::design::Design;
use super::ids::DeclId;
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
