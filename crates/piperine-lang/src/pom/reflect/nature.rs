//! Handles over natures and their quantities.

use super::{Decl, Expr, Reflect, Type};
use crate::pom::body::QuantityRole;
use crate::pom::decl::{
    DeclKind, NatureData, NatureFieldNode, QuantityData,
};
use crate::pom::resolution::NatureKind;



/// `nature Name { potential: .., flow: .., value: .. }`
#[derive(Clone, Copy)]
pub struct Nature<'d> {
    decl: Decl<'d>,
}


impl<'d> Nature<'d> {
    fn data(self) -> &'d NatureData {
        match self.decl.kind() {
            DeclKind::Nature(n) => n,
            _ => unreachable!("Nature handle over a non-nature declaration"),
        }
    }

    /// `Conservative`, `SignalFlow`, `Discrete`, `Resolved` or `Invalid`, by
    /// the fields declared.
    pub fn kind(self) -> NatureKind {
        self.data().kind
    }

    pub fn generics(self) -> Vec<super::GenericParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generic_param()).collect()
    }

    /// The quantities in source order, repeated ones included.
    pub fn quantities(self) -> Vec<Quantity<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_quantity()).collect()
    }

    /// The first `potential` quantity.
    pub fn potential(self) -> Option<Quantity<'d>> {
        self.quantities().into_iter().find(|q| q.role() == QuantityRole::Potential)
    }

    /// The first `flow` quantity.
    pub fn flow(self) -> Option<Quantity<'d>> {
        self.quantities().into_iter().find(|q| q.role() == QuantityRole::Flow)
    }

    /// The `impl` blocks written for this nature, in source order.
    pub fn impls(self) -> Vec<super::Impl<'d>> {
        let design = self.decl.design();
        design.impls_of(self.decl.id()).into_iter().filter_map(|i| Decl::new(design, i).as_impl()).collect()
    }

    /// The type of the first `value: T` field.
    pub fn value_type(self) -> Option<Type<'d>> {
        self.data().fields.iter().find_map(|f| match f {
            NatureFieldNode::Value { ty, .. } => Some(self.decl.design().ty(*ty)),
            _ => None,
        })
    }
}


/// `potential: Voltage { access: v, units: "V", abstol: 1u }`
#[derive(Clone, Copy)]
pub struct Quantity<'d> {
    decl: Decl<'d>,
}


impl<'d> Quantity<'d> {
    fn data(self) -> &'d QuantityData {
        match self.decl.kind() {
            DeclKind::Quantity(q) => q,
            _ => unreachable!("Quantity handle over a non-quantity declaration"),
        }
    }

    pub fn role(self) -> QuantityRole {
        self.data().role
    }

    /// The value of the attribute `name`, as written.
    fn attr_value(self, name: &str) -> Option<Expr<'d>> {
        let init = self.data().attrs.iter().find(|a| a.name.text == name)?;
        Some(self.decl.design().expr(init.value))
    }

    /// The `access: v` attribute: the name of the access function.
    pub fn access(self) -> Option<Expr<'d>> {
        self.attr_value("access")
    }

    pub fn units(self) -> Option<Expr<'d>> {
        self.attr_value("units")
    }

    pub fn abstol(self) -> Option<Expr<'d>> {
        self.attr_value("abstol")
    }
}


impl<'d> Reflect<'d> for Nature<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}


impl PartialEq for Nature<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}


impl std::fmt::Debug for Nature<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Nature({:?})", self.decl.id())
    }
}


impl<'d> Reflect<'d> for Quantity<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}


impl PartialEq for Quantity<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}


impl std::fmt::Debug for Quantity<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quantity({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_nature(self) -> Option<Nature<'d>> {
        matches!(self.kind(), DeclKind::Nature(_)).then_some(Nature { decl: self })
    }

    pub fn as_quantity(self) -> Option<Quantity<'d>> {
        matches!(self.kind(), DeclKind::Quantity(_)).then_some(Quantity { decl: self })
    }
}
