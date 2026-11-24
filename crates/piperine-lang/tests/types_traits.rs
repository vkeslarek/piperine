//! Scope: the checker's traits and `impl`s (generics GN-03, GN-04, GN-10, part I §3.1, §3.7): the
//! table that answers the inherent item, the trait item visible on a type, the implementation of a
//! trait item and whether a type implements a trait.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use generic_sources::GenericSources;

/// The natures every source places in the open.
const HEAD: &str = GenericSources::HEAD;

use piperine_lang::pom::{DeclId, DeclKind, Design};
use piperine_lang::types::{ImplTable, Ty};

/// The declarations of a source, looked up by their owner and name.
struct Items<'d> {
    design: &'d Design,
}

impl Items<'_> {
    /// The declaration of kind `kind` named `name` whose parent is named `parent` (`None`: any parent).
    fn find(&self, parent: Option<&str>, name: &str, kind: fn(&DeclKind) -> bool) -> DeclId {
        (0..self.design.decl_count())
            .map(|i| DeclId(i as u32))
            .find(|decl| {
                let data = self.design.decl_data(*decl);
                let parent_ok = parent.is_none_or(|wanted| data.parent.is_some_and(|p| self.design.decl_data(p).name.as_ref().is_some_and(|n| n.text == wanted)));
                kind(&data.kind) && parent_ok && data.name.as_ref().is_some_and(|n| n.text == name)
            })
            .unwrap_or_else(|| panic!("no `{name}`"))
    }

    /// The trait `name`.
    fn trait_(&self, name: &str) -> DeclId {
        self.find(None, name, |k| matches!(k, DeclKind::Trait(_)))
    }

    /// The function `name` declared in the trait `owner`.
    fn trait_fn(&self, owner: &str, name: &str) -> DeclId {
        self.find(Some(owner), name, |k| matches!(k, DeclKind::Fn(_)))
    }

    /// The struct `name`, as a type.
    fn structure(&self, name: &str) -> Ty {
        Ty::structure(self.find(None, name, |k| matches!(k, DeclKind::Struct(_))))
    }
}

const SOURCE: &str = "trait Shape { fn area(self) -> f64; fn sides(self) -> u32 { 3 } }\n\
trait Solid: Shape { fn volume(self) -> f64; }\n\
struct Sq { a: f64 }\n\
impl Sq { fn make() -> Sq { Sq { a: 1.0 } } const KIND: u32 = 4; }\n\
impl Shape for Sq { fn area(self) -> f64 { self.a } }\n\
trait Known { fn known(self) -> bool { true } }\n\
impl<T: Repr> Known for T {}\n\
trait Describe { fn describe(self) -> f64; }\n\
impl Describe for f64 { fn describe(self) -> f64 { self } }\n";

#[test]
fn the_impl_table_answers_inherent_visible_and_implementation() {
    // §3.1.7, §3.7.4, §3.7.5, §3.7.8: the inherent item of a type, the trait item visible on it (the
    // `impl`'s own, else the trait's default), the implementation of a trait item for a type, and
    // whether a type implements a trait, a blanket `impl` included where its bound holds
    let design = support::build(SOURCE);
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let items = Items { design: &design };
    let table = ImplTable::new(&design);
    let sq = items.structure("Sq");
    let admits = &mut |_: &Ty, _: &piperine_lang::types::Bound| true;
    let refuses = &mut |_: &Ty, _: &piperine_lang::types::Bound| false;

    // inherent: the `impl Sq` items, a constant included; a trait item is not inherent
    assert!(table.inherent(&sq, "make").is_some() && table.inherent(&sq, "KIND").is_some());
    assert_eq!(table.inherent(&sq, "area"), None);
    assert_eq!(table.inherent(&Ty::F64, "make"), None);

    // visible: the `impl`'s own item, the trait's default, and a blanket `impl` only where its bound holds
    let own = table.visible(&sq, "area", admits);
    assert_eq!(own.len(), 1);
    assert_ne!(own[0], items.trait_fn("Shape", "area"), "the impl's own `area`, not the declaration");
    assert_eq!(table.visible(&sq, "sides", admits), [items.trait_fn("Shape", "sides")], "no override: the trait's default");
    assert_eq!(table.visible(&sq, "known", admits), [items.trait_fn("Known", "known")], "the blanket impl");
    assert_eq!(table.visible(&sq, "known", refuses), Vec::<DeclId>::new(), "a bound that fails leaves the blanket impl out");
    assert_eq!(table.visible(&Ty::F64, "describe", refuses).len(), 1, "an impl for a scalar needs no bound");

    // implementation: the `impl`'s item, else the default; nothing for a type that does not implement the trait
    assert_eq!(table.implementation(items.trait_fn("Shape", "area"), &sq, admits), Some(own[0]));
    assert_eq!(table.implementation(items.trait_fn("Shape", "sides"), &sq, admits), Some(items.trait_fn("Shape", "sides")));
    assert_eq!(table.implementation(items.trait_fn("Shape", "area"), &Ty::Bool, admits), None);
    assert_eq!(table.implementation(items.trait_fn("Solid", "volume"), &sq, admits), None);

    // implements, and the supertraits a trait requires
    assert!(table.implements(&sq, items.trait_("Shape"), admits) && !table.implements(&Ty::F64, items.trait_("Shape"), admits));
    assert!(table.implements(&Ty::F64, items.trait_("Describe"), admits) && !table.implements(&Ty::Bool, items.trait_("Describe"), admits));
    assert!(table.implements(&Ty::Bool, items.trait_("Known"), admits) && !table.implements(&Ty::Bool, items.trait_("Known"), refuses));
    assert_eq!(table.supertraits(items.trait_("Solid")), [items.trait_("Shape")]);
    assert!(table.supertraits(items.trait_("Shape")).is_empty());
    assert_eq!(table.traits().len(), 4);
    assert!(table.defines_method("area", |_| false) && !table.defines_method("nothing", |_| false));
}

#[test]
fn methods_associated_functions_and_constants_type_check() {
    // §3.1, §3.7: a method resolves through the inherent `impl` and then through a trait (its own item or
    // its default), `Type::f(..)` and `Type::NAME` are an associated function and a constant, and `Self`
    // is the type of the `impl`
    let items = "struct Sq { a: f64 }\n\
                 impl Sq { fn make(side: f64) -> Self { Sq { a: side } } const SIDES: u32 = 4; fn area(self) -> f64 { self.a * self.a } fn scaled(self, k: f64) -> Sq { Sq { a: self.a * k } } }\n\
                 trait Shape { fn perimeter(self) -> f64; fn double(self) -> f64 { 2.0 } }\n\
                 impl Shape for Sq { fn perimeter(self) -> f64 { 4.0 * self.a } }\n";
    let body = "let s = Sq::make(2.0); let a = s.area(); let t = s.scaled(2.0); let n = Sq::SIDES; let p = s.perimeter(); let d = s.double();";
    let found = GenericSources::analog_types(items, body, &["s", "a", "t", "n", "p", "d"]).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(found, ["s = struct `Sq`", "a = f64", "t = struct `Sq`", "n = u32", "p = f64", "d = f64"]);
    // an argument of another type, and a call of an associated function as a method, are refused
    let wrong = GenericSources::analog_types(items, "let s = Sq::make(2.0); let t = s.scaled(true);", &["t"]).expect_err("a bool for an f64");
    assert_eq!(wrong[0].0, "E0180", "{wrong:?}");
    let no_self = GenericSources::analog_types(items, "let s = Sq::make(2.0); let t = s.make(1.0);", &["t"]).expect_err("`make` has no `self`");
    assert!(no_self[0].0 == "E0161" && no_self[0].1.contains("associated function with no `self`"), "{no_self:?}");
}

#[test]
fn self_outside_an_impl_or_not_first_is_e0187() {
    // §3.7: `self` is the first parameter of a function in an `impl` or a trait
    let design = support::build(&format!("{HEAD}fn loose(self) -> f64 {{ 1.0 }}\nstruct S {{ a: f64 }}\nimpl S {{ fn late(k: f64, self) -> f64 {{ 1.0 }} }}\n"));
    let found = piperine_lang::types::Types::check_model(&design).expect_err("both are refused");
    let codes: Vec<String> = found.iter().map(|d| d.diagnostic.code.to_string()).collect();
    assert_eq!(codes, ["E0187", "E0187"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("neither"), "{:?}", found[0]);
    assert!(found[1].diagnostic.message.contains("must be the first parameter"), "{:?}", found[1]);
}

#[test]
fn an_impl_on_a_module_type_is_accepted() {
    // §3.1.4 (the user's decision of 2026-11-22, replaces `an_impl_on_a_module_is_e0130`): a module type takes an inherent `impl`
    assert_eq!(GenericSources::standard_codes("module M() { }\nimpl M { fn f(self) -> f64 { 1.0 } }\n"), Vec::<String>::new());
}

#[test]
fn a_method_named_like_a_carrier_method_is_e0131() {
    // §3.1.3: `changed`, `prev`, `others` and `inflow` are the methods of every carrier
    let codes = GenericSources::standard_codes("struct S { a: f64 }\nimpl S { fn changed(self) -> bool { true } fn prev(self) -> f64 { 1.0 } fn others(self) -> f64 { 1.0 } fn inflow(self) -> f64 { 1.0 } fn fine(self) -> f64 { 1.0 } }\n");
    assert_eq!(codes, ["E0131", "E0131", "E0131", "E0131"]);
}

#[test]
fn a_non_function_item_in_an_impl_on_a_nature_is_e0132() {
    // §3.1 Resolution: an `impl` on a nature or a bundle holds functions only; a value type may hold constants
    assert_eq!(GenericSources::standard_codes("impl E { const K: u32 = 1; fn f(self) -> f64 { 1.0 } }\n"), ["E0132"]);
    assert_eq!(GenericSources::standard_codes("struct Bn { out a: Bit }\nimpl Bn { const K: u32 = 1; }\n"), ["E0132"]);
    assert_eq!(GenericSources::standard_codes("struct S { a: f64 }\nimpl S { const K: u32 = 1; }\n"), Vec::<String>::new());
}

#[test]
fn impl_trait_for_a_bundle_is_e0134() {
    // §3.1.9: a trait is not implemented for a bundle
    assert_eq!(GenericSources::standard_codes("trait Tr { fn f(self) -> f64; }\nstruct Bn { out a: Bit }\nimpl Tr for Bn { fn f(self) -> f64 { 1.0 } }\n"), ["E0134"]);
}

#[test]
fn impl_trait_for_a_nature_is_accepted() {
    // §3.1.9, MD-78: a nature takes trait impls, its `self` a handle; a trait that a nature implements is a capability
    assert_eq!(GenericSources::standard_codes("trait Cap { fn cap(self) -> f64; }\nimpl Cap for E { fn cap(self) -> f64 { 1.0 } }\n"), Vec::<String>::new());
}

#[test]
fn a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type() {
    // §3.7.4: every item of the trait without a default is defined; the one with a default may be left out
    let found = GenericSources::e0187_messages("trait Shape { fn area(self) -> f64; fn sides(self) -> u32 { 3 } }\nstruct Sq { a: f64 }\nimpl Shape for Sq { }\n");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("`impl Shape for Sq` leaves out `area`"), "{}", found[0]);
    assert!(GenericSources::e0187_messages("trait Shape { fn sides(self) -> u32 { 3 } }\nstruct Sq { a: f64 }\nimpl Shape for Sq { }\n").is_empty());
}

#[test]
fn extra_items_signatures_supertraits_crate_and_pub_are_e0187() {
    let shape = "trait Shape { fn area(self) -> f64; }\nstruct Sq { a: f64 }\n";
    // §3.7.4: an item the trait lacks, and an item of another signature
    let extra = GenericSources::e0187_messages(&format!("{shape}impl Shape for Sq {{ fn area(self) -> f64 {{ 1.0 }} fn extra(self) -> f64 {{ 1.0 }} }}\n"));
    assert!(extra.len() == 1 && extra[0].contains("`impl Shape for Sq` defines `extra`, which the trait does not have"), "{extra:?}");
    let signature = GenericSources::e0187_messages(&format!("{shape}impl Shape for Sq {{ fn area(self) -> bool {{ true }} }}\n"));
    assert!(signature.len() == 1 && signature[0].contains("`area` has the type `fn(struct `Sq`) -> bool`, the trait gives it `fn(struct `Sq`) -> f64`"), "{signature:?}");
    // §3.7.3: a supertrait the type does not implement
    let supertrait = GenericSources::e0187_messages(&format!("{shape}trait Solid: Shape {{ fn volume(self) -> f64; }}\nimpl Solid for Sq {{ fn volume(self) -> f64 {{ 1.0 }} }}\n"));
    assert!(supertrait.len() == 1 && supertrait[0].contains("does not implement the supertrait `Shape`"), "{supertrait:?}");
    // §3.7.11: no `pub` on an item of a trait impl
    let public = GenericSources::e0187_messages(&format!("{shape}impl Shape for Sq {{ pub fn area(self) -> f64 {{ 1.0 }} }}\n"));
    assert!(public.len() == 1 && public[0].contains("written `pub`"), "{public:?}");
    // §3.7.8, MD-78: the arguments a trait takes are named by the `impl`, and each item is checked under them
    let generic = "trait Conv<X, Y = X> { fn conv(self, x: X) -> Y; }\nstruct S { a: f64 }\n";
    assert!(GenericSources::e0187_messages(&format!("{generic}impl Conv<f64> for S {{ fn conv(self, x: f64) -> f64 {{ x }} }}\n")).is_empty(), "the default Y = X is f64");
    let under = GenericSources::e0187_messages(&format!("{generic}impl Conv<f64, u8> for S {{ fn conv(self, x: f64) -> f64 {{ x }} }}\n"));
    assert!(under.len() == 1 && under[0].contains("`conv`"), "{under:?}");
}

#[test]
fn an_impl_is_written_in_the_crate_of_the_trait_or_of_the_type() {
    // §3.7.7: `impl Tr for T` is in the crate that declares `Tr` or the one that declares `T`
    use piperine_lang::pom::CrateGraph;
    let mut graph = CrateGraph::new();
    let app = graph.add_crate("app", "app/lib.msl").unwrap();
    let util = graph.add_crate("util", "util/lib.msl").unwrap();
    graph.add_dependency(app, "util", util).unwrap();
    let lib = "pub trait Tr { fn f(self) -> f64; }\n";
    let own_type = "use util::Tr;\nstruct S { a: f64 }\nimpl Tr for S { fn f(self) -> f64 { 1.0 } }\n";
    let foreign = "use util::Tr;\nimpl Tr for f64 { fn f(self) -> f64 { 1.0 } }\n";
    for (src, expected) in [(own_type, 0), (foreign, 1)] {
        let design = Design::builder().std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", "").source("app/lib.msl", src).source("util/lib.msl", lib).crates(graph.clone()).build();
        assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
        let found = piperine_lang::types::Types::check_model(&design).err().unwrap_or_default();
        assert_eq!(found.len(), expected, "{src}: {found:?}");
        assert!(found.iter().all(|d| d.diagnostic.message.contains("declares neither the trait nor the type")), "{found:?}");
    }
}

#[test]
fn trait_methods_resolve_on_values_through_bounds_and_by_path() {
    // §3.7.8: a trait's method on a value is the impl's item or the trait's default (`Callee::Method`), through a
    // bound it is the trait's function (`Callee::Bound`), and `Tr::f(x)` and `T::f(x)` call a trait item by path
    let items = "trait Shape { fn area(self) -> f64; fn doubled(self) -> f64 { 2.0 * self.area() } }\n\
                 struct Sq { a: f64 }\n\
                 impl Shape for Sq { fn area(self) -> f64 { self.a } }\n\
                 fn through<T: Shape>(x: T) -> f64 { x.area() + T::area(x) + Shape::area(x) + x.doubled() }\n";
    let body = "let s = Sq { a: 2.0 }; let a = s.area(); let b = s.doubled(); let c = Shape::area(s); let d = through(s);";
    let found = GenericSources::analog_types(items, body, &["a", "b", "c", "d"]).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(found, ["a = f64", "b = f64", "c = f64", "d = f64"]);
    let design = support::build(&format!("{HEAD}{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}"));
    let types = piperine_lang::types::Types::check(&design, "Top").expect("the source checks");
    let table = Items { design: &design };
    let (area, doubled) = (table.trait_fn("Shape", "area"), table.trait_fn("Shape", "doubled"));
    let callees: Vec<_> = (0..design.expr_count()).filter_map(|i| types.callee(piperine_lang::pom::ExprId(i as u32))).collect();
    let bound = callees.iter().filter(|callee| matches!(callee, piperine_lang::types::Callee::Bound { item } if *item == area || *item == doubled)).count();
    assert_eq!(bound, 5, "x.area(), T::area(x), Shape::area(x) and x.doubled() inside `through`, and self.area() in the default: {callees:?}");
    assert!(callees.iter().any(|callee| matches!(callee, piperine_lang::types::Callee::Method(item) if *item == doubled)), "s.doubled() is the trait's default");
    let own = callees.iter().filter(|callee| matches!(callee, piperine_lang::types::Callee::Method(item) if *item != doubled)).count();
    assert_eq!(own, 2, "s.area() and Shape::area(s) are the impl's own item: {callees:?}");
    let missing = GenericSources::analog_types(items, "let s = 1.0; let c = Shape::area(s);", &["c"]).expect_err("f64 does not implement Shape");
    assert_eq!(missing[0].0, "E0187", "{missing:?}");
}

#[test]
fn a_bound_with_trait_arguments_is_met_by_the_impl_naming_them() {
    // MD-78: `T: Conv<u8>` is satisfied by `impl Conv<u8> for S` and not by an impl for other arguments
    let items = "trait Conv<X> { fn conv(self, x: X) -> X; }\nstruct S { a: f64 }\n\
                 impl Conv<u8> for S { fn conv(self, x: u8) -> u8 { x } }\n\
                 fn takes_u8<T: Conv<u8>>(t: T) -> u8 { 1 }\nfn takes_bool<T: Conv<bool>>(t: T) -> u8 { 1 }\n";
    let met = GenericSources::analog_types(items, "let s = S { a: 1.0 }; let r = takes_u8(s);", &["r"]);
    assert_eq!(met, Ok(vec!["r = u8".to_string()]));
    let unmet = GenericSources::analog_types(items, "let s = S { a: 1.0 }; let r = takes_bool(s);", &["r"]).expect_err("an impl for u8 is not one for bool");
    assert!(unmet[0].0 == "E0187" && unmet[0].1.contains("does not satisfy the bound `Conv`"), "{unmet:?}");
}

#[test]
fn a_blanket_impl_applies_to_every_type_its_bound_admits() {
    // §3.7.5: `impl<T: Repr> Known for T` gives `known()` to every type with a layout and to none without one
    let items = "trait Known { fn known(self) -> bool { true } }\nimpl<T: Repr> Known for T {}\n#[derive(PartialEq, Repr)] enum Mode { A, B }\nstruct Plain { a: f64 }\n";
    let met = GenericSources::analog_types(items, "let x = 3u8; let y = true; let z = 1.5; let m = Mode::A; let a = x.known(); let b = y.known(); let d = z.known(); let c = m.known();", &["a", "b", "d", "c"]);
    assert_eq!(met, Ok(["a = bool", "b = bool", "d = bool", "c = bool"].map(String::from).to_vec()));
    // a struct with no layout is not admitted
    let refused = GenericSources::analog_types(items, "let p = Plain { a: 1.0 }; let e = p.known();", &["e"]).expect_err("a struct without Repr");
    assert_eq!(refused[0].0, "E0161", "{refused:?}");
    let design = support::build(&format!("{HEAD}{items}"));
    let table = piperine_lang::types::ImplTable::new(&design);
    let known = Items { design: &design }.trait_("Known");
    let all = &mut |_: &Ty, _: &piperine_lang::types::Bound| true;
    assert!(table.implements(&Ty::F64, known, all));
}

#[test]
fn overlapping_impls_are_e0187() {
    // §3.7.6: two `impl`s of one trait that apply to one type are refused used or not; arguments tell
    // impls of one generic trait apart
    let found = GenericSources::e0187_messages("trait Tr { fn f(self) -> f64 { 1.0 } }\nstruct S { a: f64 }\nimpl Tr for S {}\nimpl Tr for S {}\n");
    assert!(found.len() == 1 && found[0].contains("two `impl`s of `Tr` apply to `S`"), "{found:?}");
    let blanket = GenericSources::e0187_messages("trait Tr { fn f(self) -> f64 { 1.0 } }\nstruct S { a: f64 }\nimpl<T: Repr> Tr for T {}\nimpl Tr for u8 {}\n");
    assert!(blanket.len() == 1 && blanket[0].contains("two `impl`s of `Tr` apply to `u8`"), "{blanket:?}");
    let disjoint = "trait Tr { fn f(self) -> f64 { 1.0 } }\nstruct S { a: f64 }\nimpl<T: Repr> Tr for T {}\nimpl Tr for S {}\n";
    assert!(GenericSources::e0187_messages(disjoint).is_empty(), "S has no Repr, so the blanket impl does not apply to it");
    let args = "trait Cv<X> { fn c(self, x: X) -> X { x } }\nstruct S { a: f64 }\nimpl Cv<u8> for S {}\nimpl Cv<bool> for S {}\n";
    assert!(GenericSources::e0187_messages(args).is_empty(), "the trait's arguments tell the impls apart");
}

/// The items of a correct one-hot `impl Repr`.
const ENCODE: &str = "fn encode(self) -> [bool; 4] { [self == OneHot::A, self == OneHot::B, self == OneHot::C, self == OneHot::D] }";
const DECODE: &str = "fn decode(levels: [bool; 4]) -> Option<OneHot> { if levels[0] { Some(OneHot::A) } else if levels[1] { Some(OneHot::B) } else if levels[2] { Some(OneHot::C) } else if levels[3] { Some(OneHot::D) } else { None } }";

/// A one-hot enum whose `Repr` is written by hand, with the items `encode` and `decode` as given.
const ONE_HOT: &str = "#[derive(PartialEq)] enum OneHot { A, B, C, D }\nimpl Repr for OneHot { const WIDTH: u32 = 4; ";

#[test]
fn a_hand_written_repr_type_checks_and_is_carried() {
    // §2 rules 2 and 4: a one-hot `impl Repr` gives the type `WIDTH` levels, so a `Bit<OneHot>` carrier is accepted
    // (E0112 for the same enum without one), `OneHot::WIDTH` reads 4 and `x.encode()` is `[bool; 4]`
    let items = format!("{ONE_HOT}{ENCODE} {DECODE} }}\nfn width() -> u32 {{ OneHot::WIDTH }}\nfn levels(x: OneHot) -> [bool; 4] {{ x.encode() }}\nmodule Top() {{ signal s: Bit<OneHot> = OneHot::A; }}\n");
    let design = Design::builder().source("top.msl", format!("{HEAD}{items}")).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = piperine_lang::types::Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    assert!(!types.reached().is_empty());
    assert_eq!(GenericSources::standard_codes(&items), Vec::<String>::new(), "the whole model checks too");
    let without = Design::builder().source("top.msl", format!("{HEAD}#[derive(PartialEq)] enum OneHot {{ A, B }}\nmodule Top() {{ signal s: Bit<OneHot> = OneHot::A; }}\n")).build();
    let found = piperine_lang::types::Types::check(&without, "Top").err().expect("no Repr");
    assert_eq!(found[0].diagnostic.code, "E0112");
}

#[test]
fn a_hand_written_repr_missing_an_item_is_e0187() {
    // §2: `WIDTH`, `encode` and `decode`, no other item, each with the signature §2 gives
    let missing = GenericSources::e0187_messages(&format!("{ONE_HOT}{ENCODE} }}\n"));
    assert!(missing.len() == 1 && missing[0].contains("`impl Repr for OneHot` leaves out `decode`"), "{missing:?}");
    let extra = GenericSources::e0187_messages(&format!("{ONE_HOT}{ENCODE} {DECODE} fn more(self) -> f64 {{ 1.0 }} }}\n"));
    assert!(extra.len() == 1 && extra[0].contains("defines `more`"), "{extra:?}");
    let wrong = GenericSources::e0187_messages(&format!("{ONE_HOT}fn encode(self) -> [bool; 3] {{ [true, false, false] }} {DECODE} }}\n"));
    assert!(wrong.len() == 1 && wrong[0].contains("`encode` has the type"), "{wrong:?}");
}

/// A generic `Pair<T>` whose `Repr` is written by hand: two copies of the levels of `T`, so its `WIDTH` is `2 * T::WIDTH`.
const GENERIC_PAIR: &str = "#[derive(PartialEq)] struct Pair<T> { a: T, b: T }\n\
    impl<T: Repr> Repr for Pair<T> {\n\
        const WIDTH: u32 = 2 * T::WIDTH;\n\
        fn encode(self) -> [bool; 2 * T::WIDTH] { let first = self.a.encode(); let second = self.b.encode(); [false; 2 * T::WIDTH] }\n\
        fn decode(levels: [bool; 2 * T::WIDTH]) -> Option<Pair<T>> { None }\n\
    }\n";

#[test]
fn a_hand_written_repr_on_a_generic_type_is_typed_per_argument() {
    // GN-32 AC 1 (§2 rule 2, §3.8): `impl<T: Repr> Repr for Pair<T>` is accepted with `T` opaque, and `Pair<u8>`, `Pair<bool>`
    // and `Pair<f64>` then satisfy `Repr` through their own `WIDTH` (`2 * T::WIDTH`), so each is a carried type
    let carriers = "module Top() { signal s: Bit<Pair<u8>> = Pair { a: 1u8, b: 2u8 }; signal t: Bit<Pair<bool>> = Pair { a: true, b: false }; signal u: Bit<Pair<f64>> = Pair { a: 1.0, b: 2.0 }; }\n";
    let items = format!("{GENERIC_PAIR}{carriers}");
    let design = Design::builder().source("top.msl", format!("{HEAD}{items}")).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let checked = piperine_lang::types::Types::check(&design, "Top");
    assert!(checked.is_ok(), "{:?}", checked.err().map(|found| found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>()));
    assert_eq!(GenericSources::standard_codes(&items), Vec::<String>::new(), "the whole model checks too");
    // an argument with no `Repr` leaves the instance without one: E0112, naming the instance
    let plain = format!("{GENERIC_PAIR}struct Plain {{ x: f64 }}\nmodule Top() {{ signal s: Bit<Pair<Plain>> = Pair {{ a: Plain {{ x: 1.0 }}, b: Plain {{ x: 2.0 }} }}; }}\n");
    let design = Design::builder().source("top.msl", format!("{HEAD}{plain}")).build();
    let found = piperine_lang::types::Types::check(&design, "Top").err().expect("Plain has no Repr");
    assert_eq!(found[0].diagnostic.code, "E0112", "{:?}", found.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
}
