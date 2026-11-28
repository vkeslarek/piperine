//! Scope: the run settings of `piperine_lang::types` (generics phase D, GN-24 and GN-25): `temperature()` is an
//! `Option<f64>`, `m` and `dtemp` are ordinary names (no implicit param exists), and a placement that binds a name
//! its module does not declare is `E0320` whatever the name.

use crate::support;

use piperine_lang::pom::{DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// Sources of this target: every one is `HEAD` followed by the items under test, checked from `Top`.
struct RunSources;

impl RunSources {
    /// The design of `items`, which must parse and resolve.
    fn design(items: &str) -> Design {
        let design = support::build(&format!("{HEAD}{items}"));
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{items}", support::codes(&design));
        design
    }

    /// The type of the binding `name` in a clean check of `items`.
    fn local(items: &str, name: &str) -> Ty {
        let design = Self::design(items);
        let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{items}", Self::messages(&d)));
        let decl = design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id();
        types.local(decl).unwrap_or_else(|| panic!("no type for `{name}`")).clone()
    }

    /// The codes of the diagnostics a check of `items` reports.
    fn codes(items: &str) -> Vec<String> {
        let design = Self::design(items);
        Types::check(&design, "Top").err().unwrap_or_default().iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The messages of `diagnostics`, for a failure report.
    fn messages(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
        diagnostics.iter().map(|d| format!("{}: {}", d.diagnostic.code, d.diagnostic.message)).collect()
    }
}

#[test]
fn temperature_is_an_option_of_the_run() {
    // GN-25 AC 1 (§8.5.2, §14.4): `temperature()` is `Option<f64>`; `unwrap_or` gives the `f64` the recommended fallback
    // reads; a const context is still refused (`E0323`, §8.1.21)
    let items = "module Top() { let t = temperature(); let k = temperature().unwrap_or(300.15); }";
    assert_eq!(RunSources::local(items, "t"), Ty::Option(Box::new(Ty::F64)));
    assert_eq!(RunSources::local(items, "k"), Ty::F64);
    assert_eq!(RunSources::codes("module Top() { a: f64 = temperature().unwrap_or(1.0); }"), ["E0323"], "a param default is a const context");
    assert_eq!(RunSources::codes("module Top() { a: f64 = temperature(); }").len(), 1, "an `f64` is not an `Option<f64>`");
}

#[test]
fn m_and_dtemp_are_ordinary_names() {
    // GN-25 AC 3 (§8.5.1): a module declares `m` and `dtemp` like any name; a placement binds them like any param; and
    // one that does not declare `m` refuses the binding as it refuses any unknown param (`E0320`)
    let own = "module Resistor(a: E, b: E) { r: f64 = 1k; m: Option<f64>; dtemp: f64 = 0.0; let count = m.unwrap_or(1.0); }\n\
               module Top() { signal a: E; let r1 = Resistor(a, gnd) { r: 1k, m: Some(4.0), dtemp: 5.0 }; }";
    assert_eq!(RunSources::codes(own), Vec::<String>::new());
    let none = support::build(&format!("{HEAD}module Resistor(a: E, b: E) {{ r: f64 = 1k; }}\nmodule Top() {{ signal a: E; let r1 = Resistor(a, gnd) {{ r: 1k, m: Some(4.0) }}; }}"));
    assert_eq!(support::codes(&none), ["E0320"], "no implicit param exists");
}

/// Sources over the real standard library (`multiplicity()` and the constants live there).
struct StdSources;

impl StdSources {
    /// The type of the binding `name` of `Top` in a clean check of the module `items` over the real `std`.
    fn local(items: &str, name: &str) -> Ty {
        let design = Design::builder().source("top.msl", &format!("{HEAD}{items}")).build();
        assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
        let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{items}", RunSources::messages(&d)));
        let decl = design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id();
        types.local(decl).unwrap_or_else(|| panic!("no type for `{name}`")).clone()
    }

    /// The codes of the diagnostics a check of `Top` over the real `std` reports.
    fn codes(items: &str) -> Vec<String> {
        let design = Design::builder().source("top.msl", &format!("{HEAD}{items}")).build();
        Types::check(&design, "Top").err().unwrap_or_default().iter().map(|d| d.diagnostic.code.to_string()).collect()
    }
}

#[test]
fn multiplicity_is_a_stdlib_function_over_simparam() {
    // GN-24 AC 1, AC 3 (§8.5.2, §22.2.2): `multiplicity()` is an ordinary `Option<f64>` function of `std`; a module
    // combines its own `m` with it in a body `let` (the same read in a param default is `E0321` at elaboration, §8.1.16)
    let items = "module Top() { m: Option<f64>; let run = multiplicity(); let count = m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY); }";
    assert_eq!(StdSources::local(items, "run"), Ty::Option(Box::new(Ty::F64)));
    assert_eq!(StdSources::local(items, "count"), Ty::F64);
    assert_eq!(StdSources::codes(items), Vec::<String>::new(), "the whole module checks clean");
}
