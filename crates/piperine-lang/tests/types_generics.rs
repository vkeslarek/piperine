//! Scope: the checker's generic items (generics GN-01, GN-06, GN-07, part I §3.8): the type
//! parameter as a type, a generic item's parameter list, substitution, and the check of a generic
//! body once at its definition against its bounds (Q1 (c)).

#[allow(dead_code)]
#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design};
use piperine_lang::types::{ConstTerm, GenericValue, Ty};

/// The declarations of a source, looked up by name.
struct Declarations<'d> {
    design: &'d Design,
}

impl<'d> Declarations<'d> {
    /// The generic parameter named `name` of the design.
    fn parameter(&self, name: &str) -> DeclId {
        (0..self.design.decl_count())
            .map(|i| DeclId(i as u32))
            .find(|decl| matches!(self.design.decl_data(*decl).kind, DeclKind::GenericParam(_)) && self.design.decl_data(*decl).name.as_ref().is_some_and(|n| n.text == name))
            .unwrap_or_else(|| panic!("no generic parameter `{name}`"))
    }
}

#[test]
fn a_type_parameter_prints_its_name() {
    // §3.8: a type parameter is a type of the generic body, named as its author wrote it; `Ty` is
    // ordered so an instantiation can key a map
    let design = support::build("fn first<T, U>(a: T, b: U) -> T { a }\n");
    let declarations = Declarations { design: &design };
    let (t, u) = (Ty::Param(declarations.parameter("T")), Ty::Param(declarations.parameter("U")));
    assert_eq!(t.name(&design), "T");
    assert_eq!(Ty::Array(Box::new(u.clone()), 2).name(&design), "[U; 2]");
    assert!(t < u, "parameters order by declaration");
    assert!(t.is_value(), "a parameter is a value type in a generic body");
    let key = vec![GenericValue::Type(t), GenericValue::Const(ConstTerm::Value(4))];
    assert!(key < vec![GenericValue::Type(u), GenericValue::Const(ConstTerm::Value(4))]);
}
