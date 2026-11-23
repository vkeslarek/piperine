//! Scope: the checker's generic items (generics GN-01, GN-06, GN-07, part I §3.8): the type
//! parameter as a type, a generic item's parameter list, substitution, and the check of a generic
//! body once at its definition against its bounds (Q1 (c)).

#[allow(dead_code)]
#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design};
use piperine_lang::types::{ConstTerm, GenericValue, Length, Ty};

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
    assert_eq!(Ty::array(u.clone(), 2).name(&design), "[U; 2]");
    assert!(t < u, "parameters order by declaration");
    assert!(t.is_value(), "a parameter is a value type in a generic body");
    let key = vec![GenericValue::Type(t), GenericValue::Const(ConstTerm::Value(4))];
    assert!(key < vec![GenericValue::Type(u), GenericValue::Const(ConstTerm::Value(4))]);
}

#[test]
fn an_array_length_may_name_a_const_parameter() {
    // D14: a generic length is legal inside a generic item, prints as the parameter, and is no number
    let design = support::build("fn first<T, const N: usize>(a: [T; N]) -> T { a[0] }\n");
    let declarations = Declarations { design: &design };
    let (t, n) = (declarations.parameter("T"), declarations.parameter("N"));
    let generic = Ty::Array(Box::new(Ty::Param(t)), Length::Generic(ConstTerm::Param(n)));
    assert_eq!(generic.name(&design), "[T; N]");
    assert_eq!(Length::Generic(ConstTerm::Param(n)).fixed(), None);
    assert_eq!(Ty::array(Ty::F64, 3), Ty::Array(Box::new(Ty::F64), Length::Fixed(3)));
    assert_eq!(Length::Fixed(3).fixed(), Some(3));
}
