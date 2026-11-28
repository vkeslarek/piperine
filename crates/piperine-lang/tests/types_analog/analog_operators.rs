//! Scope: the type rows of the analog operators of `analog-operators` (AO-01 to AO-09): `ddx`
//! with `E0451` and `E0450`, and the rows of `delay`, `noise` and `simparam` (T23). Every source
//! is `Top` with two `E` nodes and an `analog` block under test.



use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_lang::types::Types;
use piperine_lang_fixtures::elaboration::SourceElaboration;
use piperine_lang_fixtures::pom::PomSource;

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// The sources of the tests and how each is checked.
struct Sources;

impl Sources {
    /// The source of a module `Top(a: E, b: E)` with `items` before it and `body` as its `analog` block.
    fn source(items: &str, body: &str) -> String {
        format!("{HEAD}{items}\nmodule Top(a: E, b: E) {{ }}\nanalog for Top {{ {body} }}")
    }

    /// The design of `src`, which must parse and resolve.
    fn design_of(src: &str) -> Design {
        let design = PomSource::build(src);
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", PomSource::codes(&design));
        design
    }

    /// The diagnostics of checking `src`'s `Top`; empty when it is clean.
    fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
        let design = Self::design_of(src);
        match Types::check(&design, "Top") {
            Ok(_) => Vec::new(),
            Err(d) => d,
        }
    }

    /// The codes of `diagnostics`, in order.
    fn codes_of(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
        diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The text the primary span of `d` covers.
    fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
        &src[d.diagnostic.span().range()]
    }
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
        let src = Sources::source("", body);
        assert_eq!(Sources::codes_of(&Sources::diagnostics(&src)), Vec::<String>::new(), "{body}");
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
        let src = Sources::source("", body);
        let d = Sources::diagnostics(&src);
        assert_eq!(Sources::codes_of(&d), ["E0451"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(Sources::snippet(&src, &d[0]), at, "{body}");
    }
}

#[test]
fn ddx_outside_analog_or_read_by_a_definition_is_e0450() {
    // a `fn`, a module body `let` read by a definition, and a param default are not `analog`
    let in_fn = format!("{HEAD}fn f(x: f64) -> f64 {{ ddx(x, v(gnd)) }}\nmodule Top(a: E, b: E) {{ }}\nanalog for Top {{ let y = f(1.0); }}");
    let d = Sources::diagnostics(&in_fn);
    assert!(Sources::codes_of(&d).iter().any(|c| c == "E0450"), "{:?}", Sources::codes_of(&d));
    let in_body = format!("{HEAD}module Top(a: E, b: E) {{ let y = ddx(v(a), v(a)); }}");
    let d = Sources::diagnostics(&in_body);
    assert!(Sources::codes_of(&d).iter().any(|c| c == "E0450"), "{:?}", Sources::codes_of(&d));
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
        "let x: Option<f64> = simparam(\"gmin\");",
        "let x: f64 = simparam(\"gmin\").unwrap_or(1e-12);",
    ] {
        let src = Sources::source("", body);
        assert_eq!(Sources::codes_of(&Sources::diagnostics(&src)), Vec::<String>::new(), "{body}");
    }
    // `simparam` is allowed where a definition reads: a module body `let` is no `E0450`.
    let in_body = format!("{HEAD}module Top(a: E, b: E) {{ let g = simparam(\"gmin\").unwrap_or(1e-12); }}");
    assert_eq!(Sources::codes_of(&Sources::diagnostics(&in_body)), Vec::<String>::new());
    // `delay` and `noise` are core modules: a `fn` is `E0350`, as for `ddt`.
    for form in ["delay(x, 1n)", "noise(x)"] {
        let src = format!("{HEAD}fn f(x: f64) -> f64 {{ {form} }}\nmodule Top(a: E, b: E) {{ }}\nanalog for Top {{ let y = f(1.0); }}");
        assert!(Sources::codes_of(&Sources::diagnostics(&src)).iter().any(|c| c == "E0350"), "{form}: {:?}", Sources::codes_of(&Sources::diagnostics(&src)));
    }
    // the arguments are typed: a text where an `f64` goes, and a number where the name goes
    assert_eq!(Sources::codes_of(&Sources::diagnostics(&Sources::source("", "let x = delay(\"v\", 1n);"))).len(), 1);
    assert_eq!(Sources::codes_of(&Sources::diagnostics(&Sources::source("", "let x = simparam(1.0);"))).len(), 1);
    // `delay` lowers since T33, `noise` since T34 and `simparam` since T35: `elab_operators`
}

#[test]
fn a_value_of_another_type_given_to_delay_or_noise_is_a_type_mismatch() {
    // §14.6 and §14.8 take `f64` inputs; a value of another type (a `bool`, a text) is the checker's plain
    // type mismatch, `E0180`, at the argument. `E0150`, the old "an unknown value reaches `analog`", is retired
    // with the explicit model (explicit-unknowns EU-02): there is no flag to check, only a type.
    for (body, at) in [
        ("let x = delay(true, 1n);", "true"),
        ("let x = delay(v(a), true);", "true"),
        ("let x = delay(v(a), 1n, true);", "true"),
        ("let x = noise(true);", "true"),
        ("let x = noise(\"v\");", "\"v\""),
    ] {
        let src = Sources::source("", body);
        let d = Sources::diagnostics(&src);
        assert_eq!(Sources::codes_of(&d), ["E0180"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(Sources::snippet(&src, &d[0]), at, "{body}");
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
        let design = SourceElaboration::design(&src);
        let d = Types::check(&design, "Top").expect_err(&src);
        assert_eq!(Sources::codes_of(&d), ["E0603"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(Sources::snippet(&src, &d[0]), at, "{src}");
    }
    let trace = "module Top(p: Electrical) { }\nanalog for Top { emit!(\"trace\", vp = v(p)); }";
    let design = SourceElaboration::design(trace);
    assert!(Types::check(&design, "Top").is_ok(), "a trace on another stream types (T37 lowers it)");
}
