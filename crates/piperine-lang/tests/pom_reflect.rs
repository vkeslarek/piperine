//! Scope: the reflection queries over a resolved design (POM-11): the lookups
//! and casts, the carrier nature of a port or signal, the instance views and
//! the reverse queries.

use piperine_lang::pom::{Decl, Design, Reflect};

const PRELUDE: &str = include_str!("fixtures/pom/prelude.msl");

/// A design of `src` with the fixture prelude beside it.
fn with_prelude(src: &str) -> Design {
    Design::builder().prelude("prelude.msl", PRELUDE).source("top.msl", src).build()
}

#[test]
fn carrier_nature_through_array_and_flip() {
    let src = "\
module M(a: Electrical, b: [Bit; 4], c: flip Electrical, d: [flip Bit; 2], e: f64, f: Nope) {
    signal s: Electrical;
    signal t: [[Bit; 2]; 3];
    signal u: u8;
}
";
    let design = with_prelude(src);
    let m = design.module("M").unwrap();
    let nature = |name: &str| design.nature(name).unwrap();
    let port = |name: &str| m.port(name).unwrap().nature();
    assert_eq!(port("a"), Some(nature("Electrical")));
    assert_eq!(port("b"), Some(nature("Bit")));
    assert_eq!(port("c"), Some(nature("Electrical")));
    assert_eq!(port("d"), Some(nature("Bit")));
    assert_eq!(port("e"), None);
    assert_eq!(port("f"), None);
    assert_eq!(m.signal("s").unwrap().nature(), Some(nature("Electrical")));
    assert_eq!(m.signal("t").unwrap().nature(), Some(nature("Bit")));
    assert_eq!(m.signal("u").unwrap().nature(), None);
}

#[test]
fn nature_impls_lists_its_impl_blocks() {
    let src = "impl Electrical { pub fn one(self) -> f64 { 1.0 } }\nimpl Electrical { pub fn two(self) -> f64 { 2.0 } }\n";
    let design = with_prelude(src);
    assert_eq!(design.nature("Electrical").unwrap().impls().len(), 2);
    assert_eq!(design.nature("Bit").unwrap().impls().len(), 1);
    assert!(design.nature("Wire").unwrap().impls().is_empty());
}

const NAMES: &str = "\
mod a {
    pub struct P { x: f64 }
    pub fn helper() -> f64 { 1.0 }
    module M(x: Bit, y: Bit) {
        signal w: Bit;
        if true { signal g: Bit; }
    }
    mod b { pub fn deep() -> f64 { 2.0 } }
}
module Top(p: Bit) { signal s: Bit; }
fn top_level() -> f64 { 3.0 }
struct Q { y: f64 }
";

#[test]
fn lookups_by_name_and_path() {
    let design = with_prelude(NAMES);
    assert_eq!(design.module("M").unwrap().name(), Some("M"));
    assert_eq!(design.module("Top").unwrap().name(), Some("Top"));
    assert!(design.module("Nope").is_none());
    assert!(design.module("a").is_none(), "a namespace is no module");
    assert_eq!(design.nature("Electrical").unwrap().name(), Some("Electrical"));
    assert!(design.nature("M").is_none());
    assert_eq!(design.find("a::M").unwrap().as_module(), design.module("M"));
    assert_eq!(design.find("a::b::deep").unwrap().name(), Some("deep"));
    assert!(design.find("a::nope").is_none());
    assert!(design.find("nope").is_none());
    let names = |v: Vec<Option<&str>>| v.into_iter().flatten().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(names(design.modules().iter().filter(|m| !m.file().is_prelude()).map(|m| m.name()).collect()), ["M", "Top"]);
    let functions: Vec<_> = design.functions().iter().filter_map(|f| f.name()).collect();
    assert_eq!(functions, ["helper", "deep", "top_level"], "non-prelude free functions in source order");
    let structs: Vec<_> = design.structs().iter().filter_map(|s| s.name()).collect();
    assert_eq!(structs, ["P", "Q"]);
    let spaces: Vec<_> = design.namespaces().iter().filter_map(|n| n.name()).collect();
    assert_eq!(spaces, ["a", "b"], "named namespaces; file roots are unnamed");
}

#[test]
fn module_lookup_prefers_authored_files() {
    // The prelude declares `idt`; a design module of that name wins, else the prelude's is found.
    let design = with_prelude("module idt() {}\n");
    assert!(!design.module("idt").unwrap().file().is_prelude());
    let design = with_prelude("module Top() {}\n");
    assert!(design.module("idt").unwrap().file().is_prelude());
}

#[test]
fn qualified_names() {
    let design = with_prelude(NAMES);
    let q = |d: Decl<'_>| d.qualified_name();
    assert_eq!(q(design.module("M").unwrap().decl()), "a::M");
    assert_eq!(q(design.module("M").unwrap().signal("w").unwrap().decl()), "a::M.w");
    assert_eq!(q(design.module("M").unwrap().port("y").unwrap().decl()), "a::M.y");
    assert_eq!(q(design.module("Top").unwrap().decl()), "Top");
    assert_eq!(q(design.find("a::b::deep").unwrap()), "a::b::deep");
    assert_eq!(q(design.find("a::b").unwrap()), "a::b");
    assert_eq!(q(design.root()), "");
    // A generation adds no segment.
    let g = design.module("M").unwrap().member("g");
    let g = g.or_else(|| design.module("M").unwrap().items().into_iter().find(|d| d.name() == Some("g")));
    let g = g.or_else(|| design.module("M").unwrap().signals().into_iter().find(|s| s.name() == Some("g")).map(|s| s.decl())).expect("signal g");
    assert_eq!(q(g), "a::M.g");
    assert_eq!(design.module("M").unwrap().qualified_name(), "a::M");
}

#[test]
fn casts_match_kind_only() {
    let design = with_prelude(NAMES);
    let count = |d: Decl<'_>| {
        [
            d.as_namespace().is_some(),
            d.as_use().is_some(),
            d.as_const().is_some(),
            d.as_function().is_some(),
            d.as_fn_param().is_some(),
            d.as_struct().is_some(),
            d.as_field().is_some(),
            d.as_enum().is_some(),
            d.as_variant().is_some(),
            d.as_trait().is_some(),
            d.as_impl().is_some(),
            d.as_generic_param().is_some(),
            d.as_nature().is_some(),
            d.as_quantity().is_some(),
            d.as_module().is_some(),
            d.as_port().is_some(),
            d.as_param().is_some(),
            d.as_signal().is_some(),
            d.as_let().is_some(),
            d.as_binding().is_some(),
            d.as_definition().is_some(),
            d.as_generation().is_some(),
            d.as_behavior().is_some(),
            d.as_reaction().is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count()
    };
    let mut seen = 0;
    for d in design.decls() {
        assert_eq!(count(d), 1, "{d:?} answers exactly one cast");
        seen += 1;
    }
    assert!(seen > 20);
    assert!(design.find("a::M").unwrap().as_signal().is_none());
    assert!(design.find("a::M").unwrap().as_module().is_some());
    assert!(design.module("M").unwrap().signal("w").unwrap().decl().as_module().is_none());
}

#[test]
fn instance_connection_by_port() {
    let src = "\
module Resistor(p: Electrical, n: Electrical) {}
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let r1 = Resistor(a, b);
    let r2 = Resistor(a, n: b);
    let r3 = Unknown(a, b);
}
";
    let design = with_prelude(src);
    let res = design.module("Resistor").unwrap();
    let top = design.module("Top").unwrap();
    let (p, n) = (res.port("p").unwrap(), res.port("n").unwrap());
    let r1 = top.instance("r1").unwrap();
    assert_eq!(r1.connection(p).unwrap().text(), "a");
    assert_eq!(r1.connection(n).unwrap().text(), "b");
    let r2 = top.instance("r2").unwrap();
    assert_eq!(r2.connection(p).unwrap().text(), "a");
    assert_eq!(r2.connection(n).unwrap().text(), "b");
    // A port of another module, and an instance whose module is unknown, give none.
    let other = design.module("Top").unwrap();
    assert!(other.ports().is_empty());
    let q = design.module("Resistor").unwrap().port("p").unwrap();
    let r3 = top.instance("r3");
    assert!(r3.is_none_or(|r| r.connection(q).is_none()));
}

#[test]
fn handle_equality() {
    let a = with_prelude(NAMES);
    let b = with_prelude(NAMES);
    assert_eq!(a.module("M").unwrap(), a.module("M").unwrap());
    assert_ne!(a.module("M").unwrap(), b.module("M").unwrap(), "same node of another design");
    assert_ne!(a.module("M").unwrap().decl(), a.module("Top").unwrap().decl());
    assert_eq!(a.module("M").unwrap().decl(), a.find("a::M").unwrap());
    assert_eq!(a.module("M").unwrap().signal("w").unwrap(), a.module("M").unwrap().signal("w").unwrap());
}
