//! Scope: the checker's generic modules (generics GN-13, GN-16, GN-14, part III §8): a generic module typed once at its
//! definition with its type, nature and const parameters opaque, the inference of a placement's arguments, the
//! behavior blocks bound to the module's generic names and const expressions in written types.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use generic_sources::GenericSources;
use piperine_lang::pom::Design;

#[test]
fn a_generic_module_body_is_checked_once_at_its_definition() {
    // GN-13 AC 1 (§0.3): a generic module nothing places is typed with its parameters opaque; a clean one passes, and a
    // method no bound gives is reported where it is written (`E0161` for a missing bound, `E0180` for a mismatch)
    let clean = "module Hold<T: Repr + Default>(in d: Bit<T>) { signal q: Bit<T>; }\n\
                 module Ladder<const N: usize>(a: E) where N >= 1 { signal rails: [E; N]; }\n\
                 module Short<S: Nature>(a: S, b: S) { }\n\
                 analog<S> for Short<S> { potential(a, b) +> 0.0; }\n";
    assert!(GenericSources::standard_codes(clean).is_empty(), "{:?}", GenericSources::standard_found(clean).iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
    let wrong = "module Hold<T>(in d: Bit<T>) { }\nanalog<T> for Hold<T> { let x: bool = 1.0; }\n";
    let codes = GenericSources::standard_codes(wrong);
    assert!(codes.iter().any(|code| code == "E0180") && codes.iter().all(|code| code != "P0001"), "{codes:?}");
    // §3.8.6: an operator the bounds do not give is refused at the definition, not at a placement
    let unbounded = "module Add<T: Repr>(in d: Bit<T>) { }\nanalog<T> for Add<T> { let x = d + d; }\n";
    let codes = GenericSources::standard_codes(unbounded);
    assert!(!codes.is_empty() && codes.iter().all(|code| code != "P0001"), "{codes:?}");
}

/// What the placement tests read: the codes and messages of the check of a source over the standard library.
struct Placements;

impl Placements {
    /// The codes and messages the whole-model check gives for `src`.
    fn found(src: &str) -> Vec<(String, String)> {
        GenericSources::standard_found(src).iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }
}

#[test]
fn generic_module_arguments_are_explicit_or_inferred() {
    // GN-13 AC 1, GN-16 AC 2 (§3.8.5, §3.8.7): turbofish arguments, else inference from the connected carriers (a nature
    // from the nodes, a type from a `Bit<T>`, a const from an array's length), then the defaults
    let items = "module Short<S: Nature>(a: S, b: S) { }\n\
                 module Hold<T: Repr>(in d: Bit<T>) { }\n\
                 module Rail<const N: usize>(a: [Electrical; N]) { }\n\
                 module Idle<S: Nature = Electrical>(x: f64) { }\n";
    let clean = format!("{items}module Top() {{ signal a: Electrical; signal b: Electrical; signal d: Bit<bool> = false; signal rails: [Electrical; 3];\n\
                        let s = Short(a, b); let h = Hold(d); let r = Rail(rails); let i = Idle(1.0);\n\
                        let t = Hold::<bool>(d); let u = Rail::<3>(rails); }}\n");
    assert_eq!(Placements::found(&clean), vec![], "every argument is explicit or inferred");
    // E0181: a parameter nothing gives (no default, no connection that names it)
    let open = format!("{items}module Top() {{ let r = Short(gnd, gnd); }}\n");
    let found = Placements::found(&open);
    assert!(found.iter().any(|(code, message)| code == "E0181" && message.contains("`S`")), "{found:?}");
    // E0187: a count and a bound
    let counted = format!("{items}module Top() {{ signal d: Bit<bool> = false; let h = Hold::<bool, bool>(d); }}\n");
    assert!(Placements::found(&counted).iter().any(|(code, _)| code == "E0187"));
    let unbound = format!("{items}module Top() {{ signal d: Bit<bool> = false; let h = Hold::<Electrical>(d); }}\n");
    assert!(Placements::found(&unbound).iter().any(|(code, _)| code == "E0187"), "{:?}", Placements::found(&unbound));
}

#[test]
fn two_natures_for_one_nature_parameter_are_e0180_naming_both() {
    // GN-16 AC 2 (DC4): `Short(a_el, b_th)` gives `S` two natures, so the inference fails as a type mismatch naming both
    let src = "module Short<S: Nature>(a: S, b: S) { }\n\
               module Top() { signal a: Electrical; signal b: Thermal; let s = Short(a, b); }\n";
    let found = Placements::found(src);
    let mismatch = found.iter().find(|(code, _)| code == "E0180").unwrap_or_else(|| panic!("{found:?}"));
    assert!(mismatch.1.contains("Electrical") && mismatch.1.contains("Thermal"), "{mismatch:?}");
}

#[test]
fn a_behavior_block_binds_its_modules_generic_names() {
    // GN-13 AC 2 (§8): inside `analog<S> for idt<S>` and `digital<T> for Setup<T>` the module's names are in scope with the
    // module's types: `signal y: S` is a node of the nature parameter, `Bit<T>` a carrier of the type parameter
    let analog = "module Integ<S: Nature>(x: f64) -> f64 { signal y: S; potential(y) }\n\
                  analog<S> for Integ<S> { flow(y) +> ddt(potential(y)) - x; }\n";
    assert_eq!(Placements::found(analog), vec![], "the nature parameter is read inside the block");
    let digital = "module Latch<T: Repr + Default>(in clk: Bit, in d: Bit<T>) { signal q: Bit<T>; }\n\
                   digital<T> for Latch<T> { on clk.rise() { q <- d; } }\n";
    assert_eq!(Placements::found(digital), vec![], "the type parameter is read inside the block");
    // a body that treats `T` as a type it is not is the mismatch, at the definition
    let wrong = "module Latch<T: Repr + Default>(in clk: Bit, in d: Bit<T>) { signal q: Bit<T>; }\n\
                 digital<T> for Latch<T> { on clk.rise() { q <- true; } }\n";
    assert!(Placements::found(wrong).iter().any(|(code, message)| code == "E0432" && message.contains("expected `T`, found `bool`")), "{:?}", Placements::found(wrong));
}

#[test]
fn behavior_names_that_differ_are_e0403_naming_both_lists() {
    // GN-13 AC 3 (§8): a block whose list differs from the module's in name, number or order is `E0403` naming both lists
    let cases = [("analog<T> for Short<T> { }", "<T>", "<S>"), ("analog<S, U> for Short<S, U> { }", "<S, U>", "<S>"), ("analog<B, A> for Pair<B, A> { }", "<B, A>", "<A, B>")];
    for (block, written, declared) in cases {
        let src = format!("module Short<S: Nature>(a: S, b: S) {{ }}\nmodule Pair<A: Nature, B: Nature>(a: A, b: B) {{ }}\n{block}\n");
        let design = Design::builder().source("top.msl", format!("{}{src}", GenericSources::HEAD)).build();
        let found: Vec<_> = design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0403").collect();
        assert_eq!(found.len(), 1, "{block}: {:?}", support::codes(&design));
        let message = &found[0].diagnostic.message;
        assert!(message.contains(&format!("`{written}`")) && message.contains(&format!("`{declared}`")), "{block}: {message}");
    }
}

#[test]
fn const_expressions_in_written_types_are_accepted_everywhere() {
    // GN-14 AC 1 (§1.2.5, §3.8.5): a const expression in a written type is accepted outside a generic function too: an
    // integer width in a generic function's signature, an array length in a generic module's items; none is `P0001`
    let clean = "fn widen<const A: usize>(x: UInt<{A + 1}>) -> UInt<{A + 1}> { x }\n\
                 module Taps<const N: usize, const D: usize>(a: [Electrical; N]) where D >= 1 {\n\
                     signal rails: [Electrical; N + 1];\n\
                     signal states: [State; D - 1];\n\
                 }\n";
    assert_eq!(Placements::found(clean), vec![], "const expressions in written types");
    // a width that no const expression makes valid is the width error, not an unsupported form
    let negative = "fn bad(x: Int<-1>) -> bool { true }\n";
    let found = Placements::found(negative);
    assert!(!found.is_empty() && found.iter().all(|(code, _)| code != "P0001"), "{found:?}");
}
