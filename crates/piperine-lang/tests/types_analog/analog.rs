//! Analog forms of `piperine_lang::types` (TYP-04): probes through `access` aliases and
//! `potential`, contributions (`+>`), and the core primitives of the subset (`ddt`, `ac_stim`,
//! `limit`, the environment functions), with the codes `E0240`, `E0410`, `E0417` and `E0450`.
//! Every source is `Top` with two `E` nodes, one `T` node and an `analog` block under test.


use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_lang::types::{Ty, Types};
use piperine_lang_fixtures::checked::CheckedSource;
use piperine_lang_fixtures::pom::PomSource;

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
nature T { potential: Tp { access: temp, units: \"K\", abstol: 1m }, flow: P { access: pwr, units: \"W\", abstol: 1n } }\n\
module Top(a: E, b: E, t: T) { }\n";

/// The sources of a module `Top` with two `E` nodes and a `T` node (`HEAD`), and what the check says of their analog blocks.
struct ElectricalSource;

impl ElectricalSource {
    fn source(items: &str, body: &str) -> String {
        format!("{HEAD}{items}\nanalog for Top {{ {body} }}")
    }

    fn local_types(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
        CheckedSource::local_types_of(&ElectricalSource::source(items, body), names)
    }

    fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
        let src = ElectricalSource::source(items, body);
        let found = CheckedSource::expected_diagnostics(&src);
        (src, found)
    }
}

#[test]
fn potential_probes_are_f64() {
    let t = ElectricalSource::local_types("", "let x = v(a); let y = v(a, b); let z = potential(a, b); let w = potential(t); let u = v(a, gnd) + v(gnd);", &["x", "y", "z", "w", "u"]);
    assert!(t.iter().all(|t| *t == Ty::F64), "{t:?}");

    // A flow probe is a contribution target; read as a value it is outside the subset.
    let (src, d) = ElectricalSource::diagnostics("", "let x = i(a, b);");
    assert_eq!(CheckedSource::codes(&d), ["P0001"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "i(a, b)");
    let src = ElectricalSource::source("", "i(a, b) +> 1.0; flow(a, b) +> 2.0;");
    let design = CheckedSource::design_of(&src);
    Types::check(&design, "Top").expect("flow probes as targets");
}

#[test]
fn probe_nodes_of_two_natures_are_e0240() {
    let (src, d) = ElectricalSource::diagnostics("", "let x = v(a, t);");
    assert_eq!(CheckedSource::codes(&d), ["E0240"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "v(a, t)");

    let (src, d) = ElectricalSource::diagnostics("", "potential(a, t) +> 1.0;");
    assert_eq!(CheckedSource::codes(&d), ["E0240"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "potential(a, t)");
}

#[test]
fn a_contribution_target_that_is_no_probe_is_e0410() {
    let (src, d) = ElectricalSource::diagnostics("", "let x = 1.0; x +> 2.0;");
    assert_eq!(CheckedSource::codes(&d), ["E0410"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "x");

    let (src, d) = ElectricalSource::diagnostics("", "(v(a) + 1.0) +> 0.0;");
    assert_eq!(CheckedSource::codes(&d), ["E0410"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "(v(a) + 1.0)");

    let src = ElectricalSource::source("", "v(a, b) +> 1.0; i(a, gnd) +> 2.0;");
    let design = CheckedSource::design_of(&src);
    Types::check(&design, "Top").expect("probe targets");
}

#[test]
fn a_contribution_that_is_not_f64_is_e0417() {
    let (src, d) = ElectricalSource::diagnostics("", "v(a, b) +> true;");
    assert_eq!(CheckedSource::codes(&d), ["E0417"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "true");

    let (src, d) = ElectricalSource::diagnostics("", "v(a, b) +> 1;");
    assert_eq!(CheckedSource::codes(&d), ["E0417"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "1");
}

#[test]
fn ddt_ac_stim_and_environment_are_f64() {
    let t = ElectricalSource::local_types(
        "",
        "let d = ddt(v(a)); let d2 = ddt(v(a), 1e-6); let s = ac_stim(1.0); let s2 = ac_stim(1.0, 0.5); \
         let tk = temperature().unwrap_or(1.0); let g = gmin(); let sc = source_scale();",
        &["d", "d2", "s", "s2", "tk", "g", "sc"],
    );
    assert!(t.iter().all(|t| *t == Ty::F64), "{t:?}");

    let (src, d) = ElectricalSource::diagnostics("", "let x = ddt(true);");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "true");
    let (_, d) = ElectricalSource::diagnostics("", "let x = ac_stim();");
    assert_eq!(CheckedSource::codes(&d), ["E0141"]);
    let (_, d) = ElectricalSource::diagnostics("", "let x = temperature(1.0);");
    assert_eq!(CheckedSource::codes(&d), ["E0141"]);

    // `delay`, `noise`, `ddx` and `simparam` left the refused-by-name list when analog-operators
    // T22 and T23 gave them rows (`types_analog_operators`); their counts are judged like any row's.
    for call in ["delay(1.0)", "noise()", "ddx(1.0)", "simparam()"] {
        let (_, d) = ElectricalSource::diagnostics("", &format!("let x = {call};"));
        assert_eq!(CheckedSource::codes(&d), ["E0141"], "{call}: {d:?}");
    }
}

#[test]
fn limit_checks_its_input_function_and_seed() {
    let items = "fn lim(new: f64, old: f64) -> f64 { new }\nfn lim3(new: f64, old: f64, k: f64) -> f64 { new }";
    let t = ElectricalSource::local_types(items, "let x = limit(v(a), |new, old| new); let y = limit(v(a), |new, old| lim(new, old), 0.6); let z = limit(v(a), lim); let w = limit(v(a), lim, 1.0);", &["x", "y", "z", "w"]);
    assert!(t.iter().all(|t| *t == Ty::F64), "{t:?}");

    // The input function has the type `fn(f64, f64) -> f64`.
    let (src, d) = ElectricalSource::diagnostics(items, "let x = limit(v(a), |new| new);");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "|new| new");
    let (src, d) = ElectricalSource::diagnostics(items, "let x = limit(v(a), lim3);");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "lim3");
    // The seed and the input are `f64`.
    let (src, d) = ElectricalSource::diagnostics(items, "let x = limit(v(a), lim, true);");
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "true");
    let (src, d) = ElectricalSource::diagnostics(items, "let x = limit(true, lim);");
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "true");
}

#[test]
fn a_core_primitive_in_a_fn_is_e0450() {
    let (src, d) = ElectricalSource::diagnostics("fn f(x: f64) -> f64 { ddt(x) }", "let y = f(1.0);");
    // `ddt` is a core module: a placement in a `fn` is `E0350` (§14 errors, §8.3.4).
    assert_eq!(CheckedSource::codes(&d), ["E0350"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "ddt(x)");

    let (src, d) = ElectricalSource::diagnostics("fn f() -> f64 { ac_stim(1.0) }", "let y = f();");
    assert_eq!(CheckedSource::codes(&d), ["E0450"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "ac_stim(1.0)");

    // In a closure as well.
    let (src, d) = ElectricalSource::diagnostics("", "let y = limit(v(a), |new, old| ddt(new));");
    assert_eq!(CheckedSource::codes(&d), ["E0350"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "ddt(new)");

    // The environment is allowed everywhere.
    let src = ElectricalSource::source("fn vt() -> f64 { temperature().unwrap_or(1.0) * gmin() }", "let y = vt();");
    let design = CheckedSource::design_of(&src);
    Types::check(&design, "Top").expect("environment functions in a fn");
}

#[test]
fn the_std_vt_checks_clean() {
    // `vt` reads `temperature()` (the core table) and the constants `K_B` and `Q`.
    let design = Design::builder().source("top.msl", "use std::analog::vt;\nmodule Top() { }\nanalog for Top { let x = vt(300.15); }").build();
    assert!(design.diagnostics().is_empty(), "{:?}", PomSource::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let reached: Vec<&str> = types.reached().iter().filter_map(|d| design.decl_data(*d).name.as_ref().map(|n| n.text.as_str())).collect();
    for name in ["vt", "K_B", "Q"] {
        assert!(reached.contains(&name), "`{name}` is reached: {reached:?}");
    }
}

#[test]
fn freq_analysis_kinds_and_bound_step_type() {
    // FIX-09 to FIX-11 (§14.4, §14.10): `freq()` is an `f64`, `analysis(kind)` a `bool` for each
    // of `Dc`, `Transient`, `Ac` and `Noise` (and `Static`), and `bound_step(dt)` a statement
    // of `analog` that takes one `f64`.
    let t = ElectricalSource::local_types("", "let f = freq(); let d = analysis(Dc); let tr = analysis(Transient); let ac = analysis(Ac); let n = analysis(Noise); bound_step(1.0e-9);", &["f", "d", "tr", "ac", "n"]);
    assert_eq!(t, [Ty::F64, Ty::Bool, Ty::Bool, Ty::Bool, Ty::Bool]);

    let (src, d) = ElectricalSource::diagnostics("", "bound_step(true);");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "true");
    let (_, d) = ElectricalSource::diagnostics("", "bound_step();");
    assert_eq!(CheckedSource::codes(&d), ["E0141"]);
    let (_, d) = ElectricalSource::diagnostics("", "let f = freq(1.0);");
    assert_eq!(CheckedSource::codes(&d), ["E0141"]);
    // an argument that is no kind of analysis is still refused by name
    let (_, d) = ElectricalSource::diagnostics("", "let x = analysis(Some);");
    assert_eq!(CheckedSource::codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("analysis"), "{}", d[0].diagnostic.message);

    // §8.2.5: `freq()` is continuous, so a definition may not read it (`E0336`); `analysis(Dc)` is
    // constant during an analysis (§14.4.1), so it may be read there.
    let allowed = "module Top(out b: Bit) { b = analysis(Dc); }";
    let with_prelude = |src: &str| {
        let design = Design::builder().source("top.msl", src).build();
        assert!(design.diagnostics().is_empty(), "{:?}", PomSource::codes(&design));
        design
    };
    assert!(Types::check(&with_prelude(allowed), "Top").is_ok(), "analysis(Dc) is discrete");
    let refused = "module Top(out f: Bit<f64>) { f = freq(); }";
    let codes: Vec<String> = Types::check(&with_prelude(refused), "Top").expect_err("freq() is continuous").iter().map(|d| d.diagnostic.code.to_string()).collect();
    assert_eq!(codes, ["E0336"]);
}

#[test]
fn simparam_types_with_a_name_as_an_option() {
    // FIX-10 AC 4 moved: `simparam(name)` is an `Option<f64>` since analog-operators T23 and the user decision
    // of 2026-11-23 (AO-07): no default argument. A name that is no text is a mismatch.
    let (src, d) = ElectricalSource::diagnostics("", "let x = simparam(1.0);");
    assert_eq!(CheckedSource::codes(&d).len(), 1, "{d:?}");
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "1.0");
}

/// `limexp` (std `analog.msl`, §22.2.4) is a module with a result whose body hands `limit` a
/// closure: the closure's parameters are bindings the module walk must not refuse (D9). It is
/// checked over the real standard library, which the helpers above replace by an empty one.
#[test]
fn limexp_checks_with_its_closure_bindings() {
    let src = "module Top(a: Electrical) { }\nanalog for Top { let x = limexp(v(a) / 0.025); }";
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "{:?}", PomSource::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    assert_eq!(*types.local(CheckedSource::binding(&design, "x")).expect("x is typed"), Ty::F64);
}
