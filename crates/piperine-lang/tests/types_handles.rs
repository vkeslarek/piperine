//! Scope: handles in the checker (generics GN-11, GN-12, part I §3.1.5): a function parameter of a nature type, and
//! `self` of an `impl` on a nature, is read, probed, passed on and has carrier methods called on it; a probe of
//! anything else in a function keeps `E0450`.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use piperine_lang::pom::{DeclId, DeclKind, Design};
use piperine_lang::types::Types;

/// MSL sources over the standard library, checked from `Top`.
struct HandleSources;

impl HandleSources {
    /// The `(code, message)` pairs of the design and of the check of `Top` of `src`.
    fn found(src: &str) -> Vec<(String, String)> {
        let design = Design::builder().source("top.msl", src).build();
        let mut found: Vec<(String, String)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
        if let Err(refused) = Types::check(&design, "Top") {
            found.extend(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())));
        }
        found
    }
}

#[test]
fn a_handle_is_read_probed_passed_on_and_has_carrier_methods_in_the_checker() {
    // GN-11 AC 1 and 3, §3.1.5: a parameter of a nature type is probed (`v(a)`), passed to another function, and a
    // discrete one is read as its value and has carrier methods (`a.prev()`); `self` of an `impl` on a nature is a handle
    // (`temp(self)`)
    let probed = "fn level(a: Electrical) -> f64 { v(a) }\nfn through(a: Electrical) -> f64 { level(a) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> through(o); }\n";
    assert_eq!(HandleSources::found(probed), []);
    let discrete = "fn held(a: Bit) -> bool { a }\nfn before(a: Bit) -> bool { a.prev() }\nfn both(a: Bit) -> bool { held(a) && before(a) }\nmodule Top() { signal c: Bit = false; signal d: Bit = false; }\ndigital for Top { on c.rise() { d <- both(c); } }\n";
    assert_eq!(HandleSources::found(discrete), []);
    let method = "impl Thermal { fn celsius(self) -> f64 { temp(self) - 273.15 } }\nmodule Top() { signal t: Thermal; signal o: Electrical; }\nanalog for Top { v(o) +> t.celsius(); }\n";
    assert_eq!(HandleSources::found(method), []);
    let design = Design::builder().source("top.msl", probed).build();
    let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{found:?}"));
    let parameter = (0..design.decl_count()).map(|index| DeclId(index as u32)).find(|decl| matches!(design.decl_data(*decl).kind, DeclKind::FnParam(_)) && design.decl_data(*decl).name.as_ref().is_some_and(|name| name.text == "a")).expect("a parameter `a`");
    assert!(types.handle(parameter), "a nature-typed parameter is a handle");
}

#[test]
fn a_probe_of_a_handle_in_a_function_is_not_e0450() {
    // GN-12 AC 3, V3: the `E0450` of a probe in a function leaves for a probe whose operands are handles; a probe of
    // anything else keeps it, and so does a function that is not reached through a handle
    let handle = "fn level(a: Electrical) -> f64 { v(a) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> level(o); }\n";
    assert!(HandleSources::found(handle).iter().all(|(code, _)| code != "E0450"));
    let other = "fn level(a: Electrical) -> f64 { let b = a; potential(gnd) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> level(o); }\n";
    let found = HandleSources::found(other);
    assert!(found.iter().any(|(code, _)| code == "E0450"), "{found:?}");
    let number = "fn level(x: f64) -> f64 { potential(x) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> level(1.0); }\n";
    let found = HandleSources::found(number);
    assert!(found.iter().any(|(code, _)| code == "E0450" || code == "E0180"), "{found:?}");
}
