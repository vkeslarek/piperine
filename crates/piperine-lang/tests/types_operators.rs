//! Scope: the type rows of the analog operators of `analog-operators` (AO-01 to AO-09): `ddx`
//! with `E0451` and `E0450`, and the rows of `delay`, `noise` and `simparam` (T23). Every source
//! is `Top` with two `E` nodes and an `analog` block under test.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_lang::types::Types;

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// The source of a module `Top(a: E, b: E)` with `items` before it and `body` as its `analog` block.
fn source(items: &str, body: &str) -> String {
    format!("{HEAD}{items}\nmodule Top(a: E, b: E) {{ }}\nanalog for Top {{ {body} }}")
}

/// The design of `src`, which must parse and resolve.
fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

/// The diagnostics of checking `src`'s `Top`; empty when it is clean.
fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
    let design = design_of(src);
    match Types::check(&design, "Top") {
        Ok(_) => Vec::new(),
        Err(d) => d,
    }
}

fn codes_of(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

/// The text the primary span of `d` covers.
fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

#[test]
fn ddx_types_against_a_potential_or_a_flow_probe() {
    // a potential probe, a two-node probe, a flow probe, and `ddx` of `ddx`
    for body in [
        "let x: f64 = ddx(v(a) * v(a), v(a));",
        "let x: f64 = ddx(v(a, b), v(a, b));",
        "let x: f64 = ddx(v(a), i(a, b));",
        "let x: f64 = ddx(ddx(v(a) * v(a), v(a)), v(a));",
    ] {
        let src = source("", body);
        assert_eq!(codes_of(&diagnostics(&src)), Vec::<String>::new(), "{body}");
    }
}

#[test]
fn ddx_against_a_non_probe_is_e0451() {
    // a value, a `p.inflow()`, and a plain number are no probe call (§14.2.2)
    for (body, at) in [
        ("let x = ddx(v(a), 1.0);", "1.0"),
        ("let y = v(a); let x = ddx(v(a), y);", "y"),
        ("let x = ddx(v(a), a.inflow());", "a.inflow()"),
    ] {
        let src = source("", body);
        let d = diagnostics(&src);
        assert_eq!(codes_of(&d), ["E0451"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
}

#[test]
fn ddx_outside_analog_or_read_by_a_definition_is_e0450() {
    // a `fn`, a module body `let` read by a definition, and a param default are not `analog`
    let in_fn = format!("{HEAD}fn f(x: f64) -> f64 {{ ddx(x, v(gnd)) }}\nmodule Top(a: E, b: E) {{ }}\nanalog for Top {{ let y = f(1.0); }}");
    let d = diagnostics(&in_fn);
    assert!(codes_of(&d).iter().any(|c| c == "E0450"), "{:?}", codes_of(&d));
    let in_body = format!("{HEAD}module Top(a: E, b: E) {{ let y = ddx(v(a), v(a)); }}");
    let d = diagnostics(&in_body);
    assert!(codes_of(&d).iter().any(|c| c == "E0450"), "{:?}", codes_of(&d));
}
