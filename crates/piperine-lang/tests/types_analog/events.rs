//! Events, reactions and records of `piperine_lang::types` (feature `msl-digital`, DIG-05): the
//! forms that make an `Event` (`changed`, `rise`, `fall`, `cross`, `at`, `||`, `&&` with a guard),
//! where one may stand (part IV §13) and the head of an `on` (`E0430`). Sources are written over
//! the embedded standard library.


use piperine_lang::pom::{ExprId, ExprKind};
use piperine_lang::types::{Builtin, Ty, Types};
use piperine_lang_fixtures::buck::BuckSource;
use piperine_lang_fixtures::checked::CheckedSource;

/// The sources of reactions over a module `Top` (`HEAD`): empty reactions of given heads, and one reaction with a body.
struct ReactionSource;

impl ReactionSource {
    /// `Top` with `heads` as the heads of empty reactions.
    fn with_heads(heads: &[&str]) -> String {
        let reactions: String = heads.iter().map(|h| format!("on {h} {{ }}\n")).collect();
        format!("{HEAD}digital for Top {{\n{reactions}}}")
    }

    /// `Top` with one reaction on `head` whose body is `body`, over a `bool` carrier `q` and an `f64`
    /// carrier `x`.
    fn with_body(head: &str, body: &str) -> String {
        format!("module Top(in clk: Bit, in en: Bit, out q: Bit = false, out x: Bit<f64> = 0.0) {{ }}\ndigital for Top {{\non {head} {{ {body} }}\n}}")
    }
}

/// The buck's own modules written as sources of the event tests, over the buck's constants.
struct BuckReactions;

impl BuckReactions {
    /// The buck's `Dpwm` with the buck's own helper functions, minus the `Never` monitor it places
    /// (the monitor is a stdlib module with a `require`; T14 and T16 reach it).
    fn dpwm() -> String {
        let module = BuckSource::module_with_digital("dpwm.msl", "Dpwm");
        let monitor = "    /// The two commands are never on together (SPEC §5.3, §10 S10).\n    let no_shoot_through = Never(clk, hs & ls);\n";
        assert!(module.contains(monitor), "the buck's Dpwm places Never");
        format!("{}\n{}\n{}\n{}", BuckSource::spec(), BuckSource::item("dpwm.msl", "fn rev3"), BuckSource::item("dpwm.msl", "fn dithered_width"), module.replace(monitor, ""))
    }

    /// The buck's constants and the module `name` of `file` with its `digital` block.
    fn buck_module(file: &str, name: &str) -> String {
        format!("{}\n{}", BuckSource::spec(), BuckSource::module_with_digital(file, name))
    }
}

const HEAD: &str = "module Top(in clk: Bit, in en: Bit, in k: Bit<f64>, p: Electrical) { }\n";

#[test]
fn event_expressions_are_typed() {
    // DIG-05 AC 1: `c.changed()`, `c.rise()`, `c.fall()`, `cross(e)`, `at(t)`, `a || b` and
    // `ev && guard` are an `Event`, in a body `let` and as the head of an `on`; a guard reads
    // anything a reaction reads, a probe included.
    let src = "module Top(in clk: Bit, in en: Bit, in k: Bit<f64>, p: Electrical) {\n    let a = clk.changed();\n    let b = clk.rise();\n    let c = clk.fall();\n    let d = cross(v(p) - 0.5);\n    let e = at(1u + k * 2.0);\n    let f = clk.rise() || clk.fall();\n    let g = clk.rise() && en;\n    let h = (clk.rise() || en.changed()) && v(p) > 0.2;\n}";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
        assert_eq!(types.local(CheckedSource::binding(&design, name)), Some(&Ty::Event), "{name}");
    }
    let builtins: Vec<Option<Builtin>> = ["changed", "rise", "fall"].iter().flat_map(|n| (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && matches!(&design.expr_node(*e).kind, ExprKind::MethodCall { method, .. } if method.text == *n)).map(|e| types.builtin(e)).collect::<Vec<_>>()).collect();
    assert_eq!(builtins.iter().filter(|b| **b == Some(Builtin::Changed)).count(), 2);
    assert_eq!(builtins.iter().filter(|b| **b == Some(Builtin::Rise)).count(), 4);
    assert_eq!(builtins.iter().filter(|b| **b == Some(Builtin::Fall)).count(), 2);

    // As the heads of reactions, a guard of any `bool` included.
    let src = ReactionSource::with_heads(&["clk.rise()", "cross(v(p) - 0.5)", "at(1.0)", "clk.rise() || en.fall()", "clk.rise() && en && v(p) > 0.0"]);
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    for head in ["clk.rise()", "cross(v(p) - 0.5)", "at(1.0)", "clk.rise() || en.fall()", "clk.rise() && en && v(p) > 0.0"] {
        let found = CheckedSource::expressions_reading(&design, &types, &src, head);
        assert!(!found.is_empty() && found.iter().all(|e| types.ty(*e) == Some(&Ty::Event)), "{head}: {found:?}");
    }

    // `rise` and `fall` are the edges of a `Bit` of `bool`; a node has no `changed`.
    let d = CheckedSource::standard_expected_diagnostics("module Top(in c: Bit<u8>) { let e = c.rise(); }");
    assert_eq!(CheckedSource::codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("rise") && d[0].diagnostic.message.contains("Bit<u8>"), "{}", d[0].diagnostic.message);
    let src = "module Top(p: Electrical) { let e = p.changed(); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0230".to_string()], "p.changed()"));

    // `settle` is an `Event` since unknown-semantics-events T5 (`settle_is_an_event_in_every_event_position`).
    // (A tolerance given to `cross` types since `msl-fixes` T7, see
    // `cross_takes_ttol_and_etol_by_position_and_by_name`.)
    // No tolerance, or tolerances that are `None`, is the event the `Cross` node has.
    let src = ReactionSource::with_heads(&["cross(v(p))", "cross(v(p), None, None)"]);
    CheckedSource::checked(&CheckedSource::standard_design_of(&src), &src);
}

#[test]
fn settle_is_an_event_in_every_event_position() {
    // §13 rule 10: `settle` is a core `Event` value; §13 rule 14: it combines with `&&` and `||`
    // like any event. It replaces the `P0001` "built-in `settle`" this test pinned (L52,
    // unknown-semantics-events UE-03).
    let heads = ["settle", "settle && en", "settle || clk.rise()", "clk.rise() || settle", "(settle || clk.changed()) && en"];
    let src = ReactionSource::with_heads(&heads);
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    for head in heads {
        let found = CheckedSource::expressions_reading(&design, &types, &src, head);
        assert!(!found.is_empty() && found.iter().all(|e| types.ty(*e) == Some(&Ty::Event)), "{head}: {found:?}");
    }
    // a body `let` is an event position, and the name it binds stands in a head
    let src = "module Top(in clk: Bit, in en: Bit) { let s = settle; let t = s || clk.fall(); }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    for name in ["s", "t"] {
        assert_eq!(types.local(CheckedSource::binding(&design, name)), Some(&Ty::Event), "{name}");
    }
}

#[test]
fn settle_outside_an_event_position_is_e0441() {
    // §13 rule 8, rule 10: `settle` is an `Event` and is never carried, stored or passed
    let cases: [(&str, &str); 3] = [
        ("module Top(in clk: Bit, out q: Bit) { q = settle; }", "settle"),
        ("module Top(in clk: Bit) { }\nanalog for Top { let x = settle; }", "settle"),
        ("fn pick(e: bool) -> bool { e }\nmodule Top(in clk: Bit) { let y = pick(settle); }", "settle"),
    ];
    for (src, at) in cases {
        let d = CheckedSource::standard_expected_diagnostics(src);
        assert_eq!(CheckedSource::codes(&d), ["E0441"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(src, &d[0]), at, "{src}");
    }
}

#[test]
fn a_bool_head_is_e0430() {
    // DIG-05 AC 2 (§12.1.10): the head of an `on` is an `Event`. `on clk { .. }` with `clk: Bit` is
    // `E0430` at `clk`; so is any other `bool`, and any other type.
    for head in ["clk", "en && clk", "k > 0.0", "true", "k"] {
        let src = ReactionSource::with_heads(&[head]);
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0430"], "{head}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), head);
    }
    // An `||` of an event and a `bool` is a mismatch of its operand.
    let src = ReactionSource::with_heads(&["clk.rise() || en"]);
    let d = CheckedSource::standard_expected_diagnostics(&src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(&src, &d[0])), (vec!["E0180".to_string()], "en"));
}

#[test]
fn a_misplaced_not_or_and_is_e0440() {
    // DIG-05 AC 3 (§13 rule 1): `!` applies to a `bool`, never to an `Event`; `&&` has the event on
    // the left and the guard on the right.
    for head in ["!clk.rise()", "(!clk.fall()) || clk.rise()", "en && clk.rise()", "clk.rise() || (en && clk.fall())"] {
        let src = ReactionSource::with_heads(&[head]);
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0440"], "{head}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    }
    let src = ReactionSource::with_heads(&["!clk.rise()"]);
    assert_eq!(CheckedSource::snippet(&src, &CheckedSource::standard_expected_diagnostics(&src)[0]), "!clk.rise()");
    let src = ReactionSource::with_heads(&["en && clk.rise()"]);
    assert_eq!(CheckedSource::snippet(&src, &CheckedSource::standard_expected_diagnostics(&src)[0]), "en && clk.rise()");
    // `!` on a `bool` guard is the ordinary `!`.
    let src = ReactionSource::with_heads(&["clk.rise() && !en"]);
    CheckedSource::checked(&CheckedSource::standard_design_of(&src), &src);
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
        let d = CheckedSource::standard_expected_diagnostics(src);
        assert_eq!(CheckedSource::codes(&d), ["E0441"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(src, &d[0]), at, "{src}");
    }
    // An event position takes one: a body `let`, a function returning `Event`, an operand of `||`.
    let src = "module Top(in clk: Bit, in en: Bit) { let a = clk.rise(); let b = a || clk.fall(); }";
    CheckedSource::checked(&CheckedSource::standard_design_of(src), src);
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
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0442"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{body}");
    }
    // A time that reads carriers and params is discrete.
    let src = "module Top(in k: Bit<u8>) { half: f64 = 50n; let e = at(1u + k as f64 * half); }";
    CheckedSource::checked(&CheckedSource::standard_design_of(src), src);
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
        let d = CheckedSource::standard_expected_diagnostics(src);
        assert_eq!(CheckedSource::codes(&d)[0], "E0443", "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(src, &d[0]), at, "{src}");
    }
}

#[test]
fn a_reaction_body_is_typed() {
    // DIG-05 AC 7: a body with `let`, `if`, `match`, calls, commits with and without `after`, a probe
    // and `now()`. The reactions of `Dpwm`, `Pid`, `RefRamp`, `Adc` and `SetpointReg` of the buck
    // are the positive cases, read as written.
    CheckedSource::assert_standard_top_clean(&BuckReactions::dpwm(), "Dpwm");
    let pid = format!("{}\n{}", BuckReactions::buck_module("control.msl", "Pid"), "module Top() { }");
    let design = CheckedSource::standard_design_of(&pid);
    let types = Types::check(&design, "Pid").unwrap_or_else(|d| panic!("Pid: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let got: Vec<Option<&Ty>> = ["e", "p", "u", "windup", "d"].iter().map(|n| types.local(CheckedSource::binding(&design, n))).collect();
    assert_eq!(got, [Some(&Ty::Int(16)), Some(&Ty::Int(32)), Some(&Ty::Int(32)), Some(&Ty::Bool), Some(&Ty::Int(32))]);
    let ramp = format!("{}\n{}", BuckReactions::buck_module("control.msl", "RefRamp"), "module Top() { }");
    CheckedSource::assert_standard_top_clean(&ramp, "RefRamp");
    CheckedSource::assert_standard_top_clean(&BuckReactions::buck_module("control.msl", "SetpointReg"), "SetpointReg");
    CheckedSource::assert_standard_top_clean(&BuckReactions::buck_module("sense.msl", "Adc"), "Adc");

    // A body of every statement kind, and the read of a carrier in a guard and in the body
    // (DIG-04 AC 3): `u9`.
    let src = "#[derive(Repr, PartialEq)]\nenum Phase { Idle, Run }\n\
module Top(in clk: Bit, in go: Bit, p: Electrical, out cnt: Bit<u9> = 0, out when: Bit<f64> = 0.0, out ph: Bit<Phase> = Phase::Idle) { gap: f64 = 5n; }\n\
digital for Top {\n    on clk.rise() && cnt > 3 {\n        let next = cnt + 1;\n        let level = v(p);\n        if go { cnt <- next; } else { cnt <- 0 after gap; }\n        match ph { Phase::Idle => { ph <- Phase::Run; } Phase::Run => { ph <- Phase::Idle after gap; } }\n        when <- now() + level;\n    }\n}";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    let cnts = CheckedSource::expressions_reading(&design, &types, src, "cnt");
    assert!(cnts.len() >= 3 && cnts.iter().all(|e| types.ty(*e) == Some(&Ty::UInt(9)) || matches!(types.ty(*e), Some(Ty::UInt(9)))), "{cnts:?}");
    assert_eq!(types.local(CheckedSource::binding(&design, "next")), Some(&Ty::UInt(9)));
    assert_eq!(types.local(CheckedSource::binding(&design, "level")), Some(&Ty::F64));
}

#[test]
fn a_commit_of_another_type_is_e0432() {
    // DIG-05 AC 8 (§12.1.9): the committed expression has the value type of its target. An `Event`
    // committed where a type other than `bool` is expected is `E0441` (§13 rules 2 and 8, design D2;
    // onto a `bool` it is read, `an_event_committed_onto_a_bool_carrier_reads_as_a_bool`).
    for (body, at, code) in [
        ("count <- true;", "true", "E0432"),
        ("count <- 1.5;", "1.5", "E0432"),
        ("count <- wide;", "wide", "E0432"),
        ("count <- clk.rise();", "clk.rise()", "E0441"),
        ("count <- clk.rise() || clk.fall();", "clk.rise() || clk.fall()", "E0441"),
    ] {
        let src = format!("module Top(in clk: Bit, in wide: Bit<u12>, out count: Bit<u9> = 0, out flag: Bit = false) {{ }}\ndigital for Top {{ on clk.rise() {{ {body} }} }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), [code], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{body}");
    }
    // A literal takes the type of its target (a `<-` in an `analog` block is a parse error of the POM).
    let src = "module Top(in clk: Bit, out count: Bit<u9> = 0) { }\ndigital for Top { on clk.rise() { count <- 511; } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
}

#[test]
fn an_operand_of_another_type_in_a_commit_is_e0432() {
    // digital~54ec4a1d of the Book (chapter 7, §12.1.9): `count <- count + 1.5` on an integer carrier
    // is `E0432` at the operand that is not the carrier's type, not the plain mismatch `E0180`
    let src = "module Top(in clk: Bit, out count: Bit<u9> = 0) { }\ndigital for Top { on clk.rise() { count <- count + 1.5; } }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!(CheckedSource::codes(&d), ["E0432"], "{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    assert_eq!(CheckedSource::snippet(src, &d[0]), "1.5");
    let src = "module Top(in clk: Bit, out count: Bit<u9> = 0) { }\ndigital for Top { on clk.rise() { count <- count + 1; } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
    // through a parenthesis, an `if` and a block the rule keeps its code
    for (value, operand) in [("(count + 1.5)", "1.5"), ("if true { count + 1.5 } else { count }", "1.5"), ("if true { count } else { count + 1.5 }", "1.5"), ("{ count + 1.5 }", "1.5")] {
        let src = format!("module Top(in clk: Bit, out count: Bit<u9> = 0) {{ }}\ndigital for Top {{ on clk.rise() {{ count <- {value}; }} }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0432"], "{value}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), operand, "{value}");
    }
}

#[test]
fn a_delay_that_is_not_f64_is_e0433() {
    // DIG-05 AC 9 (§12.1.13, §8.2.22): the delay of a commit, and of a definition, is an `f64`.
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit = false) { }\ndigital for Top { on clk.rise() { q <- d after 5; } }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0433".to_string()], "5"));
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit = false) { }\ndigital for Top { on clk.rise() { q <- d after true; } }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0433".to_string()], "true"));
    let src = "module Top(in d: Bit, out q: Bit = false) { q = d after 1; }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0433".to_string()], "1"));
    // Seconds as an `f64`, a read of a carrier and a call included, type.
    let src = "module Top(in clk: Bit, in d: Bit, in w: Bit<f64>, out q: Bit = false) { tpd: f64 = 1n; }\ndigital for Top { on clk.rise() { q <- d after 5n; q <- d after tpd + w; } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
}

#[test]
fn a_definition_and_a_commit_are_e0221() {
    // DIG-04 AC 10 (§5.2, §12.1.8): one module drives a carrier by one definition or by commits,
    // never both; the error is at the commit. A carrier a commit drives and a module hands to an
    // `out` port has a second drive (`E0220`, DIG-04 AC 11).
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit) { q = d; }\ndigital for Top { on clk.rise() { q <- d; } }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0221".to_string()], "q"));
    assert_eq!(d[0].diagnostic.span().start, src.rfind("q <- d").unwrap());
    // Two commits of one carrier are one drive.
    let src = "module Top(in clk: Bit, in d: Bit, out q: Bit = false) { }\ndigital for Top { on clk.rise() { q <- d; q <- !d after 1n; } on clk.fall() { q <- true; } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
    // Commits and a hand-over of one carrier.
    let src = "module Source(out q: Bit = false) { }\nmodule Top(in clk: Bit, in d: Bit) { signal s: Bit; let a = Source(s); }\ndigital for Top { on clk.rise() { s <- d; } }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!(CheckedSource::codes(&d), ["E0220"]);
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
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), [code], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{body}");
    }
    for src in [
        "module Child() { }\nfn place() -> f64 { let c = Child(); 1.0 }\nmodule Top() { let x = place(); }",
        "module Child() { }\nmodule Top(p: Electrical) { }\nanalog for Top { let x = limit(v(p), |new, old| { let c = Child(); new }); }",
    ] {
        let d = CheckedSource::standard_expected_diagnostics(src);
        assert_eq!(CheckedSource::codes(&d), ["E0350"], "{src}");
        assert_eq!(CheckedSource::snippet(src, &d[0]), "Child()", "{src}");
    }
}

#[test]
fn emit_is_typed_in_a_reaction() {
    // DIG-05 AC 11 (§19 rules 10 and 11): `emit!("conv", code = q, ok = true)` in a reaction has the
    // fields of a value type; `log` and `require` of the stdlib type-check when a reaction calls
    // them, the buck's `Supervisor` and the stdlib's `Never` among them; a string literal is `&str`.
    let src = "module Top(in clk: Bit, in d: Bit<u10>) { }\ndigital for Top {\n    on clk.rise() {\n        let q = d;\n        emit!(\"conv\", code = q, ok = true);\n        log(Severity::Warning, \"top: fault\");\n        require(\"in_range\", q < 1000);\n    }\n}";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    assert_eq!(types.ty(CheckedSource::expressions_reading(&design, &types, src, "emit!(\"conv\", code = q, ok = true)")[0]), Some(&Ty::Unit));
    assert_eq!(types.ty(CheckedSource::expressions_reading(&design, &types, src, "\"top: fault\"")[0]), Some(&Ty::Str));
    let (log, require) = (CheckedSource::standard_function(&design, "log"), CheckedSource::standard_function(&design, "require"));
    assert_eq!(types.emits(log).map(|s| s.iter().cloned().collect::<Vec<_>>()), Some(vec!["log".to_string()]));
    assert_eq!(types.emits(require).map(|s| s.iter().cloned().collect::<Vec<_>>()), Some(vec!["checks".to_string()]));
    assert!(types.reached().contains(&log) && types.reached().contains(&require));
    // The stdlib's own monitor places and checks: `Never` is `on clk.rise() { require(..) }`.
    let src = "module Top(in clk: Bit, in bad: Bit) { let n = Never(clk, bad); }";
    CheckedSource::assert_standard_top_clean(src, "Top");
    // The supervisor of the buck calls `log` from a reaction.
    let mut supervisor = String::new();
    supervisor += &BuckSource::spec();
    supervisor += "\n";
    supervisor += &BuckSource::item("supervisor.msl", "pub enum Mode");
    supervisor += "\n";
    supervisor += &BuckSource::item("supervisor.msl", "pub enum Fault");
    supervisor += "\n";
    supervisor += &BuckSource::item("supervisor.msl", "pub fn in_window");
    supervisor += "\n";
    supervisor += &BuckSource::item("supervisor.msl", "pub fn ovp_threshold");
    supervisor += "\n";
    supervisor += &BuckSource::module_with_digital("supervisor.msl", "Supervisor");
    CheckedSource::assert_standard_top_clean(&supervisor, "Supervisor");
    // A string is not a value a `let` or a carrier holds.
    let d = CheckedSource::standard_expected_diagnostics("module Top() { let s = \"a\"; }");
    assert_eq!(CheckedSource::codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("string literal"), "{}", d[0].diagnostic.message);
}

#[test]
fn a_repeated_or_reserved_field_is_e0604() {
    // DIG-05 AC 12 (§19 rule 11): a field name is distinct within one `emit!`. The error is at the
    // field.
    for (fields, at) in [("a = 1.0, a = 2.0", "a = 2.0"), ("a = 1.0, b = 2.0, a = true", "a = true")] {
        let src = format!("module Top(in clk: Bit) {{ }}\ndigital for Top {{ on clk.rise() {{ emit!(\"s\", {fields}); }} }}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0604"], "{fields}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{fields}");
    }
    // A field that is not a value.
    let src = "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"s\", e = clk.rise()); } }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!(CheckedSource::codes(&d), ["E0606"], "an event is not a value: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

#[test]
fn a_field_type_conflict_is_e0605() {
    // DIG-05 AC 13 (§19 rules 12 and 15): `pass = 1.0` on `checks` is `E0605`; so is one field with
    // two types on one stream of a module, also through a function the module calls. Two modules
    // are two instances and write two streams.
    for (src, at) in [
        ("module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"checks\", kind = \"require\", name = \"n\", pass = 1.0); } }", "pass = 1.0"),
        ("module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"sim\", finish = 1); } }", "finish = 1"),
        ("module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"checks\", margin = true); } }", "margin = true"),
        ("module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"conv\", x = 1.0); emit!(\"conv\", x = true); } }", "x = true"),
        ("fn note(v: f64) { emit!(\"conv\", x = v); }\nmodule Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { note(1.0); emit!(\"conv\", x = true); } }", "x = true"),
    ] {
        let d = CheckedSource::standard_expected_diagnostics(src);
        assert_eq!(CheckedSource::codes(&d), ["E0605"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(src, &d[0]), at, "{src}");
    }
    let src = "module A(in clk: Bit) { }\ndigital for A { on clk.rise() { emit!(\"conv\", x = 1.0); } }\nmodule Top(in clk: Bit) { let a = A(clk); }\ndigital for Top { on clk.rise() { emit!(\"conv\", x = true); } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
    // Another type on a stream that is not reserved, and the stdlib's own fields, are fine.
    let src = "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"checks\", kind = \"cover\", name = \"hit\", pass = true, margin = 0.5, extra = 3); emit!(\"sim\", finish = true, failed = false, msg = \"done\"); } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
}

#[test]
fn emit_where_it_is_not_allowed_is_e0601() {
    // DIG-05 AC 14 (§19 rule 10): `emit!` reached from a definition, a module-body `let` or a const
    // context is `E0601`, at the emit; a function that calls it, at the call, is `E0122` (params-and-io PI-12
    // AC 3, §3 and §6: `E0601` is `emit!` written directly; the library's `require` is a named function over `emit!`).
    let items = "fn check(ok: bool) -> bool { require(\"c\", ok); ok }\n";
    for (src, at, code) in [
        (format!("{items}module Top(in a: Bit, out q: Bit) {{ q = check(a); }}"), "check(a)", "E0122"),
        (format!("{items}module Top(in a: Bit) {{ let x = check(a); }}"), "check(a)", "E0122"),
        (format!("{items}const C: bool = check(true);\nmodule Top() {{ k: bool = C; }}"), "check(true)", "E0122"),
        (format!("module Top(in a: Bit) {{ let x = emit!(\"s\", f = 1.0); }}"), "emit!(\"s\", f = 1.0)", "E0601"),
        (format!("{items}module Top(in a: Bit, out q: Bit) {{ q = !require(\"c\", true); }}"), "require(\"c\", true)", "E0122"),
    ] {
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d)[0], code, "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{src}");
    }
    // A function that reaches `emit!` is callable from another one, and from a reaction.
    let src = "fn check(ok: bool) { require(\"c\", ok); }\nfn both(ok: bool) { check(ok); }\nmodule Top(in clk: Bit, in a: Bit) { }\ndigital for Top { on clk.rise() { both(a); } }";
    CheckedSource::assert_standard_top_clean(src, "Top");
}

#[test]
fn a_check_reached_from_analog_is_e0603() {
    // DIG-05 AC 15 (§19 rules 4 and 14): a site reached from `analog` that writes `sim` or `checks` is
    // `E0603`; the check follows calls (`require` in `analog`, or in a `fn` called from it). A trace
    // on another stream is no `E0603` (the M1 subset refuses it by name instead, edge case 7).
    let items = "fn guard(ok: bool) { require(\"g\", ok); }\n";
    for (src, at) in [
        (format!("module Top(p: Electrical) {{ }}\nanalog for Top {{ require(\"n\", v(p) < 1.0); }}"), "require(\"n\", v(p) < 1.0)"),
        (format!("{items}module Top(p: Electrical) {{ }}\nanalog for Top {{ guard(v(p) < 1.0); }}"), "guard(v(p) < 1.0)"),
        (format!("module Top(p: Electrical) {{ }}\nanalog for Top {{ emit!(\"sim\", finish = true); }}"), "emit!(\"sim\", finish = true)"),
        (format!("module Top(p: Electrical) {{ }}\nanalog for Top {{ emit!(\"checks\", pass = true); }}"), "emit!(\"checks\", pass = true)"),
    ] {
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0603"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{src}");
    }
    // analog-operators T23: an `emit!` written in `analog` on another stream is a trace and checks; a `log` is a
    // call of a `fn` that emits, inlined where it is called (closing round, GAPS L72), and neither is an `E0603`.
    CheckedSource::assert_standard_top_clean("module Top(p: Electrical) { }\nanalog for Top { emit!(\"trace\", vp = v(p)); log(Severity::Info, \"stepped\"); }", "Top");
}

#[test]
fn an_emit_reached_from_analog_through_a_fn_checks() {
    // Edge case 7 (verification F3), as the closing round of analog-operators leaves it (GAPS L72): an `emit!` written
    // in `analog` is a trace (AO-09), and one reached through `log` or through a `fn` is the trace of the call, which
    // the elaborator inlines where it stands (§19 rule 10); only `sim` and `checks` are `E0603`.
    let items = "fn note(x: f64) { emit!(\"trace\", x = x); }\n";
    CheckedSource::assert_standard_top_clean("module Top(p: Electrical) { }\nanalog for Top { log(Severity::Info, \"stepped\"); i(p) +> v(p); }", "Top");
    CheckedSource::assert_standard_top_clean(&format!("{items}module Top(p: Electrical) {{ }}\nanalog for Top {{ note(v(p)); i(p) +> v(p); }}"), "Top");
    CheckedSource::assert_standard_top_clean("module Top(p: Electrical) { }\nanalog for Top { emit!(\"trace\", vp = v(p)); i(p) +> v(p); }", "Top");
    // The same `log` in a reaction checks.
    CheckedSource::assert_standard_top_clean("module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { log(Severity::Info, \"stepped\"); } }", "Top");
}

#[test]
fn cross_takes_ttol_and_etol_by_position_and_by_name() {
    // FIX-12 AC 1: `cross(e, ttol, etol)` with either tolerance left out, positionally or by
    // name (§3.2), is an `Event`; the binding of each call keeps its tolerance in its slot.
    let heads = [
        ("cross(v(p), 1p)", [Some("v(p)"), Some("1p"), None]),
        ("cross(v(p), etol: 1u)", [Some("v(p)"), None, Some("1u")]),
        ("cross(v(p), 1p, 1u)", [Some("v(p)"), Some("1p"), Some("1u")]),
        ("cross(v(p), etol: 1u, ttol: 1p)", [Some("v(p)"), Some("1p"), Some("1u")]),
        ("cross(v(p), None, Some(1u))", [Some("v(p)"), Some("None"), Some("Some(1u)")]),
    ];
    for (head, wanted) in heads {
        let src = ReactionSource::with_heads(&[head]);
        let design = CheckedSource::standard_design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        let call = (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| &src[design.expr_node(*e).span.range()] == head).unwrap_or_else(|| panic!("no `{head}`"));
        assert_eq!(types.ty(call), Some(&Ty::Event), "{head}");
        let bound: Vec<Option<&str>> = types.binding(call).unwrap_or_else(|| panic!("no binding for `{head}`")).iter().map(|s| s.map(|e| &src[design.expr_node(e).span.range()])).collect();
        assert_eq!(bound, wanted, "{head}");
    }
    // A name that is no parameter, one bound twice, `e` by name, and a count the row cannot take
    // (the plain mismatch of a core primitive's own count).
    for (head, code) in [("cross(v(p), atol: 1u)", "E0140"), ("cross(v(p), 1p, ttol: 2p)", "E0140"), ("cross(v(p), e: 1.0)", "E0140"), ("cross(v(p), 1p, 1u, 2u)", "E0141"), ("cross()", "E0141")] {
        let src = ReactionSource::with_heads(&[head]);
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), [code], "{head}");
    }
}

#[test]
fn an_event_reads_as_a_bool_in_a_reaction_body() {
    // UE-01 AC 1, §13 rule 2: an event used as a `bool` inside a reaction says whether it fired in
    // the microstep: `if clk.rise() { .. }`, `!ev`, `en && ev`, an event not in the head, a `let`
    // bound event. Each read is recorded and typed `bool`.
    for body in [
        "if clk.rise() { q <- true; }",
        "if !clk.fall() { q <- true; }",
        "if en && clk.rise() { q <- true; }",
        "if clk.rise() && en { q <- true; }",
        "if clk.rise() || clk.fall() { q <- true; }",
    ] {
        let src = ReactionSource::with_body("clk.changed()", body);
        let design = CheckedSource::standard_design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        let reads: Vec<ExprId> = (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| CheckedSource::in_top(&design, *e) && types.event_read(*e)).collect();
        assert!(!reads.is_empty(), "{body}: an event read is recorded");
        assert!(reads.iter().all(|e| types.ty(*e) == Some(&Ty::Bool)), "{body}: a read is typed bool: {reads:?}");
    }
    let src = ReactionSource::with_body("clk.changed()", "let up = clk.rise(); if up { q <- true; }");
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let reads = CheckedSource::expressions_reading(&design, &types, &src, "clk.rise()");
    assert!(reads.iter().all(|e| types.event_read(*e) && types.ty(*e) == Some(&Ty::Bool)), "the value of a body `let` is read as a bool: {reads:?}");
}

#[test]
fn an_event_committed_onto_a_bool_carrier_reads_as_a_bool() {
    // UE-02 AC 5, §13 rule 2 and design D2: `on up || dn { q <- up; }` commits whether `up` fired
    // in this microstep, as the `Comparator` of §13 does. The event is read, typed `bool`.
    let src = "module Top(in clk: Bit, in en: Bit, out q: Bit = false) { let up = clk.rise(); let dn = clk.fall(); }\ndigital for Top { on up || dn { q <- up; } }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    let committed: Vec<ExprId> = CheckedSource::expressions_reading(&design, &types, src, "up").into_iter().filter(|e| types.event_read(*e)).collect();
    assert_eq!(committed.len(), 1, "the commit's value is the one event read");
    assert_eq!(types.ty(committed[0]), Some(&Ty::Bool));
}

#[test]
fn an_event_where_another_type_is_expected_is_e0441() {
    // UE-02 AC 4, §13 rules 2 and 8: an event read is a `bool`; a commit onto a carrier of another
    // type is `E0441` at the event, and so is a read stored or carried outside any reaction.
    let src = ReactionSource::with_body("clk.rise()", "x <- clk.rise();");
    let d = CheckedSource::standard_expected_diagnostics(&src);
    assert_eq!(CheckedSource::codes(&d), ["E0441"], "{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "clk.rise()");
    let src = "module Top(in clk: Bit, out q: Bit) { q = clk.rise(); }";
    let d = CheckedSource::standard_expected_diagnostics(src);
    assert_eq!((CheckedSource::codes(&d), CheckedSource::snippet(src, &d[0])), (vec!["E0441".to_string()], "clk.rise()"));
}

#[test]
fn a_guard_reads_an_event_as_a_bool() {
    // UE-02 AC 1 and 2, §13 rule 9: inside a guard an `Event` subexpression and `!ev` read as a
    // `bool`, as in a reaction body: `on d.changed() && en && !en.changed()`. Each read is recorded.
    let src = "module Top(in d: Bit, in en: Bit) { }\ndigital for Top { on d.changed() && en && !en.changed() { } }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    let negated = CheckedSource::expressions_reading(&design, &types, src, "en.changed()");
    assert_eq!(negated.len(), 1);
    assert!(types.event_read(negated[0]), "`en.changed()` under `!` in a guard is an event read");
    assert_eq!(types.ty(negated[0]), Some(&Ty::Bool));
    // A guard that is itself an event expression (`ev && bool`) reads as a `bool` as a whole.
    let src = "module Top(in d: Bit, in en: Bit) { }\ndigital for Top { on d.changed() && (en.rise() && en) { } }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    let guard = CheckedSource::expressions_reading(&design, &types, src, "en.rise() && en");
    assert!(guard.iter().all(|e| types.event_read(*e) && types.ty(*e) == Some(&Ty::Bool)), "{guard:?}");
    // The event of the head is no read: the head stays an `Event`.
    let head = CheckedSource::expressions_reading(&design, &types, src, "d.changed()");
    assert!(head.iter().all(|e| !types.event_read(*e)), "{head:?}");
}

#[test]
fn negating_an_event_outside_a_guard_or_body_is_e0440() {
    // UE-02 AC 3, §13 rule 1: `!` applies to a `bool`. Outside a guard and a reaction body (a head,
    // an operand of `||`, a body `let` of a module) it is `E0440` at the `!` expression.
    for (src, at) in [
        (ReactionSource::with_heads(&["!clk.rise()"]), "!clk.rise()"),
        (ReactionSource::with_heads(&["clk.rise() || !clk.fall()"]), "!clk.fall()"),
        ("module Top(in clk: Bit) { let e = !clk.rise(); }".to_string(), "!clk.rise()"),
    ] {
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0440"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{src}");
    }
}

#[test]
fn a_bool_and_an_event_in_an_event_position_is_e0440() {
    // UE-02 AC 3, §13 rule 1: `&&` has the event on the left, `ev && guard`; a `bool` on the left
    // and an `Event` on the right in an event position is `E0440` at the `&&` expression.
    for head in ["en && clk.rise()", "clk.rise() || (en && clk.fall())"] {
        let src = ReactionSource::with_heads(&[head]);
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0440"], "{head}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    }
    // In a reaction body the same `&&` is an ordinary `bool` expression (§13 rule 2).
    let src = ReactionSource::with_body("clk.changed()", "if en && clk.rise() { q <- true; }");
    CheckedSource::checked(&CheckedSource::standard_design_of(&src), &src);
}

/// A function that returns `Event` (§13 rule 7) and the module that places it.
const HANDSHAKE: &str = "fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }\n";

#[test]
fn a_function_returning_event_is_accepted_in_every_event_position() {
    // UE-04 AC 1 and 2, §13 rule 7: a function returning `Event` is an `Event` in the head of an `on`, in a body `let`, as
    // the result of a module, as the return value of another such function and as an operand of `||` and `&&`
    for (what, body) in [
        ("a head", "digital for Top { on handshake(req, ack) { } }"),
        ("an operand of `||`", "digital for Top { on handshake(req, ack) || clk.rise() { } }"),
        ("an operand of `&&`", "digital for Top { on handshake(req, ack) && en { } }"),
        ("a body let", "digital for Top { on ready { } }"),
        ("another function's value", "fn twice(req: Bit, ack: Bit) -> Event { handshake(req, ack) || handshake(ack, req) }\ndigital for Top { on twice(req, ack) { } }"),
        ("a nested call", "fn gated(req: Bit, ack: Bit, en: Bit) -> Event { handshake(req, ack) && en }\ndigital for Top { on gated(req, ack, en) { } }"),
    ] {
        let src = format!("{HANDSHAKE}module Top(in clk: Bit, in req: Bit, in ack: Bit, in en: Bit) {{ let ready = handshake(req, ack); }}\n{body}");
        let design = CheckedSource::standard_design_of(&src);
        let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{what}: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
        let calls = CheckedSource::expressions_reading(&design, &types, &src, "handshake(req, ack)");
        assert!(calls.iter().any(|call| types.ty(*call) == Some(&Ty::Event)), "{what}: the call is an `Event`");
    }
}

#[test]
fn a_function_returning_event_is_the_result_of_a_module() {
    // UE-04 AC 1, §13: the call is the result of a module with an `Event` result, and a head places that module; no other
    // placement of the call is in the source
    let src = format!("{HANDSHAKE}module Ready(in req: Bit, in ack: Bit) -> Event {{ handshake(req, ack) }}\nmodule Top(in req: Bit, in ack: Bit) {{ }}\ndigital for Top {{ on Ready(req, ack) {{ }} }}");
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let calls = CheckedSource::expressions_reading(&design, &types, &src, "handshake(req, ack)");
    assert!(!calls.is_empty() && calls.iter().all(|call| types.ty(*call) == Some(&Ty::Event)), "the module's result is an `Event`: {calls:?}");
}

#[test]
fn a_generic_function_returning_event_is_an_event_at_each_nature() {
    // UE-04 AC 1, §3.8: `crossed<N>` returns an `Event` whatever nature its argument has
    let src = "fn crossed<N: Nature>(a: N) -> Event { cross(potential(a)) }\nmodule Top(a: Electrical, t: Thermal, out q: Bit = false) { }\ndigital for Top { on crossed(a) { q <- true; } on crossed(t) { q <- false; } }";
    let design = CheckedSource::standard_design_of(src);
    let types = CheckedSource::checked(&design, src);
    for call in ["crossed(a)", "crossed(t)"] {
        let reads = CheckedSource::expressions_reading(&design, &types, src, call);
        assert!(!reads.is_empty() && reads.iter().all(|read| types.ty(*read) == Some(&Ty::Event)), "{call} is an `Event`: {reads:?}");
    }
}

#[test]
fn a_function_returning_event_outside_an_event_position_is_e0441() {
    // UE-04 AC 4, §13 rule 8: outside an event position, a guard and a reaction body an `Event` function's call is `E0441`
    // like any `Event`; inside a guard and a reaction body it reads as a `bool` (UE-01)
    for (body, at) in [
        ("module Top(in req: Bit, in ack: Bit, out q: Bit) { q = handshake(req, ack); }", "handshake(req, ack)"),
        ("module Top(in req: Bit, in ack: Bit) { let y = 1.0 + handshake(req, ack); }", "handshake(req, ack)"),
    ] {
        let src = format!("{HANDSHAKE}{body}");
        let d = CheckedSource::standard_expected_diagnostics(&src);
        assert_eq!(CheckedSource::codes(&d), ["E0441"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{body}");
    }
    let src = format!("{HANDSHAKE}module Top(in clk: Bit, in req: Bit, in ack: Bit, out q: Bit = false) {{ }}\ndigital for Top {{ on clk.rise() {{ q <- handshake(req, ack); }} on clk.fall() && handshake(req, ack) {{ }} }}");
    let design = CheckedSource::standard_design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let reads = CheckedSource::expressions_reading(&design, &types, &src, "handshake(req, ack)");
    assert!(reads.iter().all(|call| types.event_read(*call) && types.ty(*call) == Some(&Ty::Bool)), "read as a `bool` in a body and a guard: {reads:?}");
}
