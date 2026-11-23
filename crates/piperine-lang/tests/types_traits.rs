//! Scope: the checker's traits and `impl`s (generics GN-03, GN-04, GN-10, part I §3.1, §3.7): the
//! table that answers the inherent item, the trait item visible on a type, the implementation of a
//! trait item and whether a type implements a trait.

#[path = "support/pom.rs"]
mod support;

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

/// The natures every source below places in the open.
const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// The type names of the bindings of `Top`'s `analog` block after `items`, as `name = type`, or the codes and messages of the check.
fn analog_types(items: &str, body: &str, names: &[&str]) -> Result<Vec<String>, Vec<(String, String)>> {
    let design = support::build(&format!("{HEAD}{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}"));
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}", support::codes(&design));
    match piperine_lang::types::Types::check(&design, "Top") {
        Ok(types) => Ok(names
            .iter()
            .map(|name| {
                let decl = (0..design.decl_count()).map(|i| DeclId(i as u32)).find(|d| matches!(design.decl_data(*d).kind, DeclKind::Binding(_)) && design.decl_data(*d).name.as_ref().is_some_and(|n| n.text == *name)).unwrap_or_else(|| panic!("no binding `{name}`"));
                format!("{name} = {}", types.local(decl).map_or_else(|| "?".to_string(), |ty| ty.name(&design)))
            })
            .collect()),
        Err(found) => Err(found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()),
    }
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
    let found = analog_types(items, body, &["s", "a", "t", "n", "p", "d"]).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(found, ["s = struct `Sq`", "a = f64", "t = struct `Sq`", "n = u32", "p = f64", "d = f64"]);
    // an argument of another type, and a call of an associated function as a method, are refused
    let wrong = analog_types(items, "let s = Sq::make(2.0); let t = s.scaled(true);", &["t"]).expect_err("a bool for an f64");
    assert_eq!(wrong[0].0, "E0180", "{wrong:?}");
    let no_self = analog_types(items, "let s = Sq::make(2.0); let t = s.make(1.0);", &["t"]).expect_err("`make` has no `self`");
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
