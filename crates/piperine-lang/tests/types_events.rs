//! Events, reactions and records of `piperine_lang::types` (feature `msl-digital`, DIG-05): the
//! forms that make an `Event` (`changed`, `rise`, `fall`, `cross`, `at`, `||`, `&&` with a guard),
//! where one may stand (part IV §13) and the head of an `on` (`E0430`). Sources are written over
//! the embedded standard library.

#[path = "support/buck.rs"]
#[allow(dead_code)]
mod buck;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, Reflect};
use piperine_lang::types::{Builtin, Ty, Types};

fn design_of(src: &str) -> Design {
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    design
}

fn checked<'d>(design: &'d Design, src: &str) -> Types<'d> {
    Types::check(design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()))
}

fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
    let design = design_of(src);
    Types::check(&design, "Top").expect_err("expected diagnostics")
}

fn codes(d: &[DesignDiagnostic]) -> Vec<String> {
    d.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

fn binding(design: &Design, name: &str) -> DeclId {
    design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id()
}

/// The reached expressions that read as `text`, in source order.
fn exprs_reading(design: &Design, types: &Types, src: &str, text: &str) -> Vec<ExprId> {
    (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && src[design.expr_node(*e).span.range()] == *text).collect()
}

const HEAD: &str = "module Top(in clk: Bit, in en: Bit, in k: Bit<f64>, p: Electrical) { }\n";

/// `Top` with `heads` as the heads of empty reactions.
fn with_heads(heads: &[&str]) -> String {
    let reactions: String = heads.iter().map(|h| format!("on {h} {{ }}\n")).collect();
    format!("{HEAD}digital for Top {{\n{reactions}}}")
}

#[test]
fn event_expressions_are_typed() {
    // DIG-05 AC 1: `c.changed()`, `c.rise()`, `c.fall()`, `cross(e)`, `at(t)`, `a || b` and
    // `ev && guard` are an `Event`, in a body `let` and as the head of an `on`; a guard reads
    // anything a reaction reads, a probe included.
    let src = "module Top(in clk: Bit, in en: Bit, in k: Bit<f64>, p: Electrical) {\n    let a = clk.changed();\n    let b = clk.rise();\n    let c = clk.fall();\n    let d = cross(v(p) - 0.5);\n    let e = at(1u + k.known_or(0.0) * 2.0);\n    let f = clk.rise() || clk.fall();\n    let g = clk.rise() && en.high();\n    let h = (clk.rise() || en.changed()) && v(p) > 0.2;\n}";
    let design = design_of(src);
    let types = checked(&design, src);
    for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
        assert_eq!(types.local(binding(&design, name)), Some(&Ty::Event), "{name}");
    }
    let builtins: Vec<Option<Builtin>> = ["changed", "rise", "fall"].iter().flat_map(|n| (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && matches!(&design.expr_node(*e).kind, ExprKind::MethodCall { method, .. } if method.text == *n)).map(|e| types.builtin(e)).collect::<Vec<_>>()).collect();
    assert_eq!(builtins.iter().filter(|b| **b == Some(Builtin::Changed)).count(), 2);
    assert_eq!(builtins.iter().filter(|b| **b == Some(Builtin::Rise)).count(), 4);
    assert_eq!(builtins.iter().filter(|b| **b == Some(Builtin::Fall)).count(), 2);

    // As the heads of reactions, a guard of any `bool` included.
    let src = with_heads(&["clk.rise()", "cross(v(p) - 0.5)", "at(1.0)", "clk.rise() || en.fall()", "clk.rise() && en.high() && v(p) > 0.0"]);
    let design = design_of(&src);
    let types = checked(&design, &src);
    for head in ["clk.rise()", "cross(v(p) - 0.5)", "at(1.0)", "clk.rise() || en.fall()", "clk.rise() && en.high() && v(p) > 0.0"] {
        let found = exprs_reading(&design, &types, &src, head);
        assert!(!found.is_empty() && found.iter().all(|e| types.ty(*e) == Some(&Ty::Event)), "{head}: {found:?}");
    }

    // `rise` and `fall` are the edges of a `Bit` of `bool`; a node has no `changed`.
    let d = diagnostics("module Top(in c: Bit<u8>) { let e = c.rise(); }");
    assert_eq!(codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("rise") && d[0].diagnostic.message.contains("Bit<u8>"), "{}", d[0].diagnostic.message);
    let src = "module Top(p: Electrical) { let e = p.changed(); }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0230".to_string()], "p.changed()"));

    // Out of the subset: a tolerance given to `cross`, `settle`.
    for (head, at, names) in [("cross(v(p), 1n)", "1n", "`ttol`"), ("cross(v(p), None, Some(1n))", "Some(1n)", "`etol`"), ("settle", "settle", "`settle`")] {
        let src = with_heads(&[head]);
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["P0001"], "{head}");
        assert!(d[0].diagnostic.message.contains(names), "{head}: {}", d[0].diagnostic.message);
        assert_eq!(snippet(&src, &d[0]), at, "{head}");
    }
    // No tolerance, or tolerances that are `None`, is the event the `Cross` node has.
    let src = with_heads(&["cross(v(p))", "cross(v(p), None, None)"]);
    checked(&design_of(&src), &src);
}

#[test]
fn a_bool_head_is_e0430() {
    // DIG-05 AC 2 (§12.1.10): the head of an `on` is an `Event`. `on clk { .. }` with `clk: Bit` is
    // `E0430` at `clk`; so is any other `bool`, and any other type.
    for head in ["clk", "en && clk", "k.known_or(1.0) > 0.0", "true", "k.known_or(1.0)"] {
        let src = with_heads(&[head]);
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0430"], "{head}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), head);
    }
    // An `||` of an event and a `bool` is a mismatch of its operand.
    let src = with_heads(&["clk.rise() || en"]);
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["P0002".to_string()], "en"));
}

#[test]
fn a_misplaced_not_or_and_is_e0440() {
    // DIG-05 AC 3 (§13 rule 1): `!` applies to a `bool`, never to an `Event`; `&&` has the event on
    // the left and the guard on the right.
    for head in ["!clk.rise()", "(!clk.fall()) || clk.rise()", "en && clk.rise()", "clk.rise() || (en && clk.fall())"] {
        let src = with_heads(&[head]);
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0440"], "{head}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    }
    let src = with_heads(&["!clk.rise()"]);
    assert_eq!(snippet(&src, &diagnostics(&src)[0]), "!clk.rise()");
    let src = with_heads(&["en && clk.rise()"]);
    assert_eq!(snippet(&src, &diagnostics(&src)[0]), "en && clk.rise()");
    // `!` on a `bool` guard is the ordinary `!`.
    let src = with_heads(&["clk.rise() && !en"]);
    checked(&design_of(&src), &src);
}

#[test]
fn an_event_outside_an_event_position_is_e0441() {
    // DIG-05 AC 4 (§13 rule 8): an `Event` in a definition, in an `analog` block, as an argument or
    // in a `fn` that does not return one is `E0441`, at the event. Its positions are the head of an
    // `on`, a body `let`, the result of a module and the value of a function that returns `Event`.
    let cases: [(&str, &str); 5] = [
        ("module Top(in clk: Bit, out q: Bit) { q = clk.rise(); }", "clk.rise()"),
        ("module Top(in clk: Bit, out q: Bit) { q = clk.rise() || clk.fall(); }", "clk.rise() || clk.fall()"),
        ("module Top(in clk: Bit) { }\nanalog for Top { let x = clk.changed(); }", "clk.changed()"),
        ("fn pick(e: bool) -> bool { e }\nmodule Top(in clk: Bit) { let y = pick(clk.rise()); }", "clk.rise()"),
        ("module Top(in clk: Bit) { }\nanalog for Top { let z = true && clk.fall(); }", "clk.fall()"),
    ];
    for (src, at) in cases {
        let d = diagnostics(src);
        assert_eq!(codes(&d), ["E0441"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(src, &d[0]), at, "{src}");
    }
    // An event position takes one: a body `let`, a function returning `Event`, an operand of `||`.
    let src = "module Top(in clk: Bit, in en: Bit) { let a = clk.rise(); let b = a || clk.fall(); }";
    checked(&design_of(src), src);
}

#[test]
fn a_continuous_at_is_e0442() {
    // DIG-05 AC 5 (§13 rule 11): the argument of `at` is a discrete expression: no probe, no
    // `now()`, no `analysis(Static)`, directly or through a `let`.
    for (body, at) in [
        ("let e = at(now() + 1n);", "now() + 1n"),
        ("let e = at(v(p));", "v(p)"),
        ("let t = v(p) * 2.0; let e = at(t);", "t"),
        ("let e = at(if analysis(Static) { 1.0 } else { 2.0 });", "if analysis(Static) { 1.0 } else { 2.0 }"),
    ] {
        let src = format!("module Top(p: Electrical) {{ {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0442"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    // A time that reads carriers and params is discrete.
    let src = "module Top(in k: Bit<u8>) { half: f64 = 50n; let e = at(1u + k.known_or(0) as f64 * half); }";
    checked(&design_of(src), src);
}

#[test]
fn cross_or_at_outside_an_event_position_is_e0443() {
    // DIG-05 AC 6 (§13 rule 10): `cross` and `at` are core modules placed in event positions only:
    // in an `analog` block, as a definition, as an argument, in a `fn`. A reaction body is asserted
    // with the reaction bodies (T13).
    let cases: [(&str, &str); 5] = [
        ("module Top(p: Electrical) { }\nanalog for Top { let x = cross(v(p)); }", "cross(v(p))"),
        ("module Top(p: Electrical) { }\nanalog for Top { let x = at(1.0); }", "at(1.0)"),
        ("module Top(p: Electrical, out q: Bit) { q = cross(v(p)); }", "cross(v(p))"),
        ("fn f(x: f64) -> bool { true }\nmodule Top(p: Electrical) { let a = f(cross(v(p))); }", "cross(v(p))"),
        ("fn g() -> bool { let e = at(1.0); true }\nmodule Top() { let a = g(); }", "at(1.0)"),
    ];
    for (src, at) in cases {
        let d = diagnostics(src);
        assert_eq!(codes(&d)[0], "E0443", "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(src, &d[0]), at, "{src}");
    }
}

// ---- reactions and commits (T13) ----

/// The buck's `Dpwm` with the buck's own helper functions, minus the `Never` monitor it places
/// (the monitor is a stdlib module with a `require`; T14 and T16 reach it).
fn dpwm() -> String {
    let module = buck::module_with_digital("dpwm.msl", "Dpwm");
    let monitor = "    /// The two commands are never on together (SPEC §5.3, §10 S10).\n    let no_shoot_through = Never(clk, hs & ls);\n";
    assert!(module.contains(monitor), "the buck's Dpwm places Never");
    format!("{}\n{}\n{}\n{}", buck::spec(), buck::item("dpwm.msl", "fn rev3"), buck::item("dpwm.msl", "fn dithered_width"), module.replace(monitor, ""))
}

fn buck_module(file: &str, name: &str) -> String {
    format!("{}\n{}", buck::spec(), buck::module_with_digital(file, name))
}

fn checked_top(src: &str, top: &str) {
    let design = design_of(src);
    if let Err(d) = Types::check(&design, top) {
        panic!("{top}: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
}

#[test]
fn a_reaction_body_is_typed() {
    // DIG-05 AC 7: a body with `let`, `if`, `match`, calls, commits with and without `after`, a probe
    // and `now()`. The reactions of `Dpwm`, `Pid`, `RefRamp`, `Adc` and `SetpointReg` of the buck
    // are the positive cases, read as written.
    checked_top(&dpwm(), "Dpwm");
    let pid = format!("{}\n{}", buck_module("control.msl", "Pid"), "module Top() { }");
    let design = design_of(&pid);
    let types = Types::check(&design, "Pid").unwrap_or_else(|d| panic!("Pid: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let got: Vec<Option<&Ty>> = ["e", "p", "u", "windup", "d"].iter().map(|n| types.local(binding(&design, n))).collect();
    assert_eq!(got, [Some(&Ty::Int(16)), Some(&Ty::Int(32)), Some(&Ty::Int(32)), Some(&Ty::Bool), Some(&Ty::Int(32))]);
    let ramp = format!("{}\n{}", buck_module("control.msl", "RefRamp"), "module Top() { }");
    checked_top(&ramp, "RefRamp");
    checked_top(&buck_module("control.msl", "SetpointReg"), "SetpointReg");
    checked_top(&buck_module("sense.msl", "Adc"), "Adc");

    // A body of every statement kind, and the read of a carrier in a guard and in the body
    // (DIG-04 AC 3): `u9`.
    let src = "#[derive(Repr, Clone, Copy, PartialEq)]\nenum Phase { Idle, Run }\n\
module Top(in clk: Bit, in go: Bit, p: Electrical, out cnt: Bit<u9> = 0, out when: Bit<f64> = 0.0, out ph: Bit<Phase> = Phase::Idle) { gap: f64 = 5n; }\n\
digital for Top {\n    on clk.rise() && cnt > 3 {\n        let next = cnt + 1;\n        let level = v(p);\n        if go.known_or(false) { cnt <- next; } else { cnt <- 0 after gap; }\n        match ph.known_or(Phase::Idle) { Phase::Idle => { ph <- Phase::Run; } Phase::Run => { ph <- Phase::Idle after gap; } }\n        when <- now() + level;\n    }\n}";
    let design = design_of(src);
    let types = checked(&design, src);
    let cnts = exprs_reading(&design, &types, src, "cnt");
    assert!(cnts.len() >= 3 && cnts.iter().all(|e| types.ty(*e) == Some(&Ty::UInt(9)) || matches!(types.ty(*e), Some(Ty::UInt(9)))), "{cnts:?}");
    assert_eq!(types.local(binding(&design, "next")), Some(&Ty::UInt(9)));
    assert_eq!(types.local(binding(&design, "level")), Some(&Ty::F64));
}

#[test]
fn a_commit_of_another_type_is_e0432() {
    // DIG-05 AC 8 (§12.1.9): the committed expression has the value type of its target. An `Event`
    // committed is `E0441` (§13 rule 8); a commit outside a reaction body is outside the subset.
    for (body, at, code) in [
        ("count <- true;", "true", "E0432"),
        ("count <- 1.5;", "1.5", "E0432"),
        ("count <- wide;", "wide", "E0432"),
        ("flag <- clk.rise();", "clk.rise()", "E0441"),
        ("flag <- clk.rise() || clk.fall();", "clk.rise() || clk.fall()", "E0441"),
    ] {
        let src = format!("module Top(in clk: Bit, in wide: Bit<u12>, out count: Bit<u9> = 0, out flag: Bit = false) {{ }}\ndigital for Top {{ on clk.rise() {{ {body} }} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), [code], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    // A literal takes the type of its target (a `<-` in an `analog` block is a parse error of the POM).
    let src = "module Top(in clk: Bit, out count: Bit<u9> = 0) { }\ndigital for Top { on clk.rise() { count <- 511; } }";
    checked_top(src, "Top");
}

#[test]
fn a_delay_that_is_not_f64_is_e0433() {
    // DIG-05 AC 9 (§12.1.13, §8.2.22): the delay of a commit, and of a definition, is an `f64`.
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit = false) { }\ndigital for Top { on clk.rise() { q <- d after 5; } }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0433".to_string()], "5"));
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit = false) { }\ndigital for Top { on clk.rise() { q <- d after true; } }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0433".to_string()], "true"));
    let src = "module Top(in d: Bit, out q: Bit = false) { q = d after 1; }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0433".to_string()], "1"));
    // Seconds as an `f64`, a read of a carrier and a call included, type.
    let src = "module Top(in clk: Bit, in d: Bit, in w: Bit<f64>, out q: Bit = false) { tpd: f64 = 1n; }\ndigital for Top { on clk.rise() { q <- d after 5n; q <- d after tpd + w.known_or(0.0); } }";
    checked_top(src, "Top");
}

#[test]
fn a_definition_and_a_commit_are_e0221() {
    // DIG-04 AC 10 (§5.2, §12.1.8): one module drives a carrier by one definition or by commits,
    // never both; the error is at the commit. A carrier a commit drives and a module hands to an
    // `out` port has a second drive (`E0220`, DIG-04 AC 11).
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit) { q = d; }\ndigital for Top { on clk.rise() { q <- d; } }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0221".to_string()], "q"));
    assert_eq!(d[0].diagnostic.span().start, src.rfind("q <- d").unwrap());
    // Two commits of one carrier are one drive.
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit) { }\ndigital for Top { on clk.rise() { q <- d; q <- !d after 1n; } on clk.fall() { q <- true; } }";
    checked_top(src, "Top");
    // Commits and a hand-over of one carrier.
    let src = "module Source(out q: Bit) { }\nmodule Top(in clk: Bit, in d: Bit) { signal s: Bit; let a = Source(s); }\ndigital for Top { on clk.rise() { s <- d; } }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0220"]);
    assert_eq!(d[0].diagnostic.span().start, src.find("s)").unwrap());
}

#[test]
fn a_placement_in_a_reaction_is_e0350() {
    // DIG-05 AC 10 (§8.3.4): a module placed in a reaction body, in a `fn` or in a closure is
    // `E0350`, at the placement; a core module (`ddt`, `limit`) in a reaction body is too. `cross`
    // and `at` in a reaction body are `E0443` (§13 rule 10).
    let items = "module Child() { }\nmodule Top(in clk: Bit, p: Electrical) { }\n";
    for (body, at, code) in [
        ("let c = Child();", "Child()", "E0350"),
        ("let d = ddt(v(p));", "ddt(v(p))", "E0350"),
        ("let e = cross(v(p));", "cross(v(p))", "E0443"),
        ("let t = at(1.0);", "at(1.0)", "E0443"),
    ] {
        let src = format!("{items}digital for Top {{ on clk.rise() {{ {body} }} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), [code], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    for src in [
        "module Child() { }\nfn place() -> f64 { let c = Child(); 1.0 }\nmodule Top() { let x = place(); }",
        "module Child() { }\nmodule Top(p: Electrical) { }\nanalog for Top { let x = limit(v(p), |new, old| { let c = Child(); new }); }",
    ] {
        let d = diagnostics(src);
        assert_eq!(codes(&d), ["E0350"], "{src}");
        assert_eq!(snippet(src, &d[0]), "Child()", "{src}");
    }
}
