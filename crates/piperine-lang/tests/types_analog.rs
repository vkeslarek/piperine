//! Analog forms of `piperine_lang::types` (TYP-04): probes through `access` aliases and
//! `potential`, contributions (`+>`), and the core primitives of the subset (`ddt`, `ac_stim`,
//! `limit`, the environment functions), with the codes `E0240`, `E0410`, `E0417` and `E0450`.
//! Every source is `Top` with two `E` nodes, one `T` node and an `analog` block under test.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
nature T { potential: Tp { access: temp, units: \"K\", abstol: 1m }, flow: P { access: pwr, units: \"W\", abstol: 1n } }\n\
module Top(a: E, b: E, t: T) { }\n";

fn source(items: &str, body: &str) -> String {
    format!("{HEAD}{items}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

fn binding(design: &Design, name: &str) -> DeclId {
    design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id()
}

fn local_types(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
    let src = source(items, body);
    let design = design_of(&src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    names.iter().map(|n| types.local(binding(&design, n)).unwrap_or_else(|| panic!("no type for `{n}`")).clone()).collect()
}

fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = source(items, body);
    let design = design_of(&src);
    let err = Types::check(&design, "Top").expect_err("expected diagnostics");
    (src, err)
}

fn codes_of(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

#[test]
fn potential_probes_are_f64() {
    let t = local_types("", "let x = v(a); let y = v(a, b); let z = potential(a, b); let w = potential(t); let u = v(a, gnd) + v(gnd);", &["x", "y", "z", "w", "u"]);
    assert!(t.iter().all(|t| *t == Ty::F64), "{t:?}");

    // A flow probe is a contribution target; read as a value it is outside the subset.
    let (src, d) = diagnostics("", "let x = i(a, b);");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "i(a, b)");
    let src = source("", "i(a, b) +> 1.0; flow(a, b) +> 2.0;");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("flow probes as targets");
}

#[test]
fn probe_nodes_of_two_natures_are_e0240() {
    let (src, d) = diagnostics("", "let x = v(a, t);");
    assert_eq!(codes_of(&d), ["E0240"]);
    assert_eq!(snippet(&src, &d[0]), "v(a, t)");

    let (src, d) = diagnostics("", "potential(a, t) +> 1.0;");
    assert_eq!(codes_of(&d), ["E0240"]);
    assert_eq!(snippet(&src, &d[0]), "potential(a, t)");
}

#[test]
fn a_contribution_target_that_is_no_probe_is_e0410() {
    let (src, d) = diagnostics("", "let x = 1.0; x +> 2.0;");
    assert_eq!(codes_of(&d), ["E0410"]);
    assert_eq!(snippet(&src, &d[0]), "x");

    let (src, d) = diagnostics("", "(v(a) + 1.0) +> 0.0;");
    assert_eq!(codes_of(&d), ["E0410"]);
    assert_eq!(snippet(&src, &d[0]), "(v(a) + 1.0)");

    let src = source("", "v(a, b) +> 1.0; i(a, gnd) +> 2.0;");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("probe targets");
}

#[test]
fn a_contribution_that_is_not_f64_is_e0417() {
    let (src, d) = diagnostics("", "v(a, b) +> true;");
    assert_eq!(codes_of(&d), ["E0417"]);
    assert_eq!(snippet(&src, &d[0]), "true");

    let (src, d) = diagnostics("", "v(a, b) +> 1;");
    assert_eq!(codes_of(&d), ["E0417"]);
    assert_eq!(snippet(&src, &d[0]), "1");
}

#[test]
fn ddt_ac_stim_and_environment_are_f64() {
    let t = local_types(
        "",
        "let d = ddt(v(a)); let d2 = ddt(v(a), 1e-6); let s = ac_stim(1.0); let s2 = ac_stim(1.0, 0.5); \
         let tk = temperature(); let g = gmin(); let sc = source_scale();",
        &["d", "d2", "s", "s2", "tk", "g", "sc"],
    );
    assert!(t.iter().all(|t| *t == Ty::F64), "{t:?}");

    let (src, d) = diagnostics("", "let x = ddt(true);");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    let (_, d) = diagnostics("", "let x = ac_stim();");
    assert_eq!(codes_of(&d), ["P0002"]);
    let (_, d) = diagnostics("", "let x = temperature(1.0);");
    assert_eq!(codes_of(&d), ["P0002"]);

    // A core primitive with no row of the subset is refused by name, one `P0001` each.
    for call in ["delay(1.0, 1.0)", "noise(1.0)", "ddx(1.0, v(a))", "cross(1.0)", "at(1.0)", "now()", "freq()", "analysis(Static)", "simparam(\"x\")"] {
        let (src, d) = diagnostics("", &format!("let x = {call};"));
        assert_eq!(codes_of(&d), ["P0001"], "{call}: {d:?}");
        let name = call.split('(').next().unwrap_or(call);
        assert!(d[0].diagnostic.message.contains(name), "{call}: {}", d[0].diagnostic.message);
        assert!(snippet(&src, &d[0]).starts_with(name), "{call}: {}", snippet(&src, &d[0]));
    }
}

#[test]
fn limit_checks_its_input_function_and_seed() {
    let items = "fn lim(new: f64, old: f64) -> f64 { new }\nfn lim3(new: f64, old: f64, k: f64) -> f64 { new }";
    let t = local_types(items, "let x = limit(v(a), |new, old| new); let y = limit(v(a), |new, old| lim(new, old), 0.6); let z = limit(v(a), lim); let w = limit(v(a), lim, 1.0);", &["x", "y", "z", "w"]);
    assert!(t.iter().all(|t| *t == Ty::F64), "{t:?}");

    // The input function has the type `fn(f64, f64) -> f64`.
    let (src, d) = diagnostics(items, "let x = limit(v(a), |new| new);");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "|new| new");
    let (src, d) = diagnostics(items, "let x = limit(v(a), lim3);");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "lim3");
    // The seed and the input are `f64`.
    let (src, d) = diagnostics(items, "let x = limit(v(a), lim, true);");
    assert_eq!(snippet(&src, &d[0]), "true");
    let (src, d) = diagnostics(items, "let x = limit(true, lim);");
    assert_eq!(snippet(&src, &d[0]), "true");
}

#[test]
fn a_core_primitive_in_a_fn_is_e0450() {
    let (src, d) = diagnostics("fn f(x: f64) -> f64 { ddt(x) }", "let y = f(1.0);");
    // `ddt` is a core module: a placement in a `fn` is `E0350` (§14 errors, §8.3.4).
    assert_eq!(codes_of(&d), ["E0350"]);
    assert_eq!(snippet(&src, &d[0]), "ddt(x)");

    let (src, d) = diagnostics("fn f() -> f64 { ac_stim(1.0) }", "let y = f();");
    assert_eq!(codes_of(&d), ["E0450"]);
    assert_eq!(snippet(&src, &d[0]), "ac_stim(1.0)");

    // In a closure as well.
    let (src, d) = diagnostics("", "let y = limit(v(a), |new, old| ddt(new));");
    assert_eq!(codes_of(&d), ["E0350"]);
    assert_eq!(snippet(&src, &d[0]), "ddt(new)");

    // The environment is allowed everywhere.
    let src = source("fn vt() -> f64 { temperature() * gmin() }", "let y = vt();");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("environment functions in a fn");
}

#[test]
fn the_std_vt_checks_clean() {
    // `vt` reads `temperature()` (the core table) and the constants `K_B` and `Q`.
    let design = Design::builder().source("top.msl", "use std::analog::vt;\nmodule Top() { }\nanalog for Top { let x = vt(); }").build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let reached: Vec<&str> = types.reached().iter().filter_map(|d| design.decl_data(*d).name.as_ref().map(|n| n.text.as_str())).collect();
    for name in ["vt", "K_B", "Q"] {
        assert!(reached.contains(&name), "`{name}` is reached: {reached:?}");
    }
}
