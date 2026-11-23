//! Scope: the checker's generic items (generics GN-01, GN-06, GN-07, part I §3.8): the type
//! parameter as a type, a generic item's parameter list, substitution, and the check of a generic
//! body once at its definition against its bounds (Q1 (c)).

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use generic_sources::GenericSources;

use piperine_lang::pom::{DeclId, DeclKind, Design, TypeId};
use piperine_lang::types::{Bound, ConstTerm, ConstType, GenericSignature, GenericValue, Length, Instantiation, Nominal, Obligation, ParameterDefault, ParameterKind, Substitution, Ty, Types};

/// The natures every source below places in the open.
const HEAD: &str = GenericSources::HEAD;

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
    assert!(matches!(parameters[1].bounds.as_slice(), [Bound::Trait { .. }]), "{:?}", parameters[1].bounds);
    assert!(matches!(parameters[2].bounds.as_slice(), [Bound::Callable { inputs, output: Some(_) }] if inputs.len() == 2), "{:?}", parameters[2].bounds);
    assert_eq!(parameters[3].bounds, [Bound::Nature]);
    assert!(matches!(parameters[5].default, Some(ParameterDefault::Type(_))) && matches!(parameters[6].default, Some(ParameterDefault::Const(_))));
    assert_eq!(signature.required(), 5, "the parameters before the first default");
}

#[test]
fn a_wrong_number_of_generic_arguments_is_e0187() {
    // §3.8.7: a use writes every parameter without a default and no more than the list has
    let found = GenericSources::model_errors("struct Pair<T, U = T> { a: T, b: U }\nfn too_many(p: Pair<f64, f64, f64>) -> f64 { 1.0 }\nfn too_few(p: Pair) -> f64 { 1.0 }\n");
    assert_eq!(GenericSources::codes_of(&found), ["E0187", "E0187"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("`Pair` takes 1 to 2 generic arguments, found 3"), "{:?}", found[0]);
    assert!(found[1].diagnostic.message.contains("found 0"), "{:?}", found[1]);
    let right = support::build(&format!("{HEAD}struct Pair<T, U = T> {{ a: T, b: U }}\nfn ok(p: Pair<f64>) -> f64 {{ 1.0 }}\nfn also(p: Pair<f64, u8>) -> f64 {{ 1.0 }}\n"));
    assert_eq!(Types::check_model(&right), Ok(()), "a default may be left out");
}

#[test]
fn a_parameter_list_that_breaks_the_rules_of_3_8_is_refused() {
    // §3.8.4: a default is never followed by a parameter without one; §3.8.3: a const parameter is an
    // integer or `bool`; §3.8.6: a bound names a trait
    let list = GenericSources::model_errors("struct Late<T = bool, U> { a: U }\nfn use_late(p: Late<f64>) -> f64 { 1.0 }\n");
    assert_eq!(GenericSources::codes_of(&list)[0], "E0187", "{list:?}");
    assert!(list[0].diagnostic.message.contains("`U` has no default but follows `T`"), "{:?}", list[0]);
    let constant = GenericSources::model_errors("struct Scaled<const K: f64> { a: f64 }\nfn use_scaled(p: Scaled<1>) -> f64 { 1.0 }\n");
    assert_eq!(GenericSources::codes_of(&constant)[0], "E0187", "{constant:?}");
    assert!(constant[0].diagnostic.message.contains("neither an integer nor `bool`"), "{:?}", constant[0]);
    let bound = GenericSources::model_errors("struct Plain { a: f64 }\nstruct Bounded<T: Plain> { a: T }\nfn use_bounded(p: Bounded<f64>) -> f64 { 1.0 }\n");
    assert_eq!(GenericSources::codes_of(&bound)[0], "E0161", "{bound:?}");
}

#[test]
fn written_generic_types_lower_with_their_arguments() {
    // §3.8.5 to §3.8.7: a type argument, a const argument, a default filled from an earlier parameter
    // (`U = T`) and a use nested in another are the types the message names; a wrong kind of argument
    // is `E0187`
    let items = "struct Pair<T, U = T> { a: T, b: U }\nenum Maybe<T> { No, Yes(T) }\nstruct Delay<T, const N: usize> { v: T }\n";
    let named = |signature: &str| {
        let found = GenericSources::model_errors(&format!("{items}fn f({signature}) -> f64 {{ p }}\n"));
        assert_eq!(GenericSources::codes_of(&found), ["E0180"], "{signature}: {found:?}");
        found[0].diagnostic.message.clone()
    };
    assert!(named("p: Pair<f64>").contains("Pair<f64, f64>"), "the default U = T is f64");
    assert!(named("p: Pair<u8, bool>").contains("Pair<u8, bool>"));
    assert!(named("p: Maybe<Pair<f64>>").contains("Maybe<Pair<f64, f64>>"), "a use nested in another");
    assert!(named("p: Delay<u8, 4>").contains("Delay<u8, 4>"), "a const argument");
    assert!(named("p: Delay<bool, { 2 + 2 }>").contains("Delay<bool, 4>"), "a constant expression is folded");
    let kinds = GenericSources::model_errors(&format!("{items}fn g(a: Delay<u8, bool>, b: Pair<4, f64>) -> f64 {{ 1.0 }}\n"));
    assert!(GenericSources::codes_of(&kinds).iter().all(|code| code == "E0187"), "{kinds:?}");
    assert!(kinds[0].diagnostic.message.contains("takes a constant here"), "{:?}", kinds[0]);
}

#[test]
fn a_substitution_replaces_parameters_through_every_part_of_a_type() {
    // §3.8.9: a parameter is replaced in an argument, a tuple, an option, a function type and an
    // array length; composing applies the second substitution to what the first gave
    let design = support::build("struct Pair<T, U = T> { a: T, b: U }\nfn f<T, U, const N: usize>(x: T) -> T { x }\n");
    let declarations = Declarations { design: &design };
    let (t, u, n) = (declarations.parameter("T"), declarations.parameter("U"), declarations.parameter("N"));
    let pair = declarations.item("Pair");
    let generic = Ty::Struct(Nominal { decl: pair, arguments: vec![GenericValue::Type(Ty::Param(t)), GenericValue::Type(Ty::Option(Box::new(Ty::Param(u))))] });
    let array = Ty::Array(Box::new(generic.clone()), Length::Generic(ConstTerm::Param(n)));
    let mut inner = Substitution::new();
    inner.bind(t, GenericValue::Type(Ty::UInt(8)));
    inner.bind(n, GenericValue::Const(ConstTerm::Value(4)));
    let once = inner.apply(&array);
    let expected_pair = Ty::Struct(Nominal { decl: pair, arguments: vec![GenericValue::Type(Ty::UInt(8)), GenericValue::Type(Ty::Option(Box::new(Ty::Param(u))))] });
    assert_eq!(once, Ty::Array(Box::new(expected_pair), Length::Fixed(4)), "a generic length that becomes a number is fixed");
    let mut outer = Substitution::new();
    outer.bind(u, GenericValue::Type(Ty::Bool));
    let both = inner.compose(&outer);
    assert_eq!(both.apply(&array).name(&design), "[struct `Pair<u8, Option<bool>>`; 4]");
    assert_eq!(outer.apply(&Ty::Fn(vec![Ty::Param(u), Ty::Tuple(vec![Ty::Param(t)])], Box::new(Ty::Param(u)))), Ty::Fn(vec![Ty::Bool, Ty::Tuple(vec![Ty::Param(t)])], Box::new(Ty::Bool)));
    assert!(Substitution::new().is_empty() && !both.is_empty());
    let signature = GenericSignature::of(&design, pair);
    let given = Substitution::of(&signature, &[GenericValue::Type(Ty::F64)]);
    assert_eq!(given.get(t), Some(&GenericValue::Type(Ty::F64)));
    assert_eq!(given.get(u), None, "a parameter with no argument stays unbound");
}

#[test]
fn a_written_use_is_recorded_as_an_instantiation() {
    // GN-06: `Types::written_instantiation` gives the generic item and its arguments for a type node
    // that writes a use of it, and nothing for any other
    let src = "struct Pair<T, U = T> { a: T, b: U }\nmodule Top() { }\nanalog for Top { let p: Pair<u8>; let q: f64; }";
    let design = support::build(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let declared: Vec<_> = (0..design.decl_count()).map(|i| DeclId(i as u32)).filter(|d| matches!(design.decl_data(*d).kind, DeclKind::Binding(_))).collect();
    assert_eq!(declared.len(), 2, "two bindings");
    let pair = (0..design.decl_count()).map(|i| DeclId(i as u32)).find(|d| matches!(design.decl_data(*d).kind, DeclKind::Struct(_))).expect("Pair");
    let ty = types.local(declared[0]).expect("p has a type");
    assert_eq!(*ty, Ty::Struct(Nominal { decl: pair, arguments: vec![GenericValue::Type(Ty::UInt(8)), GenericValue::Type(Ty::UInt(8))] }));
    assert_eq!(types.local(declared[1]), Some(&Ty::F64));
    let uses: Vec<Instantiation> = (0..design.type_count()).filter_map(|i| types.written_instantiation(TypeId(i as u32))).collect();
    assert_eq!(uses, [Instantiation { item: pair, arguments: vec![GenericValue::Type(Ty::UInt(8)), GenericValue::Type(Ty::UInt(8))] }], "the one use of a generic type, defaults filled");
}

#[test]
fn a_generic_body_is_checked_once_at_its_definition() {
    // Q1 (c), GN-07 AC 1: a generic function nothing calls is typed against its own parameters, each an
    // opaque type, so a body that returns what it is given is clean and one that returns it as another
    // type is refused at the definition, once, naming the parameter
    let clean = support::build(&format!("{HEAD}fn id<T>(x: T) -> T {{ x }}\nfn first<T, const N: usize>(a: [T; N]) -> T {{ a[0] }}\nfn pick<T>(c: bool, a: T, b: T) -> T {{ if c {{ a }} else {{ b }} }}\n"));
    assert_eq!(Types::check_model(&clean), Ok(()), "a generic body that fits its signature checks");
    let found = GenericSources::model_errors("fn wrong<T>(x: T) -> f64 { x }\nfn twice<T>(x: T) -> T { let y: bool = x; x }\n");
    assert_eq!(GenericSources::codes_of(&found), ["E0180", "E0180"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("expected `f64`, found `T`"), "{:?}", found[0]);
    // the `where` ranges over the const parameters and is a `bool` (§3 rule 8)
    let guarded = support::build(&format!("{HEAD}fn wide<const N: usize>(x: f64) -> f64 where N > 2 {{ x }}\nfn flagged<const B: bool>(x: f64) -> f64 where B {{ x }}\n"));
    assert_eq!(Types::check_model(&guarded), Ok(()), "a const parameter is a const expression of its type");
    let unguarded = GenericSources::model_errors("fn bad<const N: usize>(x: f64) -> f64 where 1.0 { x }\n");
    assert_eq!(GenericSources::codes_of(&unguarded), ["E0180"], "{unguarded:?}");
}

#[test]
fn a_generic_argument_that_does_not_satisfy_its_bound_is_e0187() {
    // §3.8.7: `T: Repr` takes a type with a layout, `T: Ord` a type that derives `Ord`
    let items = "struct Held<T: Repr> { a: T }\nstruct Sorted<T: Ord> { a: T }\n";
    let found = GenericSources::model_errors(&format!("{items}fn f(a: Held<str>, b: Sorted<f64>) -> f64 {{ 1.0 }}\n"));
    assert_eq!(GenericSources::codes_of(&found), ["E0187", "E0187"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("`str` does not satisfy the bound `Repr` of `T` in `Held`"), "{:?}", found[0]);
    assert!(found[1].diagnostic.message.contains("`f64` does not satisfy the bound `Ord` of `T` in `Sorted`"), "{:?}", found[1]);
    let met = support::build(&format!("{HEAD}{items}fn g(a: Held<u8>, b: Sorted<u8>, c: Held<(f64, bool)>) -> f64 {{ 1.0 }}\n"));
    assert_eq!(Types::check_model(&met), Ok(()), "a bound that holds");
}

#[test]
fn bounds_give_operators_width_and_codecs() {
    // §3 rule 9, §2: `T: PartialEq` gives `==`, `T: PartialOrd` gives `<`, `T: Repr` gives `T::WIDTH`,
    // `encode` and `decode`, `T: Default` gives `T::default()`, `F: Fn(A) -> R` gives a call; each is
    // checked at the definition with nothing instantiating it
    let design = support::build(&format!(
        "{HEAD}fn same<T: PartialEq>(a: T, b: T) -> bool {{ a == b }}\n\
         fn less<T: Ord>(a: T, b: T) -> bool {{ a < b }}\n\
         fn width<T: Repr>(a: T) -> u32 {{ T::WIDTH }}\n\
         fn round<T: Repr>(a: T) -> Option<T> {{ T::decode(a.encode()) }}\n\
         fn made<T: Default>() -> T {{ T::default() }}\n\
         fn twice<F: Fn(f64) -> f64>(f: F, x: f64) -> f64 {{ f(f(x)) }}\n\
         fn both<T: Ord + Repr>(a: T, b: T) -> bool {{ a == b && T::WIDTH > 1 }}\n"
    ));
    assert_eq!(Types::check_model(&design), Ok(()), "every member a bound gives is typed");
}

#[test]
fn a_member_no_bound_gives_is_refused_at_the_definition() {
    // GN-07 AC 2: a method or an operator the bounds do not give is refused at the definition, naming the
    // method, the parameter and the bound that would give it, with nothing instantiating the function
    let found = GenericSources::model_errors(
        "fn no_eq<T>(a: T, b: T) -> bool { a == b }\n\
         fn no_order<T: PartialEq>(a: T, b: T) -> bool { a < b }\n\
         fn no_encode<T: PartialEq>(a: T) -> bool { let l = a.encode(); true }\n\
         fn no_method<T: Repr>(a: T) -> T { a.sqrt() }\n\
         fn no_clone<T: Repr>(a: T) -> T { a.clone() }\n",
    );
    assert_eq!(GenericSources::codes_of(&found), ["E0180", "E0180", "E0161", "E0161", "E0161"], "{found:?}");
    let messages: Vec<&str> = found.iter().map(|d| d.diagnostic.message.as_str()).collect();
    assert!(messages[0].contains("`T` has no `PartialEq`") && messages[0].contains("add `T: PartialEq`"), "{}", messages[0]);
    assert!(messages[1].contains("`T` has no `PartialOrd`"), "{}", messages[1]);
    assert!(messages[2].contains("no method `encode` on `T`") && messages[2].contains("the bounds PartialEq of `T`") && messages[2].contains("comes from `Repr`"), "{}", messages[2]);
    assert!(messages[3].contains("no method `sqrt` on `T`") && messages[3].contains("the bounds Repr of `T`"), "{}", messages[3]);
    assert!(messages[4].contains("every value is copied"), "{}", messages[4]);
    // `T::WIDTH` with no `Repr` bound does not resolve: the POM refuses it before the check
    assert_eq!(support::codes(&support::build(&format!("{HEAD}fn no_width<T>(a: T) -> u32 {{ T::WIDTH }}\n"))), ["E0161"]);
}

#[test]
fn a_length_over_const_parameters_fits_until_the_item_is_instantiated() {
    // §3.8.9, Q1 (c): `[f64; N + 1]` built in a body is the declared `[f64; N + 1]` at the definition (the
    // two lengths are compared by value at each instantiation), but `N` is not another parameter `M`
    let clean = support::build(&format!(
        "{HEAD}fn widened<const N: usize>(xs: [f64; N]) -> [f64; N + 1] {{\n    let wide = [0.0; N + 1];\n    for k in 0..N {{ wide[k] = xs[k]; }}\n    wide\n}}\n\
         fn dot<const N: usize>(a: [f64; N], b: [f64; N]) -> f64 {{ let sum = 0.0; for k in 0..N {{ sum += a[k] * b[k]; }} sum }}\n\
         fn mean_of<const N: usize>(xs: [f64; N]) -> f64 where N >= 1 {{ let sum = 0.0; for x in xs {{ sum += x; }} sum / N as f64 }}\n"
    ));
    assert_eq!(Types::check_model(&clean), Ok(()), "{:?}", Types::check_model(&clean));
    let found = GenericSources::model_errors("fn mixed<const N: usize, const M: usize>(a: [f64; N]) -> [f64; M] { a }\n");
    assert_eq!(GenericSources::codes_of(&found), ["E0180"], "two different parameters are two lengths: {found:?}");
}

#[test]
fn generic_arguments_are_explicit_or_inferred() {
    // §3.8.5, §3.8.7, §1.4.4: written with `::<..>`, read from an argument (a literal takes the type
    // another argument gives it), from an array's length, from the expected type, or from a default
    let items = "fn id<T>(x: T) -> T { x }\n\
                 fn larger<T: PartialOrd>(a: T, b: T) -> T { if a < b { b } else { a } }\n\
                 fn first<T, const N: usize>(a: [T; N]) -> T { a[0] }\n\
                 fn zeros<const N: usize>() -> [f64; N] { [0.0; N] }\n\
                 fn made<T: Default>() -> T { T::default() }\n\
                 fn pick<T, U = bool>(a: T, b: Option<U>) -> U { b.unwrap_or(false) }\n";
    let body = "let a = id(1.5); let b = id::<u8>(3); let c = larger(3u8, 200); let d = first([1.0, 2.0, 3.0]); \
                let e: [f64; 4] = zeros(); let f = zeros::<2>(); let g: Option<u8> = made(); let h = id(id(true));";
    let found = GenericSources::analog_types(items, body, &["a", "b", "c", "d", "e", "f", "g", "h"]).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(found, ["a = f64", "b = u8", "c = u8", "d = f64", "e = [f64; 4]", "f = [f64; 2]", "g = Option<u8>", "h = bool"]);
}

#[test]
fn an_uninferred_generic_argument_is_e0181_naming_the_parameter() {
    // §3.8.7, D6: nothing gives `T`, so the call is refused naming it and the way to write it
    let items = "fn made<T: Default>() -> T { T::default() }\nfn zeros<const N: usize>() -> [f64; N] { [0.0; N] }\n";
    let errors = GenericSources::analog_types(items, "let a = made();", &["a"]).expect_err("T has no argument");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].0, "E0181");
    assert!(errors[0].1.contains("cannot infer the type parameter `T` of `made`") && errors[0].1.contains("made::<..>"), "{}", errors[0].1);
    let constant = GenericSources::analog_types(items, "let a = zeros();", &["a"]).expect_err("N has no argument");
    assert!(constant[0].1.contains("cannot infer the const parameter `N` of `zeros`"), "{constant:?}");
    let many = GenericSources::analog_types(items, "let a = zeros::<1, 2>();", &["a"]).expect_err("too many arguments");
    assert_eq!(many[0].0, "E0187", "{many:?}");
}

#[test]
fn an_argument_that_misses_a_bound_is_e0187() {
    // §3.8.7: an inferred or written argument satisfies every bound of its parameter, else E0187 naming
    // the argument and the bound
    let items = "fn sorted<T: Ord>(a: T, b: T) -> T { if a < b { a } else { b } }\nfn held<T: Repr>(a: T) -> T { a }\n";
    let errors = GenericSources::analog_types(items, "let a = sorted(1.5, 2.5);", &["a"]).expect_err("f64 is not Ord");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].0, "E0187");
    assert!(errors[0].1.contains("`f64` does not satisfy the bound `Ord` of `T` in `sorted`"), "{}", errors[0].1);
    let written = GenericSources::analog_types(items, "let a = held::<str>(\"x\");", &["a"]).expect_err("str has no Repr");
    assert!(written[0].1.contains("`str` does not satisfy the bound `Repr` of `T` in `held`"), "{written:?}");
    let met = GenericSources::analog_types(items, "let a = sorted(1u8, 2); let b = held(true);", &["a", "b"]);
    assert_eq!(met, Ok(vec!["a = u8".to_string(), "b = bool".to_string()]));
}

#[test]
fn each_generic_call_records_its_instantiation() {
    // §3.8.10: `Types::instantiation(call)` is the function and the argument of each parameter, in order,
    // defaults filled in; a call of a plain function has none
    let src = "fn pick<T, U = bool>(a: T, b: Option<U>) -> T { a }\nfn plain(x: f64) -> f64 { x }\nmodule Top() { }\nanalog for Top { let a = pick(1.5, Some(2u8)); let b = pick::<u8, f64>(1, Some(2.0)); let c = plain(1.0); }";
    let design = support::build(&format!("{HEAD}{src}"));
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let declarations = Declarations { design: &design };
    let pick = declarations.item("pick");
    let calls: Vec<_> = (0..design.expr_count()).map(|i| piperine_lang::pom::ExprId(i as u32)).filter_map(|e| types.instantiation(e).map(|i| (e, i.clone()))).collect();
    assert_eq!(calls.len(), 2, "two calls of the generic function, none of the plain one: {calls:?}");
    assert_eq!(calls[0].1.item, pick);
    assert_eq!(calls[0].1.arguments, [GenericValue::Type(Ty::F64), GenericValue::Type(Ty::UInt(8))]);
    assert_eq!(calls[1].1.arguments, [GenericValue::Type(Ty::UInt(8)), GenericValue::Type(Ty::F64)]);
}

#[test]
fn fn_bounds_take_closures_and_function_names() {
    // §3.8.6, GN-01 AC 2: a closure (capturing or not, its parameters typed by the bound) and a `fn` name
    // pass for `F: Fn(f64) -> f64`; the instantiation records the closure or the function (D9)
    let items = "fn twice<F: Fn(f64) -> f64>(f: F, x: f64) -> f64 { f(f(x)) }\n\
                 fn halve(v: f64) -> f64 { v * 0.5 }\n\
                 fn apply<T, U, F: Fn(T) -> U>(f: F, x: T) -> U { f(x) }\n";
    let src = format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ let k = 3.0; let a = twice(|v| v * 2.0, 1.0); let b = twice(|v| v + k, 1.0); let c = twice(halve, 4.0); let d = apply(|n: u8| n > 1, 3u8); }}");
    let design = support::build(&format!("{HEAD}{src}"));
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let declarations = Declarations { design: &design };
    assert_eq!(types.local(GenericSources::binding(&design, "a")), Some(&Ty::F64));
    assert_eq!(types.local(GenericSources::binding(&design, "d")), Some(&Ty::Bool), "U is found from the closure's result");
    let calls: Vec<_> = (0..design.expr_count()).filter_map(|i| types.instantiation(piperine_lang::pom::ExprId(i as u32)).cloned()).collect();
    assert_eq!(calls.len(), 4, "{calls:?}");
    assert!(matches!(calls[0].arguments[0], GenericValue::Closure(_)), "a plain closure: {:?}", calls[0].arguments);
    assert!(matches!(calls[1].arguments[0], GenericValue::Closure(_)), "a capturing closure: {:?}", calls[1].arguments);
    assert_eq!(calls[2].arguments, [GenericValue::Function(declarations.item("halve"))], "a function name");
    assert_eq!(calls[3].arguments[..2], [GenericValue::Type(Ty::UInt(8)), GenericValue::Type(Ty::Bool)]);
    // an argument whose signature is not the bound's is refused
    let errors = GenericSources::analog_types(items, "let a = twice(|v: bool| v, 1.0);", &["a"]).expect_err("a closure over bool is no Fn(f64) -> f64");
    assert!(errors.iter().any(|(code, _)| code == "E0187" || code == "E0180"), "{errors:?}");
}

#[test]
fn a_call_through_an_fn_bound_is_positional_only() {
    // §3.2.11: the arguments of `f(x)` through `F: Fn(f64) -> f64` are positional; a named one is E0140
    let found = GenericSources::model_errors("fn twice<F: Fn(f64) -> f64>(f: F, x: f64) -> f64 { f(v: x) }\n");
    assert_eq!(GenericSources::codes_of(&found), ["E0140"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("by position"), "{:?}", found[0]);
}

#[test]
fn array_lengths_and_where_are_deferred_to_the_instantiation() {
    // Q1 (c), §3.8.9: the definition accepts `[f64; N + 1]` against `[f64; N + 1]` and records the pair, the `where` and the
    // expression; a call substitutes the lengths it infers, so a table of three and a table of four are a type error there
    let items = "fn widened<const N: usize>(xs: [f64; N]) -> [f64; N + 1] { let wide = [0.0; N + 1]; wide }\n\
                 fn mean_of<const N: usize>(xs: [f64; N]) -> f64 where N >= 1 { 1.0 }\n\
                 fn dot<const N: usize>(a: [f64; N], b: [f64; N]) -> f64 { 1.0 }\n";
    let src = format!("{HEAD}{items}\nmodule Top() {{ }}\nanalog for Top {{ let a = widened([1.0, 2.0]); let m = mean_of([1.0]); let d = dot([1.0, 2.0], [3.0, 4.0]); }}");
    let design = support::build(&src);
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let declarations = Declarations { design: &design };
    assert_eq!(types.local(GenericSources::binding(&design, "a")), Some(&Ty::array(Ty::F64, 3)), "N = 2 from the argument folds N + 1");
    let widened = types.obligations(declarations.item("widened"));
    assert!(widened.iter().any(|o| matches!(o, Obligation::Constant { .. })), "{widened:?}");
    assert!(widened.iter().any(|o| matches!(o, Obligation::SameLength { .. })), "the length of `wide` meets the declared one: {widened:?}");
    let mean = types.obligations(declarations.item("mean_of"));
    assert!(matches!(mean, [Obligation::Where { .. }]), "the `where` is deferred: {mean:?}");
    assert!(types.obligations(declarations.item("dot")).is_empty(), "the same parameter is the same length: nothing is owed");
    let found = GenericSources::analog_types(items, "let d = dot([1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0]);", &["d"]).expect_err("three and four");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "E0180");
    assert!(found[0].1.contains("expected `[f64; 3]`, found `[f64; 4]`"), "{}", found[0].1);
}

#[test]
fn generic_structs_and_enums_type_check_per_instance() {
    // §3.8, §1.3: a constructor finds its arguments from its fields or the expected type, a field read and a
    // pattern see the field types substituted, a derive holds for the arguments that implement it, and a
    // generic body builds and reads values of its own parameters
    let items = "struct Pair<T> { a: T, b: T }\nenum Maybe<T> { No, Yes(T) }\n#[derive(PartialEq)] struct Wrap<T> { v: T }\n\
                 fn swap<T>(p: Pair<T>) -> Pair<T> { Pair { a: p.b, b: p.a } }\n\
                 fn first_or<T>(m: Maybe<T>, d: T) -> T { match m { Maybe::Yes(v) => v, Maybe::No => d } }\n";
    let body = "let p = Pair { a: 1.0, b: 2.0 }; let x = p.a; let q = swap(p); let m = Maybe::Yes(3u8); let n: Maybe<bool> = Maybe::No; let r = first_or(m, 0); \
                let e = Wrap { v: 1.0 } == Wrap { v: 2.0 }; let w = Wrap { v: Maybe::Yes(true) };";
    let found = GenericSources::analog_types(items, body, &["p", "x", "q", "m", "n", "r", "e", "w"]).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(found, ["p = struct `Pair<f64>`", "x = f64", "q = struct `Pair<f64>`", "m = Maybe<u8>", "n = Maybe<bool>", "r = u8", "e = bool", "w = struct `Wrap<Maybe<bool>>`"]);
    // a field of the wrong type for the argument the others gave, and a name nothing gives
    let wrong = GenericSources::analog_types(items, "let p = Pair { a: 1.0, b: true };", &["p"]).expect_err("T is f64 and b is bool");
    assert_eq!(wrong[0].0, "E0180", "{wrong:?}");
    let open = GenericSources::analog_types(items, "let n = Maybe::No;", &["n"]).expect_err("nothing gives T");
    assert_eq!(open[0].0, "E0181", "{open:?}");
    // an instance compares only if its arguments do: `f64` has `PartialEq`, a derive over `Maybe` (which derives nothing) does not
    let unordered = GenericSources::analog_types(items, "let e = Wrap { v: Maybe::Yes(true) } == Wrap { v: Maybe::Yes(false) };", &["e"]).expect_err("Maybe has no PartialEq");
    assert_eq!(unordered[0].0, "E0180", "{unordered:?}");
}

#[test]
fn a_generic_repr_type_over_an_argument_without_repr_is_e0110() {
    // §2 rule 3, H.8 rule 6: `#[derive(Repr)]` on a generic type holds for the arguments that have a `Repr`
    let items = "#[derive(PartialEq, Repr)] struct Cell<T> { v: T }\n";
    let found = GenericSources::model_errors(&format!("{items}fn f(c: Cell<str>) -> f64 {{ 1.0 }}\n"));
    assert_eq!(GenericSources::codes_of(&found), ["E0110"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("`Cell` derives `Repr`, and its argument `str` has no `Repr`"), "{:?}", found[0]);
    let design = support::build(&format!("{HEAD}{items}fn g(c: Cell<u8>) -> f64 {{ 1.0 }}\nfn h(c: Cell<(f64, bool)>) -> f64 {{ 1.0 }}\n"));
    assert_eq!(Types::check_model(&design), Ok(()), "u8 and a tuple of a float and a bool have layouts");
}

/// The forms of generics phases C, D and E, each on the smallest model that writes it.
struct LaterPhases;

impl LaterPhases {
    /// The `(code, message)` pairs of the design and of the check of `Top` of `src`, over the standard library.
    fn found(src: &str) -> Vec<(String, String)> {
        let design = Design::builder().source("top.msl", src).build();
        let mut found: Vec<(String, String)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
        if let Err(refused) = Types::check(&design, "Top") {
            found.extend(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())));
        }
        found
    }

    /// Whether the check of `src` holds `P0001` with a message that contains `form`.
    fn refuses(src: &str, form: &str) -> bool {
        Self::found(src).iter().any(|(code, message)| code == "P0001" && message.contains(form))
    }
}

#[test]
fn phase_c_d_and_e_forms_stay_p0001_by_name() {
    // GN-29 AC 2: a form that a later phase delivers stays `P0001` naming the form, never run by another rule: a generic
    // module and generation (phase C), a template value and a placement through one (phase C, GN-21 to GN-23)
    let generic_module = "module Hold<T: Repr>(in d: Bit<T>) {}\nmodule Top() { signal d: Bit<u8>; let h = Hold::<u8>(d); }\n";
    assert!(LaterPhases::refuses(generic_module, "generic module"), "{:?}", LaterPhases::found(generic_module));
    let generation = "module Top() { signal o: Electrical; for k in 0..2 { let r = 1; } }\n";
    assert!(LaterPhases::refuses(generation, "generation `for`/`if` in a module body"), "{:?}", LaterPhases::found(generation));
    let template = "module Cell(a: Electrical) {}\nmodule Top() { signal a: Electrical; let c = Cell { }; }\n";
    assert!(LaterPhases::refuses(template, "module template"), "{:?}", LaterPhases::found(template));
    let through = "module Cell(a: Electrical) {}\nmodule Top(cell: Cell) { signal a: Electrical; let p = cell(a); }\n";
    assert!(LaterPhases::refuses(through, "placement through a template value or a function"), "{:?}", LaterPhases::found(through));
}

#[test]
fn m_and_dtemp_bound_in_a_param_block_stay_refused_until_phase_d() {
    // GN-26 (phase D, with `params-and-io`): binding `m` or `dtemp` is `E0320` today, `no param`, the one refusal that
    // names the form; it must not run as anything else until `m` and `dtemp` are implicit params
    let src = "module Resistor(a: Electrical, b: Electrical) { r: f64 = 1k; }\nmodule Top() { signal a: Electrical; let r1 = Resistor(a, gnd) { r: 1k, m: 4.0 }; }\n";
    let found = LaterPhases::found(src);
    assert!(found.iter().any(|(code, message)| code == "E0320" && message == "no param `m` in `Resistor`"), "{found:?}");
}

#[test]
fn an_unreached_method_of_a_generic_impl_is_checked_at_its_definition() {
    // S3 / GN-07 (Q1 (c)): the whole-model check seeds a method of a generic `impl` as it seeds a generic function, so an
    // error in one nothing reaches is reported where it is written
    let src = "struct Pair<T> { a: T, b: T }\nimpl<T> Pair<T> { fn wrong(self) -> bool { 1.0 } }\n";
    let codes = GenericSources::standard_codes(src);
    assert!(!codes.is_empty() && codes.iter().all(|code| code != "P0001"), "{codes:?}");
    assert!(GenericSources::standard_codes("struct Pair<T> { a: T, b: T }\nimpl<T> Pair<T> { fn first(self) -> T { self.a } }\n").is_empty());
}

#[test]
fn a_call_of_a_generic_extern_intrinsic_types_through_the_generic_path() {
    // finding 10 of the phase A and B verification: `logic<T: Repr>(pattern: str) -> T` and `matches_bits<T: Repr>` are
    // generic `extern "intrinsic"` functions of the standard library; a call finds `T` as any generic call does (from the
    // argument, from the expected type or written), and a `T` nothing gives is `E0181` naming it, never "generic function
    // is not supported yet"
    let typed = |src: &str| GenericSources::standard_found(src);
    let messages = |src: &str| typed(src).iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>();
    let clean = "module Top() { signal a: Bit<u4> = logic(\"1x01\"); signal b: Bit<u4> = logic::<u4>(\"1x01\"); }\nfn matches(x: u8) -> bool { matches_bits(x, \"1x\") }\nfn matches_written(x: u8) -> bool { matches_bits::<u8>(x, \"1x\") }\n";
    assert_eq!(messages(clean), []);
    let uninferred = messages("fn nothing() -> bool { let x = logic(\"10\"); true }\n");
    assert!(uninferred.iter().any(|(code, message)| code == "E0181" && message.contains('T') && message.contains("logic")), "{uninferred:?}");
    assert!(uninferred.iter().all(|(_, message)| !message.contains("generic function")), "{uninferred:?}");
    let unbounded = messages("struct Plain { a: f64 }\nfn bad(p: Plain) -> bool { matches_bits(p, \"1\") }\n");
    assert!(unbounded.iter().any(|(code, _)| code == "E0187"), "a `T: Repr` argument without `Repr` is the bound error: {unbounded:?}");
}
