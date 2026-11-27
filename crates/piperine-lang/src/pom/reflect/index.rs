//! The handle forms of the reverse tables of [`crate::pom::Index`].

use super::{Commit, Definition, Expr, Pat, Path};
use crate::pom::reflect::{Instance, Module, Port, Signal};
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

impl<'d> Signal<'d> {
    /// Every definition and commit whose place is rooted at this signal
    /// (`s`, `s[i]`, `s.f`, `(s)`), in source order.
    pub fn writers(self) -> Vec<Writer<'d>> {
        self.decl().writers_of()
    }
}

impl<'d> Port<'d> {
    /// Every definition and commit whose place is rooted at this port, in
    /// source order.
    pub fn writers(self) -> Vec<Writer<'d>> {
        self.decl().writers_of()
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
