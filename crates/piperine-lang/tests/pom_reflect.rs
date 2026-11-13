//! Scope: the reflection queries over a resolved design (POM-11): the lookups
//! and casts, the carrier nature of a port or signal, the instance views and
//! the reverse queries.

use piperine_lang::pom::Design;

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
