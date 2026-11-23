//! Scope: the checker's generic modules (generics GN-13, GN-16, GN-14, part III §8): a generic module typed once at its
//! definition with its type, nature and const parameters opaque, the inference of a placement's arguments, the
//! behavior blocks bound to the module's generic names and const expressions in written types.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use generic_sources::GenericSources;
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
