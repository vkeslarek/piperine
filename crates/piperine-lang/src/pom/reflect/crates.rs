//! The handle over a crate of the design: its name, root namespace and files.

use super::super::design::Design;
use super::super::ids::{CrateId, FileId};
use super::{Decl, Module, Namespace, Reflect, SourceFile};

/// A crate of a [`Design`]: the root crate, a dependency, or `std`.
#[derive(Clone, Copy)]
pub struct Crate<'d> {
    design: &'d Design,
    id: CrateId,
}

impl<'d> Crate<'d> {
    pub(crate) fn new(design: &'d Design, id: CrateId) -> Self {
        Crate { design, id }
    }

    pub fn id(self) -> CrateId {
        self.id
    }

    /// The crate's name: its name in the graph, the stem of the root file for a design built from
    /// one root file, `std` for the standard library.
    pub fn name(self) -> &'d str {
        &self.design.crate_data(self.id).name
    }

    /// The root namespace: the namespace of the root file.
    pub fn root(self) -> Namespace<'d> {
        let decl = super::Decl::new(self.design, self.design.crate_data(self.id).root);
        decl.as_namespace().expect("a crate's root is a namespace")
    }

    /// Every file of the crate: its root file, then the files its `mod` items load.
    pub fn files(self) -> Vec<SourceFile<'d>> {
        (0..self.design.file_count())
            .map(|i| SourceFile::new(self.design, FileId(i as u32)))
            .filter(|f| f.krate() == self)
            .collect()
    }

    /// The dependencies the crate declares, in declaration order, each as the name this crate
    /// gives it and the crate it names. `std` is implicit and not listed.
    pub fn dependencies(self) -> Vec<(&'d str, Crate<'d>)> {
        let design = self.design;
        design.crate_data(self.id).dependencies.iter().map(|d| (d.name.as_str(), Crate::new(design, d.krate))).collect()
    }

    /// The crate this one reaches under the dependency name `name`.
    pub fn dependency(self, name: &str) -> Option<Crate<'d>> {
        self.dependencies().into_iter().find(|(n, _)| *n == name).map(|(_, c)| c)
    }

    /// Every declaration of this crate, in source order.
    pub fn decls(self) -> Vec<Decl<'d>> {
        self.design.decls().into_iter().filter(|d| d.krate() == self).collect()
    }

    /// Every module of this crate, in source order.
    pub fn modules(self) -> Vec<Module<'d>> {
        self.decls().into_iter().filter_map(Decl::as_module).collect()
    }

    /// The declaration at a `::` path from this crate's root, or at a name
    /// [`Reflect::qualified_name`] prints for a declaration of this crate.
    pub fn find(self, path: &str) -> Option<Decl<'d>> {
        let walked = path.split("::").try_fold(self.root().decl(), |at, segment| at.children().into_iter().find(|c| c.name() == Some(segment)));
        walked.or_else(|| self.decls().into_iter().find(|d| d.qualified_name() == path))
    }

    /// Is this the crate the design was built from: the first of the graph?
    pub fn is_root(self) -> bool {
        self.id == self.design.root_crate
    }

    /// Is this the standard library?
    pub fn is_std(self) -> bool {
        self.id == self.design.std_crate
    }
}

impl PartialEq for Crate<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Crate<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Crate({:?}, {})", self.id, self.name())
    }
}

impl Design {
    /// Every crate in graph order, the root crate first and `std` last.
    pub fn crates(&self) -> Vec<Crate<'_>> {
        (0..self.crates.len()).map(|i| Crate::new(self, CrateId(i as u32))).collect()
    }

    /// The first crate of this name, in crate order.
    pub fn krate(&self, name: &str) -> Option<Crate<'_>> {
        self.crates().into_iter().find(|c| c.name() == name)
    }

    /// The crate the design was built from.
    pub fn root_crate(&self) -> Crate<'_> {
        Crate::new(self, self.root_crate)
    }

    /// The standard library.
    pub fn std_crate(&self) -> Crate<'_> {
        Crate::new(self, self.std_crate)
    }
}
