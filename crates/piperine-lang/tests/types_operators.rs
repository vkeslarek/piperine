//! Scope: the type rows of the analog operators of `analog-operators` (AO-01 to AO-09): `ddx`
//! with `E0451` and `E0450`, and the rows of `delay`, `noise` and `simparam` (T23). Every source
//! is `Top` with two `E` nodes and an `analog` block under test.

#[path = "support/pom.rs"]
mod support;

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod elaborated;

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

#[test]
fn delay_noise_and_simparam_type_and_wait_for_lowering() {
    // AO-02, AO-03, AO-07: each form types as an `f64` in `analog`, with its optional argument
    // left out, given, or `None`; the elaborator refuses it with `P0001` until its lowering task.
    for body in [
        "let x: f64 = delay(v(a), 1n);",
        "let x: f64 = delay(v(a), 1n, 2n);",
        "let x: f64 = delay(v(a), 1n, None);",
        "let x: f64 = noise(1e-18);",
        "let x: f64 = noise(1e-18, \"thermal\");",
        "let x: f64 = noise(1e-18, None);",
        "let x: f64 = simparam(\"gmin\");",
        "let x: f64 = simparam(\"gmin\", 1e-12);",
        "let x: f64 = simparam(\"gmin\", None);",
    ] {
        let src = source("", body);
        assert_eq!(codes_of(&diagnostics(&src)), Vec::<String>::new(), "{body}");
    }
    // `simparam` is allowed where a definition reads: a module body `let` is no `E0450`.
    let in_body = format!("{HEAD}module Top(a: E, b: E) {{ let g = simparam(\"gmin\", 1e-12); }}");
    assert_eq!(codes_of(&diagnostics(&in_body)), Vec::<String>::new());
    // `delay` and `noise` are core modules: a `fn` is `E0350`, as for `ddt`.
    for form in ["delay(x, 1n)", "noise(x)"] {
        let src = format!("{HEAD}fn f(x: f64) -> f64 {{ {form} }}\nmodule Top(a: E, b: E) {{ }}\nanalog for Top {{ let y = f(1.0); }}");
        assert!(codes_of(&diagnostics(&src)).iter().any(|c| c == "E0350"), "{form}: {:?}", codes_of(&diagnostics(&src)));
    }
    // the arguments are typed: a text where an `f64` goes, and a number where the name goes
    assert_eq!(codes_of(&diagnostics(&source("", "let x = delay(\"v\", 1n);"))).len(), 1);
    assert_eq!(codes_of(&diagnostics(&source("", "let x = simparam(1.0);"))).len(), 1);
    // the elaborator still refuses each with a `P0001` naming it (`delay` lowers since T33 and `noise` since T34: `elab_operators`)
    let elaborated_src = "module Top(a: Electrical, b: Electrical) { }\nanalog for Top { i(a, b) +> V; }";
    for (form, name) in [("simparam(\"gmin\", 1e-12)", "simparam")] {
        let src = elaborated_src.replace("V", form);
        let err = elaborated::Elab::run(&src, "Top").expect_err(name);
        assert_eq!(elaborated::Elab::codes(&err), ["P0001"], "{name}: {:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert!(err[0].diagnostic.message.contains(name), "{name}: {}", err[0].diagnostic.message);
    }
}

#[test]
fn a_site_reached_from_analog_writing_sim_or_checks_is_e0603() {
    // AO-09 AC 5 (§19 rules 4 and 14): `E0603` is unchanged by the trace: a site reached from
    // `analog` that writes `sim` or `checks` is refused, written there or through a `fn`; a trace
    // on another stream is no `E0603`.
    let items = "fn guard(ok: bool) { require(\"g\", ok); }\n";
    for (src, at) in [
        (format!("module Top(p: Electrical) {{ }}\nanalog for Top {{ require(\"n\", v(p) < 1.0); }}"), "require(\"n\", v(p) < 1.0)"),
        (format!("{items}module Top(p: Electrical) {{ }}\nanalog for Top {{ guard(v(p) < 1.0); }}"), "guard(v(p) < 1.0)"),
        (format!("module Top(p: Electrical) {{ }}\nanalog for Top {{ emit!(\"sim\", finish = true); }}"), "emit!(\"sim\", finish = true)"),
        (format!("module Top(p: Electrical) {{ }}\nanalog for Top {{ emit!(\"checks\", pass = true); }}"), "emit!(\"checks\", pass = true)"),
    ] {
        let design = elaborated::Elab::design(&src);
        let d = Types::check(&design, "Top").expect_err(&src);
        assert_eq!(codes_of(&d), ["E0603"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{src}");
    }
    let trace = "module Top(p: Electrical) { }\nanalog for Top { emit!(\"trace\", vp = v(p)); }";
    let design = elaborated::Elab::design(trace);
    // until T37 lowers a trace the checker refuses it by name, one `P0001`, never an `E0603`
    let d = Types::check(&design, "Top").expect_err("a trace is refused until T37");
    assert_eq!(codes_of(&d), ["P0001"], "{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}
