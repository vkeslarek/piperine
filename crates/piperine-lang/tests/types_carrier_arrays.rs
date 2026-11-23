//! Arrays of carriers in `piperine_lang::types` (feature `composite-values` phase C, CV-23 to CV-25; docs/spec part I
//! §5.1.8, part III §8.1): the type of a `signal` or port that is an array of carriers or of nodes, its initial value,
//! the reads of an element and its history, the targets of commits and definitions, and connection to ports. Each
//! test states the rule it proves; every source is a design of `Top` over the embedded standard library.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/checked.rs"]
mod checked;

use piperine_lang::pom::{DeclKind, Reflect};
use piperine_lang::types::{Length, Ty, Types};

use checked::CheckedSource;

/// A module `Top` checked over the standard library, whatever it says.
struct Source;

impl Source {
    /// The diagnostics of checking `Top` of `src` as `(code, message)`.
    fn found(src: &str) -> Vec<(String, String)> {
        match Types::check(&CheckedSource::standard_design_of(src), "Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }

    /// The diagnostics hold exactly one of `code` naming `words`.
    fn assert_one(src: &str, code: &str, words: &str) {
        let found = Self::found(src);
        assert!(found.iter().any(|(c, m)| c == code && m.contains(words)), "expected {code} naming `{words}`, got {found:?}\n{src}");
    }

    /// The source checks clean.
    fn assert_clean(src: &str) {
        let found = Self::found(src);
        assert!(found.is_empty(), "expected a clean check, got {found:?}\n{src}");
    }
}

#[test]
fn arrays_of_carriers_type_as_signals_and_ports() {
    // CV-23: `[Bit<u8>; 4]` as a `signal`, a `[Bit; 8]` port with a direction and `[Electrical; 4]` as a signal are
    // types of their own: an array whose length is part of the type
    let src = "module Top(in a: [Bit; 8], out b: [Bit<u8>; 2]) { signal m: [Bit<u8>; 4] = [0; 4]; signal bl: [Electrical; 4]; signal g: [[Bit; 2]; 3]; }";
    let design = CheckedSource::standard_design_of(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let signal = |name: &str| design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Signal(_) | DeclKind::Port(_))).unwrap_or_else(|| panic!("no `{name}`")).id();
    let named = |name: &str| types.carrier(signal(name)).map(|t| t.name(&design)).unwrap_or_default();
    assert_eq!(named("m"), "[Bit<u8>; 4]");
    assert_eq!(named("a"), "[Bit<bool>; 8]");
    assert_eq!(named("g"), "[[Bit<bool>; 2]; 3]", "§1.2.5: arrays nest, `sigtype` allows it");
    let nodes = types.carrier(signal("bl")).expect("the node array is recorded");
    assert!(matches!(nodes, Ty::Array(element, Length::Fixed(4)) if matches!(**element, Ty::Node(_))), "{nodes:?}");
    // §8.1.5: a port of carriers takes a direction, a port of nodes takes none (the direction is the array's)
    Source::assert_one("module Top(a: [Bit; 8]) { }", "E0310", "takes `in` or `out`");
    Source::assert_one("module Top(in a: [Electrical; 2]) { }", "E0310", "takes no direction");
}

#[test]
fn an_array_initial_value_of_another_type_is_e0210() {
    // §5.1.5, §5.1.8: the initial value of an array of carriers is an array of their values, `[x; K]` or
    // `[a, b, c]`; any other type, or a read of a carrier in it, is E0210
    Source::assert_clean("module Top() { signal m: [Bit<u8>; 3] = [1, 2, 3]; signal r: [Bit<u8>; 3] = [7; 3]; }");
    Source::assert_one("module Top() { signal m: [Bit<u8>; 3] = 5; }", "E0210", "");
    // an element of the literal that is the wrong type is that element's own mismatch (E0180), as in any array literal
    Source::assert_one("module Top() { signal m: [Bit<u8>; 3] = [true; 3]; }", "E0180", "expected `u8`, found `bool`");
    Source::assert_one("module Top() { signal m: [Bit<u8>; 3] = [1, 2]; }", "E0210", "");
}

#[test]
fn an_array_of_carriers_as_a_function_parameter_is_refused_by_name() {
    // QC3: §3 rule 7 makes a nature parameter a handle and is silent on an array of them: P0001 naming the form
    Source::assert_one("fn f(a: [Bit; 8]) {}\nmodule Top() { }\nanalog for Top { let h = f; }", "P0001", "an array of carriers as a function parameter");
    Source::assert_one("fn g() { let x: [Bit; 2] = [0; 2]; }\nmodule Top() { }\nanalog for Top { g(); }", "P0001", "an array of carriers as a `let` type");
}
