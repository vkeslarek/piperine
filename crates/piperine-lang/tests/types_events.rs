//! Events, reactions and records of `piperine_lang::types` (feature `msl-digital`, DIG-05): the
//! forms that make an `Event` (`changed`, `rise`, `fall`, `cross`, `at`, `||`, `&&` with a guard),
//! where one may stand (part IV §13) and the head of an `on` (`E0430`). Sources are written over
//! the embedded standard library.

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
