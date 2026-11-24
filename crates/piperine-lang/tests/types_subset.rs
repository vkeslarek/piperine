//! The subset boundary of `piperine_lang::types` (TYP-05): every language construct of the
//! feature's Out of Scope table is refused with one `P0001` that names it, and an item holding
//! such a construct that the top does not reach raises nothing. Each row is one source whose
//! `Top` reaches the construct; the fragment is what the diagnostic must name.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_lang::types::Types;

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
module Resistor(p: E, n: E) { r: f64; }\n";

/// One row: a construct outside the subset, the source that reaches it from `Top` (after
/// `HEAD`), and the words its diagnostic must contain.
struct Row {
    construct: &'static str,
    source: &'static str,
    names: &'static str,
}

// `analysis` of another kind, `bound_step` and `freq` left this table when FIX-09 to FIX-11 typed
// them (`types_analog::freq_analysis_kinds_and_bound_step_type`).
// generation (`for` and `if`, in a body and in a `digital` block) left this table when generics T77 and T80 typed it
// (`types_generation`).
// `settle` left this table when unknown-semantics-events T5 typed it (an `Event`: head positions in
// `types_events::settle_is_an_event_in_every_event_position`, `E0441` elsewhere in
// `types_events::settle_outside_an_event_position_is_e0441`).
const ROWS: &[Row] = &[
    // `struct with an impl` left this table when generics T25 typed methods (`types_traits::methods_associated_functions_and_constants_type_check`).
    Row { construct: "generic function", source: "fn g<T>(x: T) -> T { x }\nmodule Top() { }\nanalog for Top { let h = g; }", names: "generic function" },
    // `bundle` left this table when resolved-nets-bundles T6 typed it (`types_bundles`).
    Row { construct: "template", source: "module Top(a: E, b: E) { let t = Resistor { r: 1.0 }; }", names: "module template" },
    Row { construct: "placement through a template", source: "module Top(a: E, b: E) { let t = Resistor { r: 1.0 }; let r = t(a, b); }", names: "module template" },
    Row { construct: "value input of an unsupported type", source: "module Top(x: (f64, f64)) { }", names: "value input port" },
    Row { construct: "extern module", source: "#[link(\"x.osdi\", model = \"x\")]\nextern \"osdi\" module X(a: E) { }\nmodule Top(a: E) { let x = X(a); }", names: "`extern` module" },
    // `delay`, `noise` and `simparam` left this table when analog-operators T23 typed them (`types_analog_operators`); the elaborator refuses each with `P0001` until its lowering task.
    // `ddx` left this table when analog-operators T22 typed it (`types_analog_operators`); the elaborator refuses it with `P0001` until T32.
    Row { construct: "flow probe read as a value", source: "module Top(a: E) { }\nanalog for Top { let x = i(a); }", names: "flow probe" },
    // `trait` left this table when generics T25 typed a trait's methods (`types_traits::methods_associated_functions_and_constants_type_check`).
];

fn check(row: &Row) -> (String, Vec<DesignDiagnostic>) {
    let src = format!("{HEAD}{}", row.source);
    let design = support::build(&src);
    assert!(design.diagnostics().is_empty(), "{}: the source must parse and resolve: {:?}\n{src}", row.construct, support::codes(&design));
    match Types::check(&design, "Top") {
        Ok(_) => panic!("{}: expected a P0001, the check passed\n{src}", row.construct),
        Err(d) => (src, d),
    }
}

#[test]
fn a_construct_outside_the_subset_is_p0001() {
    for row in ROWS {
        let (src, d) = check(row);
        assert_eq!(d.len(), 1, "{}: one diagnostic per construct, got {:?}\n{src}", row.construct, d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        assert_eq!(d[0].diagnostic.code, "P0001", "{}: {}", row.construct, d[0].diagnostic.message);
        let span = d[0].diagnostic.span();
        assert!(span.end > span.start && span.end <= src.len(), "{}: a span of the construct", row.construct);
    }
}

#[test]
fn p0001_names_the_construct() {
    for row in ROWS {
        let (_, d) = check(row);
        let message = &d[0].diagnostic.message;
        assert!(message.contains(row.names), "{}: `{message}` does not name `{}`", row.construct, row.names);
    }
}

#[test]
fn unreached_items_are_not_reported() {
    // Everything below is outside the subset and nothing reaches it from `Top`.
    let src = format!(
        "{HEAD}\
         struct S {{ a: f64 }}\nenum C {{ A, B }}\ntrait Tr {{ fn get(self) -> f64; }}\nimpl S {{ fn get(self) -> f64 {{ 1.0 }} }}\n\
         const N: u32 = 3;\n\
         fn loops(x: f64) -> f64 {{ let y = x; while y > 0.0 {{ y = y - 1.0; }} match y {{ _ => y }} }}\n\
         fn generic<T>(x: T) -> T {{ x }}\n\
         module Generic<N: Nature>(a: N) {{ }}\n\
         module Digital(in d: bool) -> f64 {{ 1.0 }}\n\
         digital for Digital {{ }}\n\
         module Top(a: E) {{ let r = Resistor {{ r: 1.0 }}(a, gnd); }}"
    );
    let design = support::build(&src);
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    assert_eq!(types.reached().len(), 2, "only Top and Resistor are reached");

    // The same names, reached, are refused.
    let reached = format!("{HEAD}fn loops(x: f64) -> f64 {{ let s = include_str!(\"a.txt\"); x }}\nmodule Top() {{ }}\nanalog for Top {{ let x = loops(1.0); }}");
    let design: Design = support::build(&reached);
    assert!(Types::check(&design, "Top").is_err());
}

/// Constructs that only resolve against the real standard library: `Bit` and `Repr` are
/// refused by name as well.
const STD_ROWS: &[Row] = &[
    // `others()` on a one-driver carrier left this table when resolved-nets T3 typed it (`types_natures::others_on_a_one_driver_carrier_is_e0224`).
    // `element definition` left this table when composite-values T76 typed it (`types_carrier_arrays`).
    // `a resolved nature` left this table when resolved-nets T16 typed it (`types_natures::an_impl_resolve_makes_a_discrete_nature_resolved`); the elaborator still refuses it by name (`elab_unlowered`) until T18.
    Row { construct: "Repr", source: "module Top() { }\nanalog for Top { let x: Repr = 1.0; }", names: "`Repr`" },
];

#[test]
fn std_types_outside_the_subset_are_p0001() {
    for row in STD_ROWS {
        let src = format!("{HEAD}{}", row.source);
        let design = Design::builder().source("top.msl", &src).build();
        assert!(design.diagnostics().is_empty(), "{}: the source must parse and resolve: {:?}\n{src}", row.construct, support::codes(&design));
        let d = Types::check(&design, "Top").expect_err(row.construct);
        assert_eq!(d.len(), 1, "{}: {:?}", row.construct, d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(d[0].diagnostic.code, "P0001", "{}: {}", row.construct, d[0].diagnostic.message);
        assert!(d[0].diagnostic.message.contains(row.names), "{}: {}", row.construct, d[0].diagnostic.message);
    }
}
