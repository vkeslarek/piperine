//! Handles over value items: `Const` here; functions, structs, enums, traits,
//! impls and imports as the item tasks add them.

use super::body::{Expr, Type};
use super::{Decl, Reflect};
use crate::pom::decl::{ConstData, DeclKind};

/// `const NAME: ty = value;`
#[derive(Clone, Copy)]
pub struct Const<'d> {
    decl: Decl<'d>,
}

impl<'d> Const<'d> {
    fn data(self) -> &'d ConstData {
        match self.decl.kind() {
            DeclKind::Const(c) => c,
            // `Decl::as_const` only builds this handle over a constant.
            _ => unreachable!("Const handle over a non-constant declaration"),
        }
    }

    pub fn ty(self) -> Type<'d> {
        self.decl.design().ty(self.data().ty)
    }

    /// The value expression; none for a trait's constant declaration.
    pub fn value(self) -> Option<Expr<'d>> {
        self.data().value.map(|v| self.decl.design().expr(v))
    }
}

impl<'d> Reflect<'d> for Const<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Const<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Const<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Const({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_const(self) -> Option<Const<'d>> {
        matches!(self.kind(), DeclKind::Const(_)).then_some(Const { decl: self })
    }
}
