//! The lookups of the design's reflection face: its declarations and the handles by name.

use super::{Const, Decl, Enum, Function, Impl, Module, Namespace, Nature, Reflect, Struct, Trait};
use crate::pom::design::Design;
use crate::pom::ids::DeclId;

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

    /// Every declaration of the design in arena (source) order: the project's files first, `std`
    /// last.
    pub fn decls(&self) -> Vec<Decl<'_>> {
        (0..self.decl_count()).map(|i| DeclId(i as u32)).filter(|id| self.leaf_owner(*id).is_none()).map(|id| Decl::new(self, id)).collect()
    }

    /// The declarations of the root crate.
    fn authored(&self) -> impl Iterator<Item = Decl<'_>> {
        self.decls().into_iter().filter(|d| d.krate().is_root())
    }

    /// Every module of the root crate, in source order.
    pub fn modules(&self) -> Vec<Module<'_>> {
        self.authored().filter_map(Decl::as_module).collect()
    }

    /// The first declaration `accept` takes: one of the root crate, else the first in crate
    /// order. The one precedence of every by-name lookup.
    fn first_project_first(&self, accept: impl Fn(Decl<'_>) -> bool) -> Option<Decl<'_>> {
        self.authored().find(|d| accept(*d)).or_else(|| self.decls().into_iter().find(|d| accept(*d)))
    }

    /// [`Design::first_project_first`] over the declarations named `name`,
    /// read from the sorted name table instead of a scan.
    fn first_named(&self, name: &str, accept: impl Fn(Decl<'_>) -> bool) -> Option<Decl<'_>> {
        let ids = self.tables().by_name.get(name)?;
        let mut candidates = ids.iter().map(|id| Decl::new(self, *id)).filter(|d| accept(*d));
        let mut first = None;
        for d in candidates.by_ref() {
            if d.krate().is_root() {
                return Some(d);
            }
            if first.is_none_or(|f: Decl<'_>| d.krate().id() < f.krate().id()) {
                first = Some(d);
            }
        }
        first
    }

    /// The module named `name`: the root crate's, else the first match in crate order.
    pub fn module(&self, name: &str) -> Option<Module<'_>> {
        self.first_named(name, |d| d.as_module().is_some()).and_then(Decl::as_module)
    }

    /// Every free function (a `fn` directly in a namespace) of the root
    /// crate, in source order.
    pub fn functions(&self) -> Vec<Function<'_>> {
        self.authored().filter(Design::is_free).filter_map(Decl::as_function).collect()
    }

    fn is_free(d: &Decl<'_>) -> bool {
        d.declared_in().is_some_and(|p| p.as_namespace().is_some())
    }

    /// The free function named `name`, the root crate first.
    pub fn function(&self, name: &str) -> Option<Function<'_>> {
        self.first_named(name, |d| Design::is_free(&d) && d.as_function().is_some()).and_then(Decl::as_function)
    }

    /// Every struct of the root crate, in source order.
    pub fn structs(&self) -> Vec<Struct<'_>> {
        self.authored().filter_map(Decl::as_struct).collect()
    }

    /// The struct named `name`, the root crate first.
    pub fn struct_named(&self, name: &str) -> Option<Struct<'_>> {
        self.first_named(name, |d| d.as_struct().is_some()).and_then(Decl::as_struct)
    }

    /// Every enum of the root crate, in source order.
    pub fn enums(&self) -> Vec<Enum<'_>> {
        self.authored().filter_map(Decl::as_enum).collect()
    }

    /// Every trait of the root crate, in source order.
    pub fn traits(&self) -> Vec<Trait<'_>> {
        self.authored().filter_map(Decl::as_trait).collect()
    }

    /// Every `const` item of the root crate, in source order.
    pub fn consts(&self) -> Vec<Const<'_>> {
        self.authored().filter_map(Decl::as_const).collect()
    }

    /// Every `impl` block of the root crate, in source order.
    pub fn impls(&self) -> Vec<Impl<'_>> {
        self.authored().filter_map(Decl::as_impl).collect()
    }

    /// Every named namespace (`mod a { .. }` or `mod a;`) of the root
    /// crate; a file's unnamed root is not listed.
    pub fn namespaces(&self) -> Vec<Namespace<'_>> {
        self.authored().filter_map(Decl::as_namespace).filter(|n| n.name().is_some()).collect()
    }

    /// Every nature of the root crate, in source order.
    pub fn natures(&self) -> Vec<Nature<'_>> {
        self.authored().filter_map(Decl::as_nature).collect()
    }

    /// The nature named `name`, the root crate first.
    pub fn nature(&self, name: &str) -> Option<Nature<'_>> {
        self.first_named(name, |d| d.as_nature().is_some()).and_then(Decl::as_nature)
    }

    /// The declaration at a `::` path from the root, each segment a child
    /// name (`a::M` is `M` inside namespace `a`), or at a name
    /// [`Reflect::qualified_name`] prints (`a::E.A`, `impl S.X`).
    pub fn find(&self, path: &str) -> Option<Decl<'_>> {
        let walked = path.split("::").try_fold(self.root(), |at, segment| at.children().into_iter().find(|c| c.name() == Some(segment)));
        // The other form is a name `qualified_name` printed: the first declaration with exactly it.
        walked.or_else(|| self.first_project_first(|d| d.qualified_name() == path))
    }
}
