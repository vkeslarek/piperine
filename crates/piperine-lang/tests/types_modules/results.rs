//! Modules with a result, time and the analysis of `piperine_lang::types` (feature
//! `msl-digital`, DIG-06): `module name(inputs) -> T { items result }` and its placement in an
//! expression (part III §8.3), `now()` and `analysis(Static)` (part IV §14.4), what an `analog`
//! block reads of a carrier (any expression, part I §3.3.6) and a `let` that reads itself. Sources
//! are written over the embedded standard library: `transition`, `above` and `init` are the
//! stdlib's own.

use piperine_lang::pom::Reflect;
use piperine_lang::types::{Callee, Ty, Types};
use piperine_lang_fixtures::checked::CheckedSource;

const TWICE: &str = "module twice(x: f64) -> f64 { 2.0 * x }\n";

#[test]
fn a_module_with_a_result_is_typed() {
    // DIG-06 AC 1 (§8 rule 3): a module with value inputs and `-> T` whose last expression has the
    // type `T`; any other type is `E0301`, at the result; no result is `E0301` at the module.
    let src = format!("{TWICE}module ev(x: f64) -> Event {{ cross(x) }}\nmodule Top() {{ let a = twice(1.0); }}");
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    // A read of a value input has its type.
    assert!(CheckedSource::expressions_reading(&design, &types, &src, "x").iter().all(|e| types.ty(*e) == Some(&Ty::F64)) || CheckedSource::expressions_reading(&design, &types, &src, "x").is_empty());
    for (module, at) in [
        ("module bad(x: f64) -> f64 { true }", "true"),
        ("module bad(x: f64) -> bool { x }", "x"),
        ("module bad(x: f64) -> Event { x }", "x"),
        ("module bad(x: f64) -> f64 { let y = x; }", "bad"),
    ] {
        let src = format!("{module}\nmodule Top() {{ let a = bad(1.0); }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0301"], "{module}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{module}");
    }
    // A value input takes no direction and no initial value (§8.1.5, §8.1.7).
    let src = "module mix(in x: f64) -> f64 { x }\nmodule Top() { let a = mix(1.0); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0310".to_string()], "f64"));
    let src = "module mix(x: f64 = 1.0) -> f64 { x }\nmodule Top() { let a = mix(1.0); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0311".to_string()], "1.0"));

    // `transition`, `above` and `init` of the stdlib, placed as the buck and `A2D` place them.
    let src = "module Top(in g: Bit, p: Electrical, out d: Bit) {\n    let gate = transition(if g { 1.0 } else { 0.0 }, 0.0, 5n, 5n);\n    let up = above(v(p) - 0.5);\n    let start = init();\n    let cmp = A2D(p, d) { vth: 0.74, hyst: 20m };\n}\nanalog for Top { v(p) +> gate; }\ndigital for Top { on up { } on start { } }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    assert_eq!([types.local(CheckedSource::root_binding(&design, "gate")), types.local(CheckedSource::root_binding(&design, "up")), types.local(CheckedSource::root_binding(&design, "start"))], [Some(&Ty::F64), Some(&Ty::Event), Some(&Ty::Event)]);
    let transition = design.module("transition").expect("std transition").decl().id();
    assert!(types.reached().contains(&transition));
}

#[test]
fn a_placement_has_its_result_type() {
    // DIG-06 AC 2 (§8.3.4): a placement in a body `let`, an `analog` block, an event position and an
    // argument of another placement has the type of the result; a module without one, the module.
    let src = format!("{TWICE}module ev(x: f64) -> Event {{ cross(x) }}\nmodule Plain(a: Electrical) {{ }}\nmodule Top(p: Electrical) {{\n    let a = twice(1.0);\n    let nested = twice(twice(2.0));\n    let inst = Plain(p);\n}}\nanalog for Top {{ let b = twice(v(p)); }}\ndigital for Top {{ on ev(v(p)) {{ }} }}");
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    assert_eq!([types.local(CheckedSource::root_binding(&design, "a")), types.local(CheckedSource::root_binding(&design, "nested")), types.local(CheckedSource::root_binding(&design, "b"))], [Some(&Ty::F64), Some(&Ty::F64), Some(&Ty::F64)]);
    let plain = design.module("Plain").expect("Plain").decl().id();
    assert_eq!(types.local(CheckedSource::root_binding(&design, "inst")), Some(&Ty::module(plain)));
    let head = CheckedSource::expressions_reading(&design, &types, &src, "ev(v(p))");
    assert_eq!(head.iter().map(|e| types.ty(*e)).collect::<Vec<_>>(), [Some(&Ty::Event)]);
    let inner = CheckedSource::expressions_reading(&design, &types, &src, "twice(2.0)");
    let twice = design.module("twice").expect("twice").decl().id();
    assert_eq!((types.ty(inner[0]), types.callee(inner[0])), (Some(&Ty::F64), Some(Callee::Module(twice))));
    // A result placed in an event position outside one is `E0354` (§8.3.10), proved by
    // `an_event_result_outside_an_event_position_is_e0354`; it was `E0441`.
    let src = format!("module ev(x: f64) -> Event {{ cross(x) }}\nmodule Top(p: Electrical) {{ }}\nanalog for Top {{ let e = ev(v(p)); }}");
    let d = CheckedSource::standard_expected_diagnostics(&src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(&src, &d[0])), (vec!["E0354".to_string()], "ev(v(p))"));
}

#[test]
fn trailing_option_inputs_may_be_left_out() {
    // DIG-06 AC 3, 8 (§3.2, §8.3.7): `transition(x)` and `transition(x, 0.0, tr, tr)` are accepted,
    // `None` and `Some` too; a value for an `Option<T>` is `Some` of it; too few or too many
    // arguments are `E0141`, and so is a missing input before a trailing `Option`.
    let src = "module Top(in k: Bit<f64>) {\n    tr: f64 = 1n;\n    let a = transition(k);\n    let b = transition(k, 0.0, tr, tr);\n    let c = transition(k, None, Some(2n), tr);\n    let d = transition(k, 1n);\n}";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    let literals = CheckedSource::expressions_reading(&design, &types, src, "1n");
    assert!(literals.iter().any(|e| types.implicit_some(*e)), "a value for an `Option` input is `Some` of it");
    for (call, at) in [("transition()", "transition()"), ("transition(k, 0.0, 1n, 1n, 2n)", "transition(k, 0.0, 1n, 1n, 2n)")] {
        let src = format!("module Top(in k: Bit<f64>) {{ let a = {call}; }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0141"], "{call}");
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at);
    }
    let src = "module mix(a: Option<f64>, b: f64) -> f64 { b }\nmodule Top() { let r = mix(1.0); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0141".to_string()], "mix(1.0)"));
    // A plain module keeps its own code for a wrong count.
    let src = "module Plain(in a: Bit) { }\nmodule Top(in k: Bit) { let r = Plain(k, k); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!(CheckedSource::codes(&d), ["E0312"]);
    // An argument of the wrong type is a mismatch at the argument.
    let src = "module Top(in k: Bit<f64>) { let a = transition(k, true); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0180".to_string()], "true"));
}

#[test]
fn a_placement_in_a_const_context_is_e0352() {
    // DIG-06 AC 4 (§8.3.4): a placement in a param default, a `where`, a `const` item and a bound
    // param value is `E0352`, at the placement.
    let items = format!("{TWICE}module Child() {{ x: f64 = 1.0; }}\n");
    for (rest, at) in [
        ("module Top() { d: f64 = twice(1.0); }", "twice(1.0)"),
        ("module Top() { d: f64 = 1.0 where twice(d) > 0.0; }", "twice(d)"),
        ("const C: f64 = twice(1.0);\nmodule Top() { d: f64 = C; }", "twice(1.0)"),
        ("module Top() { let c = Child() { x: twice(2.0) }; }", "twice(2.0)"),
        ("module Top() { let c = Child { x: twice(2.0) }(); }", "twice(2.0)"),
        ("module Top(p: Electrical) { }\nanalog for Top { let a = 1.0; }\nmodule Other() { d: f64 = ddt(1.0); }\nmodule Use() { let o = Other(); }", "ddt(1.0)"),
    ] {
        let src = format!("{items}{rest}");
        let design = CheckedSource::standard_design_of(&src);
        let top = if rest.contains("module Use") { "Use" } else { "Top" };
        let d = Types::check(&design, top).expect_err(rest);
        assert_eq!(CheckedSource::codes(&d), ["E0352"], "{rest}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{rest}");
    }
}

#[test]
fn now_and_analysis_static_are_typed() {
    // DIG-06 AC 5 (§14.4): `now()` is `f64` and `analysis(Static)` is `bool` in a module body, an
    // `analog` block and a reaction; each is `E0323` in a param default; the other analysis kinds
    // are typed `bool` too since FIX-11 (they were `P0001` naming `analysis`).
    let src = "module Top(in clk: Bit, p: Electrical, out t: Bit<f64> = 0.0) {\n    let a = now();\n    let b = analysis(Static);\n}\nanalog for Top { let c = now(); let d = analysis(Static); }\ndigital for Top { on clk.rise() { let e = now(); let f = analysis(Static); t <- e; } }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    let got: Vec<Option<&Ty>> = ["a", "b", "c", "d", "e", "f"].iter().map(|n| types.local(CheckedSource::root_binding(&design, n))).collect();
    assert_eq!(got, [Some(&Ty::F64), Some(&Ty::Bool), Some(&Ty::F64), Some(&Ty::Bool), Some(&Ty::F64), Some(&Ty::Bool)]);
    for (default, at) in [("d: f64 = now();", "now()"), ("d: bool = analysis(Static);", "analysis(Static)")] {
        let src = format!("module Top() {{ {default} }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0323"], "{default}");
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at);
    }
    for kind in ["Ac", "Dc", "Transient", "Noise"] {
        let src = format!("module Top() {{ let a = analysis({kind}); }}");
        let design = CheckedSource::standard_design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        assert_eq!(types.local(CheckedSource::root_binding(&design, "a")), Some(&Ty::Bool), "{kind}");
        let src = format!("module Top() {{ d: bool = analysis({kind}); }}");
        assert_eq!(CheckedSource::codes(&CheckedSource::standard_expected_diagnostics(&src)), ["E0323"], "{kind} in a param default");
    }
}

#[test]
fn a_bare_analog_read_of_a_carrier_has_no_diagnostic() {
    // errors#1 of the Book (chapter 2) was `E0150` with a help line; §3.3.6 makes the source plain
    let src = "module Top(in c: Bit<f64>, p: Electrical) { }\nanalog for Top { v(p) +> c; }";
    CheckedSource::checked(&CheckedSource::standard_design_of(src), src);
}

#[test]
fn an_analog_read_of_a_carrier_is_the_value() {
    // DIG-06 AC 6 as §3.3.6 amends it: an `analog` block reads a discrete carrier like any
    // expression, a bare read, one through a `let`, one through a value input and one through a
    // result; the sources that were `E0150` are accepted
    let items = format!("{TWICE}module Amp(x: f64, p: Electrical) {{ }}\nanalog for Amp {{ v(p) +> x; }}\n");
    for (inside, analog) in [
        ("", "v(p) +> c;"),
        ("", "v(p) +> c + 1.0;"),
        ("", "v(p) +> c.prev();"),
        ("", "v(p) +> (c + 1.0);"),
        ("let k = c * 2.0;", "v(p) +> k;"),
        ("let a = Amp(c, p);", ""),
        ("let g = twice(c);", "v(p) +> g;"),
        ("", "let q = c; v(p) +> q;"),
        ("", "let q = c + 1.0; v(p) +> q + q;"),
    ] {
        let src = format!("{items}module Top(in c: Bit<f64>, in en: Bit, p: Electrical) {{ {inside} }}\nanalog for Top {{ {analog} }}");
        CheckedSource::checked(&CheckedSource::standard_design_of(&src), &src);
    }
    // A bare read beside the others.
    let src = format!("{items}module Top(in c: Bit<f64>, in en: Bit, p: Electrical) {{\n    let k = c * 2.0;\n    let g = twice(c);\n    let a = Amp(c, p);\n}}\nanalog for Top {{ v(p) +> k + g + c + if en {{ 1.0 }} else {{ 0.0 }}; }}");
    CheckedSource::checked(&CheckedSource::standard_design_of(&src), &src);
    // An expression at an `in` port is a definition of the parent and outside the rule.
    let src = "module Child(in a: Bit<f64>) { }\nmodule Top(in c: Bit<f64>) { let k = Child(c * 2.0); }";
    CheckedSource::checked(&CheckedSource::standard_design_of(src), src);
}

#[test]
fn a_let_that_reads_itself_is_e0332_or_e0353() {
    // DIG-06 AC 7 (§8.2.16, §8.3.5): `let a = a + 1.0;` is `E0332`; a cycle through a result that
    // depends directly on its input is `E0353`: `let y = twice(y);` with `module twice(x: f64) -> f64
    // { 2.0 * x }`, and through a core module.
    let src = "module Top() { let a = a + 1.0; }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!(CheckedSource::codes(&d), ["E0332"]);
    assert_eq!((CheckedSource::snippet(src, &d[0]), d[0].diagnostic.span().start), ("a", src.find("a + 1.0").unwrap()));
    for (items, body, arg) in [
        (TWICE, "let y = twice(y);", "(y)"),
        ("", "let y = ddt(y);", "(y)"),
        (TWICE, "let y = twice(1.0 + twice(y));", "(y))"),
    ] {
        let src = format!("{items}module Top() {{ {body} }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0353"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!((CheckedSource::snippet(&src, &d[0]), d[0].diagnostic.span().start), ("y", src.find(arg).unwrap() + 1), "{body}");
    }
    // A cycle through a result that reads only its own carriers is legal (`let y = idt(f(y))`,
    // §8.3.5): it types (the feedback test lives in `types_structure.rs`, analog-structure T25,
    // which lifted the `P0001` this row asserted).
    let src = "module integ(x: f64) -> f64 {\n    signal s: Bit<f64> = 0.0;\n    s\n}\nmodule Top() { let y = integ(y); }";
    CheckedSource::checked(&CheckedSource::standard_design_of(src), src);
}

#[test]
fn a_continuous_result_read_where_probes_are_not_allowed_is_e0354() {
    // §8.3.10 (`E0354`): a continuous result read in a param default, where no probe may be written,
    // names the result; it was `P0001`. A discrete result is read there without a code.
    let src = "module Top() { signal a: Electrical; let g = ddt(v(a)); q: f64 = g; }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0354".to_string()], "g"));
    assert!(d[0].diagnostic.message.contains("`g`"), "{}", d[0].diagnostic.message);
}

#[test]
fn an_event_result_outside_an_event_position_is_e0354() {
    // §8.3.10 (`E0354`): the `Event` result of a placement read where only a value may stand. An
    // core `cross` written there is not a result: it keeps its own code `E0443`.
    for body in ["analog for Top { let e = ev(v(p)); }", "analog for Top { i(p) +> ev(v(p)); }"] {
        let src = format!("module ev(x: f64) -> Event {{ cross(x) }}\nmodule Top(p: Electrical) {{ }}\n{body}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(&src, &d[0])), (vec!["E0354".to_string()], "ev(v(p))"), "{body}");
    }
    let src = "module Top(p: Electrical) { }\nanalog for Top { let e = cross(v(p)); }";
    assert_eq!(CheckedSource::codes(&CheckedSource::standard_expected_diagnostics(src)), ["E0443"]);
}

#[test]
fn a_composite_param_of_a_module_with_a_result_checks_at_its_placement() {
    // part III §8.3.12: a placement's param block gives an array or a tuple param its value like any other param; the check
    // reads `c[1]` as an `f64` and the placement as the module's result type
    let src = "module Poly(x: f64) -> f64 { c: [f64; 2] = [1.0, 2.0]; x * c[1] }\nmodule Top() { let a = Poly(2.0) { c: [5.0, 7.0] }; }";
    let design = CheckedSource::standard_design_of(src);
    CheckedSource::checked(&design, src);
}
