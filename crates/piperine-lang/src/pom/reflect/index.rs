//! The handle forms of the reverse tables of [`crate::pom::Index`].

use super::{Commit, Decl, Definition, Expr, Pat, Path};
use crate::pom::design::Design;
use crate::pom::index::WriterId;
use crate::pom::reflect::hardware::{Module, Instance, Port, Signal};
use super::Reflect;

/// A node that names a declaration: see [`Reflect::references`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reference<'d> {
    /// A path (a segment of it resolved to the declaration).
    Path(Path<'d>),
    /// A `.name` or `.method()` expression.
    Member(Expr<'d>),
    /// An identifier pattern that names it (a constant, a unit variant, a unit struct).
    Pattern(Pat<'d>),
}

/// Something that writes a place: see [`Signal::writers`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Writer<'d> {
    /// `place = value;` in a module body.
    Definition(Definition<'d>),
    /// `place <- value;` in a reaction.
    Commit(Commit<'d>),
}

fn writers_of<'d>(design: &'d Design, decl: Decl<'d>) -> Vec<Writer<'d>> {
    design
        .index
        .writers(decl.id())
        .iter()
        .filter_map(|w| match *w {
            WriterId::Definition(d) => Decl::new(design, d).as_definition().map(Writer::Definition),
            WriterId::Commit(s) => Some(Writer::Commit(Commit::new(design, s))),
        })
        .collect()
}

impl<'d> Signal<'d> {
    /// Every definition and commit whose place is rooted at this signal
    /// (`s`, `s[i]`, `s.f`, `(s)`), in source order.
    pub fn writers(self) -> Vec<Writer<'d>> {
        writers_of(self.decl().design(), self.decl())
    }
}

impl<'d> Port<'d> {
    /// Every definition and commit whose place is rooted at this port, in
    /// source order.
    pub fn writers(self) -> Vec<Writer<'d>> {
        writers_of(self.decl().design(), self.decl())
    }
}

impl<'d> Module<'d> {
    /// Every instance, in the whole design, whose `module()` is this module,
    /// in source order.
    pub fn instantiated_by(self) -> Vec<Instance<'d>> {
        let design = self.decl().design();
        design.index.instantiated_by(self.decl().id()).iter().map(|e| Instance::new(design, *e)).collect()
    }
}
