//! The handle over a crate of the design: its name, root namespace and files.

use super::super::design::Design;
use super::super::ids::{CrateId, FileId};
use super::{Namespace, SourceFile};

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

    /// The crate the design was built from.
    pub fn root_crate(&self) -> Crate<'_> {
        Crate::new(self, self.root_crate)
    }

    /// The standard library.
    pub fn std_crate(&self) -> Crate<'_> {
        Crate::new(self, self.std_crate)
    }
}
