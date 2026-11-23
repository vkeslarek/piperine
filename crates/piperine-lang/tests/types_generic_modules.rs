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
use piperine_lang::types::UnfinishedForm;

/// The whole-model check with generic modules admitted: the form is still behind the unfinished-forms gate.
struct Admitted;

impl Admitted {
    /// The diagnostics of the check of `src`.
    fn found(src: &str) -> Vec<piperine_lang::pom::DesignDiagnostic> {
        GenericSources::admitting_found(src, &[UnfinishedForm::GenericModules])
    }

    /// The codes of [`Admitted::found`].
    fn codes(src: &str) -> Vec<String> {
        GenericSources::codes_of(&Self::found(src))
    }

    /// The codes and messages of [`Admitted::found`].
    fn pairs(src: &str) -> Vec<(String, String)> {
        Self::found(src).iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }
}

#[test]
fn a_generic_module_body_is_checked_once_at_its_definition() {
    // GN-13 AC 1 (§0.3): a generic module nothing places is typed with its parameters opaque; a clean one passes, and a
    // method no bound gives is reported where it is written (`E0161` for a missing bound, `E0180` for a mismatch)
    let clean = "module Hold<T: Repr + Default>(in d: Bit<T>) { signal q: Bit<T>; }\n\
                 module Ladder<const N: usize>(a: E) where N >= 1 { signal rails: [E; N]; }\n\
                 module Short<S: Nature>(a: S, b: S) { }\n\
                 analog<S> for Short<S> { potential(a, b) +> 0.0; }\n";
    assert!(Admitted::codes(clean).is_empty(), "{:?}", Admitted::found(clean).iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
    let wrong = "module Hold<T>(in d: Bit<T>) { }\nanalog<T> for Hold<T> { let x: bool = 1.0; }\n";
    let codes = Admitted::codes(wrong);
    assert!(codes.iter().any(|code| code == "E0180") && codes.iter().all(|code| code != "P0001"), "{codes:?}");
    // §3.8.6: an operator the bounds do not give is refused at the definition, not at a placement
    let unbounded = "module Add<T: Repr>(in d: Bit<T>) { }\nanalog<T> for Add<T> { let x = d + d; }\n";
    let codes = Admitted::codes(unbounded);
    assert!(!codes.is_empty() && codes.iter().all(|code| code != "P0001"), "{codes:?}");
}

#[test]
fn a_param_of_a_type_parameter_is_typed_under_its_bounds() {
    // GN-13 AC 1 (§8.1.15): `reset: T` is a value of the module's type parameter, its default and `where` checked
    // over it; a param of a template type (`C: Fn(Cell)`) stays `P0001` naming it (phase D)
    let clean = "module Dff<T: Repr + Default>(in d: Bit<T>) { reset: T = T::default(); }\n";
    assert_eq!(Admitted::pairs(clean), vec![], "a param of T with a default over T");
    let wrong = "module Dff<T: Repr + Default>(in d: Bit<T>) { reset: T = true; }\n";
    assert!(Admitted::codes(wrong).iter().any(|code| code == "E0432" || code == "E0180"), "{:?}", Admitted::pairs(wrong));
    let template = "module Host<C: Fn(f64)>(x: f64) { cell: C; }\n";
    let found = Admitted::pairs(template);
    assert!(found.iter().any(|(code, message)| code == "P0001" && message.contains("param of type `C`")), "{found:?}");
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
    assert_eq!(Admitted::pairs(&clean), vec![], "every argument is explicit or inferred");
    // E0181: a parameter nothing gives (no default, no connection that names it)
    let open = format!("{items}module Top() {{ let r = Short(gnd, gnd); }}\n");
    let found = Admitted::pairs(&open);
    assert!(found.iter().any(|(code, message)| code == "E0181" && message.contains("`S`")), "{found:?}");
    // E0187: a count and a bound
    let counted = format!("{items}module Top() {{ signal d: Bit<bool> = false; let h = Hold::<bool, bool>(d); }}\n");
    assert!(Admitted::pairs(&counted).iter().any(|(code, _)| code == "E0187"));
    let unbound = format!("{items}module Top() {{ signal d: Bit<bool> = false; let h = Hold::<Electrical>(d); }}\n");
    assert!(Admitted::pairs(&unbound).iter().any(|(code, _)| code == "E0187"), "{:?}", Admitted::pairs(&unbound));
}

#[test]
fn two_natures_for_one_nature_parameter_are_e0180_naming_both() {
    // GN-16 AC 2 (DC4): `Short(a_el, b_th)` gives `S` two natures, so the inference fails as a type mismatch naming both
    let src = "module Short<S: Nature>(a: S, b: S) { }\n\
               module Top() { signal a: Electrical; signal b: Thermal; let s = Short(a, b); }\n";
    let found = Admitted::pairs(src);
    let mismatch = found.iter().find(|(code, _)| code == "E0180").unwrap_or_else(|| panic!("{found:?}"));
    assert!(mismatch.1.contains("Electrical") && mismatch.1.contains("Thermal"), "{mismatch:?}");
}


#[test]
fn an_underscore_generic_argument_is_inferred() {
    // GN-13 AC 1 (§1.2.12, spec index row 22): `_` in the generic arguments of a placement is as if left out
    let items = "module Short<S: Nature>(a: S, b: S) { }\nmodule Rail<const N: usize>(a: [Electrical; N]) { }\n";
    let clean = format!("{items}module Top() {{ signal a: Electrical; signal b: Electrical; signal rails: [Electrical; 3]; let s = Short::<_>(a, b); let r = Rail::<_>(rails); }}\n");
    assert_eq!(Admitted::pairs(&clean), vec![], "`_` is inferred from the connections");
    let open = format!("{items}module Top() {{ let s = Short::<_>(gnd, gnd); }}\n");
    let found = Admitted::pairs(&open);
    assert!(found.iter().any(|(code, message)| code == "E0181" && message.contains("`S`")), "{found:?}");
}

#[test]
fn a_generic_placement_feeds_back_through_its_unknown() {
    // GN-16 AC 2 (§8.3.5): `let y = idt((x - y) / tau)` reads its own result, legal because `idt`'s result reads only its
    // node; a generic placement is pushed on `placing` like any other, so the feedback rule runs (no `E0332`)
    let items = "module Integ<S: Nature>(x: f64) -> f64 { signal n: S; potential(n) }\n\
                 analog<S> for Integ<S> { flow(n) +> ddt(potential(n)) - x; }\n";
    let clean = format!("{items}module Lowpass(input: f64, o: Electrical) {{ let y = Integ::<Electrical>(input - y); }}\n");
    let found = Admitted::pairs(&clean);
    assert!(found.iter().all(|(code, _)| code != "E0332"), "{found:?}");
    // the Book's `lowpass` over the standard library's `idt`
    let book = "module lowpass(x: f64, tau: f64) -> f64 { let y = idt((x - y) / tau); y }\n";
    let found = Admitted::pairs(book);
    assert!(found.iter().all(|(code, _)| code != "E0332"), "{found:?}");
}

#[test]
fn a_carrier_handed_to_a_generic_out_port_needs_no_initial_value() {
    // GN-16 AC 2 (§3.3.4): the carrier a generic placement's `out` port drives needs no initial value of its own, and a
    // carrier nothing drives still does (`bool` has no `Default`)
    let items = "module Drive<S: Nature>(a: S, out q: Bit = false) { }\n";
    let handed = format!("{items}module Top() {{ signal a: Electrical; signal s: Bit; let d = Drive::<Electrical>(a, s); }}\n");
    assert_eq!(Admitted::pairs(&handed), vec![], "handed to an `out` port, the carrier needs no initial value");
    let alone = format!("{items}module Top() {{ signal s: Bit; }}\n");
    assert!(Admitted::pairs(&alone).iter().any(|(code, _)| code == "E0187"), "{:?}", Admitted::pairs(&alone));
}

#[test]
fn a_behavior_block_binds_its_modules_generic_names() {
    // GN-13 AC 2 (§8): inside `analog<S> for idt<S>` and `digital<T> for Setup<T>` the module's names are in scope with the
    // module's types: `signal y: S` is a node of the nature parameter, `Bit<T>` a carrier of the type parameter
    let analog = "module Integ<S: Nature>(x: f64) -> f64 { signal y: S; potential(y) }\n\
                  analog<S> for Integ<S> { flow(y) +> ddt(potential(y)) - x; }\n";
    assert_eq!(Admitted::pairs(analog), vec![], "the nature parameter is read inside the block");
    let digital = "module Latch<T: Repr + Default>(in clk: Bit, in d: Bit<T>) { signal q: Bit<T>; }\n\
                   digital<T> for Latch<T> { on clk.rise() { q <- d; } }\n";
    assert_eq!(Admitted::pairs(digital), vec![], "the type parameter is read inside the block");
    // a body that treats `T` as a type it is not is the mismatch, at the definition
    let wrong = "module Latch<T: Repr + Default>(in clk: Bit, in d: Bit<T>) { signal q: Bit<T>; }\n\
                 digital<T> for Latch<T> { on clk.rise() { q <- true; } }\n";
    assert!(Admitted::pairs(wrong).iter().any(|(code, message)| code == "E0432" && message.contains("expected `T`, found `bool`")), "{:?}", Admitted::pairs(wrong));
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

