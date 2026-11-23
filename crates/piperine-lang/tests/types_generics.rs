//! Scope: the checker's generic items (generics GN-01, GN-06, GN-07, part I §3.8): the type
//! parameter as a type, a generic item's parameter list, substitution, and the check of a generic
//! body once at its definition against its bounds (Q1 (c)).

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic};
use piperine_lang::types::{Bound, ConstTerm, ConstType, GenericSignature, GenericValue, Length, ParameterDefault, ParameterKind, Ty, Types};

/// The natures every source below places in the open.
const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

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

impl Declarations<'_> {
    /// The item (a function, a struct or an enum) named `name`.
    fn item(&self, name: &str) -> DeclId {
        (0..self.design.decl_count())
            .map(|i| DeclId(i as u32))
            .find(|decl| matches!(self.design.decl_data(*decl).kind, DeclKind::Fn(_) | DeclKind::Struct(_) | DeclKind::Enum(_)) && self.design.decl_data(*decl).name.as_ref().is_some_and(|n| n.text == name))
            .unwrap_or_else(|| panic!("no item `{name}`"))
    }
}

/// The diagnostics of the model check of `src` after `HEAD`.
fn model_errors(src: &str) -> Vec<DesignDiagnostic> {
    let design = support::build(&format!("{HEAD}{src}"));
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}", support::codes(&design));
    Types::check_model(&design).expect_err("expected diagnostics")
}

/// The codes of `found`, in order.
fn codes_of(found: &[DesignDiagnostic]) -> Vec<String> {
    found.iter().map(|d| d.diagnostic.code.to_string()).collect()
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

#[test]
fn generic_parameter_lists_are_read_with_their_bounds_and_defaults() {
    // §3.8.1 to §3.8.4, §3.8.6: type, nature and const parameters, the bounds of each in order
    // (a library trait, `Repr`, a project trait, `Fn(A) -> R`), and the defaults
    let design = support::build(
        "trait Shape { fn area(self) -> f64; }\n\
         fn f<T: PartialEq + Repr, G: Shape, F: Fn(T, T) -> bool, N: Nature, const K: u32, U = bool, const B: bool = true>(a: T) -> T { a }\n",
    );
    let declarations = Declarations { design: &design };
    let signature = GenericSignature::of(&design, declarations.item("f"));
    let list: Vec<(&str, &ParameterKind)> = signature.parameters().iter().map(|p| (p.name.as_str(), &p.kind)).collect();
    assert_eq!(
        list,
        [
            ("T", &ParameterKind::Type),
            ("G", &ParameterKind::Type),
            ("F", &ParameterKind::Type),
            ("N", &ParameterKind::Nature),
            ("K", &ParameterKind::Const(ConstType::Integer("u32".into()))),
            ("U", &ParameterKind::Type),
            ("B", &ParameterKind::Const(ConstType::Bool)),
        ]
    );
    let parameters = signature.parameters();
    assert_eq!(parameters[0].bounds, [Bound::Library("PartialEq"), Bound::Repr]);
    assert!(matches!(parameters[1].bounds.as_slice(), [Bound::Trait(_)]), "{:?}", parameters[1].bounds);
    assert!(matches!(parameters[2].bounds.as_slice(), [Bound::Callable { inputs, output: Some(_) }] if inputs.len() == 2), "{:?}", parameters[2].bounds);
    assert_eq!(parameters[3].bounds, [Bound::Nature]);
    assert!(matches!(parameters[5].default, Some(ParameterDefault::Type(_))) && matches!(parameters[6].default, Some(ParameterDefault::Const(_))));
    assert_eq!(signature.required(), 5, "the parameters before the first default");
}

#[test]
fn a_wrong_number_of_generic_arguments_is_e0187() {
    // §3.8.7: a use writes every parameter without a default and no more than the list has
    let found = model_errors("struct Pair<T, U = T> { a: T, b: U }\nfn too_many(p: Pair<f64, f64, f64>) -> f64 { 1.0 }\nfn too_few(p: Pair) -> f64 { 1.0 }\n");
    assert_eq!(codes_of(&found), ["E0187", "E0187"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("`Pair` takes 1 to 2 generic arguments, found 3"), "{:?}", found[0]);
    assert!(found[1].diagnostic.message.contains("found 0"), "{:?}", found[1]);
    let right = model_errors("struct Pair<T, U = T> { a: T, b: U }\nfn ok(p: Pair<f64>) -> f64 { 1.0 }\nfn also(p: Pair<f64, u8>) -> f64 { 1.0 }\n");
    assert!(!codes_of(&right).contains(&"E0187".to_string()), "a default may be left out: {right:?}");
}

#[test]
fn a_parameter_list_that_breaks_the_rules_of_3_8_is_refused() {
    // §3.8.4: a default is never followed by a parameter without one; §3.8.3: a const parameter is an
    // integer or `bool`; §3.8.6: a bound names a trait
    let list = model_errors("struct Late<T = bool, U> { a: U }\nfn use_late(p: Late<f64>) -> f64 { 1.0 }\n");
    assert_eq!(codes_of(&list)[0], "E0187", "{list:?}");
    assert!(list[0].diagnostic.message.contains("`U` has no default but follows `T`"), "{:?}", list[0]);
    let constant = model_errors("struct Scaled<const K: f64> { a: f64 }\nfn use_scaled(p: Scaled<1>) -> f64 { 1.0 }\n");
    assert_eq!(codes_of(&constant)[0], "E0187", "{constant:?}");
    assert!(constant[0].diagnostic.message.contains("neither an integer nor `bool`"), "{:?}", constant[0]);
    let bound = model_errors("struct Plain { a: f64 }\nstruct Bounded<T: Plain> { a: T }\nfn use_bounded(p: Bounded<f64>) -> f64 { 1.0 }\n");
    assert_eq!(codes_of(&bound)[0], "E0161", "{bound:?}");
}
